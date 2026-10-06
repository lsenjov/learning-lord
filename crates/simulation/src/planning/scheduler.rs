use crate::SimulationError;
use std::{
    collections::VecDeque,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub(super) type Job<T> = Box<dyn FnOnce() -> Result<T, SimulationError> + Send>;

struct Task {
    cancelled: Arc<AtomicBool>,
    run: Box<dyn FnOnce() + Send>,
}

#[derive(Default)]
struct Queue {
    tasks: VecDeque<Task>,
    stopped: bool,
}

#[derive(Default)]
struct SharedQueue {
    queue: Mutex<Queue>,
    available: Condvar,
}

pub(super) struct Scheduler {
    shared: Arc<SharedQueue>,
    workers: Vec<JoinHandle<()>>,
}

impl Scheduler {
    pub(super) fn new(worker_count: usize) -> Result<Self, SimulationError> {
        let mut scheduler = Self {
            shared: Arc::new(SharedQueue::default()),
            workers: Vec::with_capacity(worker_count),
        };
        for index in 0..worker_count {
            let shared = Arc::clone(&scheduler.shared);
            let worker = thread::Builder::new()
                .name(format!("citizen-planning-{index}"))
                .spawn(move || work(shared))
                .map_err(|_| SimulationError::PlanningFailed)?;
            scheduler.workers.push(worker);
        }
        Ok(scheduler)
    }

    pub(super) fn submit<T: Send + 'static>(&self, jobs: Vec<Job<T>>) -> Request<T> {
        let request = Request {
            shared: Arc::new(SharedResults {
                results: Mutex::new(Results {
                    slots: (0..jobs.len()).map(|_| None).collect(),
                    remaining: jobs.len(),
                    consumed: false,
                    finished: None,
                }),
                started: Instant::now(),
                completed: Condvar::new(),
                cancelled: Arc::new(AtomicBool::new(false)),
            }),
        };
        let tasks = jobs.into_iter().enumerate().map(|(index, job)| {
            let shared = Arc::clone(&request.shared);
            Task {
                cancelled: Arc::clone(&shared.cancelled),
                run: Box::new(move || {
                    let result = catch_unwind(AssertUnwindSafe(job))
                        .unwrap_or(Err(SimulationError::PlanningFailed));
                    let mut results = shared.results.lock().unwrap();
                    if !shared.cancelled.load(Ordering::Acquire) {
                        results.slots[index] = Some(result);
                        results.remaining -= 1;
                        if results.remaining == 0 {
                            results.finished = Some(Instant::now());
                            shared.completed.notify_all();
                        }
                    }
                }),
            }
        });
        // Keep every request's roots contiguous so concurrent submitters cannot interleave them.
        self.shared.queue.lock().unwrap().tasks.extend(tasks);
        self.shared.available.notify_all();
        request
    }

    #[cfg(test)]
    pub(super) fn worker_count(&self) -> usize {
        self.workers.len()
    }
}

impl Drop for Scheduler {
    fn drop(&mut self) {
        self.shared.queue.lock().unwrap().stopped = true;
        self.shared.available.notify_all();
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

fn work(shared: Arc<SharedQueue>) {
    loop {
        let task = {
            let mut queue = shared.queue.lock().unwrap();
            loop {
                if queue.stopped {
                    return;
                }
                if let Some(task) = queue.tasks.pop_front() {
                    break task;
                }
                queue = shared.available.wait(queue).unwrap();
            }
        };
        if !task.cancelled.load(Ordering::Acquire) {
            (task.run)();
        }
    }
}

struct Results<T> {
    slots: Vec<Option<Result<T, SimulationError>>>,
    remaining: usize,
    consumed: bool,
    finished: Option<Instant>,
}

impl<T> Results<T> {
    fn take_completed(&mut self) -> Option<Vec<Result<T, SimulationError>>> {
        if self.remaining > 0 || self.consumed {
            return None;
        }
        self.consumed = true;
        Some(self.slots.drain(..).map(Option::unwrap).collect())
    }
}

struct SharedResults<T> {
    results: Mutex<Results<T>>,
    started: Instant,
    completed: Condvar,
    cancelled: Arc<AtomicBool>,
}

pub(super) struct Request<T> {
    shared: Arc<SharedResults<T>>,
}

impl<T> Request<T> {
    pub(super) fn completed(results: Vec<Result<T, SimulationError>>) -> Self {
        Self {
            shared: Arc::new(SharedResults {
                results: Mutex::new(Results {
                    slots: results.into_iter().map(Some).collect(),
                    remaining: 0,
                    consumed: false,
                    finished: Some(Instant::now()),
                }),
                started: Instant::now(),
                completed: Condvar::new(),
                cancelled: Arc::new(AtomicBool::new(false)),
            }),
        }
    }

    pub(super) fn elapsed(&self) -> Duration {
        let results = self.shared.results.lock().unwrap();
        results
            .finished
            .unwrap_or_else(Instant::now)
            .saturating_duration_since(self.shared.started)
    }

    pub(super) fn cancel(&self) {
        let _results = self.shared.results.lock().unwrap();
        self.shared.cancelled.store(true, Ordering::Release);
        self.shared.completed.notify_all();
    }

    pub(super) fn try_result(&mut self) -> Option<Vec<Result<T, SimulationError>>> {
        let mut results = self.shared.results.lock().unwrap();
        if self.shared.cancelled.load(Ordering::Acquire) {
            return None;
        }
        results.take_completed()
    }

    pub(super) fn wait(self) -> Result<Vec<Result<T, SimulationError>>, SimulationError> {
        let mut results = self.shared.results.lock().unwrap();
        loop {
            if self.shared.cancelled.load(Ordering::Acquire) {
                return Err(SimulationError::PlanningFailed);
            }
            if let Some(completed) = results.take_completed() {
                return Ok(completed);
            }
            results = self.shared.completed.wait(results).unwrap();
        }
    }
}

impl<T> Drop for Request<T> {
    fn drop(&mut self) {
        self.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::AtomicUsize,
        mpsc::{self, Receiver, Sender},
    };

    fn gated_job(label: usize, started: Sender<usize>, release: Receiver<()>) -> Job<usize> {
        Box::new(move || {
            started.send(label).unwrap();
            release.recv().unwrap();
            Ok(label)
        })
    }

    #[test]
    fn queued_roots_start_in_request_and_goal_order() {
        let scheduler = Scheduler::new(1).unwrap();
        let (started, observed) = mpsc::channel();
        let (release, gate) = mpsc::channel();
        let blocker = scheduler.submit(vec![gated_job(0, started.clone(), gate)]);
        assert_eq!(observed.recv().unwrap(), 0);
        let first = scheduler.submit(
            [1, 2, 3]
                .into_iter()
                .map(|label| {
                    let started = started.clone();
                    Box::new(move || {
                        started.send(label).unwrap();
                        Ok(label)
                    }) as Job<usize>
                })
                .collect(),
        );
        let second = scheduler.submit(
            [4, 5]
                .into_iter()
                .map(|label| {
                    let started = started.clone();
                    Box::new(move || {
                        started.send(label).unwrap();
                        Ok(label)
                    }) as Job<usize>
                })
                .collect(),
        );
        release.send(()).unwrap();
        assert_eq!(blocker.wait().unwrap(), [Ok(0)]);
        assert_eq!(first.wait().unwrap(), [Ok(1), Ok(2), Ok(3)]);
        assert_eq!(second.wait().unwrap(), [Ok(4), Ok(5)]);
        assert_eq!(observed.try_iter().collect::<Vec<_>>(), [1, 2, 3, 4, 5]);
    }

    #[test]
    fn free_worker_can_start_next_request_before_earlier_request_finishes() {
        let scheduler = Scheduler::new(2).unwrap();
        let (started, observed) = mpsc::channel();
        let (release, gate) = mpsc::channel();
        let first = scheduler.submit(vec![gated_job(0, started, gate), Box::new(|| Ok(1))]);
        assert_eq!(observed.recv().unwrap(), 0);
        let second = scheduler.submit(vec![Box::new(|| Ok(2))]);
        assert_eq!(second.wait().unwrap(), [Ok(2)]);
        release.send(()).unwrap();
        assert_eq!(first.wait().unwrap(), [Ok(0), Ok(1)]);
    }

    #[test]
    fn concurrent_requests_share_the_worker_limit() {
        let scheduler = Scheduler::new(2).unwrap();
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let gate = Arc::new((Mutex::new(false), Condvar::new()));
        let (started, observed) = mpsc::channel();
        let make_job = || {
            let active = Arc::clone(&active);
            let peak = Arc::clone(&peak);
            let gate = Arc::clone(&gate);
            let started = started.clone();
            Box::new(move || {
                let count = active.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(count, Ordering::SeqCst);
                started.send(()).unwrap();
                let mut released = gate.0.lock().unwrap();
                while !*released {
                    released = gate.1.wait(released).unwrap();
                }
                active.fetch_sub(1, Ordering::SeqCst);
                Ok(())
            }) as Job<()>
        };
        let first = scheduler.submit(vec![make_job(), make_job(), make_job()]);
        let second = scheduler.submit(vec![make_job(), make_job(), make_job()]);
        observed.recv().unwrap();
        observed.recv().unwrap();
        assert_eq!(active.load(Ordering::SeqCst), 2);
        *gate.0.lock().unwrap() = true;
        gate.1.notify_all();
        first.wait().unwrap();
        second.wait().unwrap();
        assert_eq!(peak.load(Ordering::SeqCst), 2);
        assert_eq!(scheduler.worker_count(), 2);
    }

    #[test]
    fn cancelled_queued_roots_are_skipped_and_later_requests_complete() {
        let scheduler = Scheduler::new(1).unwrap();
        let (started, observed) = mpsc::channel();
        let (release, gate) = mpsc::channel();
        let blocker = scheduler.submit(vec![gated_job(0, started, gate)]);
        observed.recv().unwrap();
        let runs = Arc::new(AtomicUsize::new(0));
        let mut cancelled = scheduler.submit(
            (0..3)
                .map(|_| {
                    let runs = Arc::clone(&runs);
                    Box::new(move || {
                        runs.fetch_add(1, Ordering::SeqCst);
                        Ok(())
                    }) as Job<()>
                })
                .collect(),
        );
        cancelled.cancel();
        let later = scheduler.submit(vec![Box::new(|| Ok(4))]);
        release.send(()).unwrap();
        blocker.wait().unwrap();
        assert_eq!(later.wait().unwrap(), [Ok(4)]);
        assert_eq!(runs.load(Ordering::SeqCst), 0);
        assert!(cancelled.try_result().is_none());
    }

    #[test]
    fn completed_request_elapsed_excludes_time_before_collection() {
        let request = Request::<()>::completed(vec![Ok(())]);
        let elapsed = request.elapsed();
        std::thread::yield_now();
        assert_eq!(request.elapsed(), elapsed);
    }

    #[test]
    fn cancellation_ignores_running_root_and_completed_results() {
        let scheduler = Scheduler::new(1).unwrap();
        let (started, observed) = mpsc::channel();
        let (release, gate) = mpsc::channel();
        let mut cancelled = scheduler.submit(vec![gated_job(0, started, gate)]);
        observed.recv().unwrap();
        cancelled.cancel();
        release.send(()).unwrap();
        let mut completed = scheduler.submit(vec![Box::new(|| Ok(1))]);
        let later = scheduler.submit(vec![Box::new(|| Ok(2))]);
        // The one worker must finish the cancelled root before it can complete this barrier.
        later.wait().unwrap();
        assert!(cancelled.try_result().is_none());
        completed.cancel();
        assert!(completed.try_result().is_none());
    }

    #[test]
    fn panicking_root_returns_an_error_and_the_worker_keeps_running() {
        let scheduler = Scheduler::new(1).unwrap();
        let panicking = scheduler.submit(vec![Box::new(|| -> Result<(), SimulationError> {
            panic!("injected planning panic");
        })]);
        let later = scheduler.submit(vec![Box::new(|| Ok(1))]);
        assert_eq!(
            panicking.wait().unwrap(),
            [Err(SimulationError::PlanningFailed)]
        );
        assert_eq!(later.wait().unwrap(), [Ok(1)]);
    }

    #[test]
    fn results_keep_goal_order_when_workers_finish_in_reverse_order() {
        let scheduler = Scheduler::new(2).unwrap();
        let (started, observed) = mpsc::channel();
        let (release, gate) = mpsc::channel();
        let (second_finished, observed_finish) = mpsc::channel();
        let request = scheduler.submit(vec![
            gated_job(0, started, gate),
            Box::new(move || {
                second_finished.send(()).unwrap();
                Ok(1)
            }),
        ]);
        observed.recv().unwrap();
        observed_finish.recv().unwrap();
        release.send(()).unwrap();
        assert_eq!(request.wait().unwrap(), [Ok(0), Ok(1)]);
    }
}
