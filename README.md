# Learning Lord

A local, single-player town simulation for learning Rust, with a desktop inspector for time and citizen behaviour. The game design is in [plans/ticking.md](plans/ticking.md).

## Structure

- `crates/simulation`: engine-independent world data using `imbl` persistent collections.
- `crates/simulation/src/planning.rs`: citizen prediction and execution of committed action batches.
- `crates/desktop`: a Bevy desktop executable with time controls, a simulation clock, and citizen readouts. A single simulation worker owns the current universe and publishes its latest completed snapshot to the UI.
- Prediction branches will remain simulation data, independent of rendering.
- The universe starts empty at simulation time zero. Agents have UUID v4 IDs, names, and a kind; citizens have hunger, a constant hourly hunger rate, tiredness, berry inventory in grams, a forage random generator, an optional action in progress, and an optional active plan.
- Cloning a universe shares collection storage. Changes to an owned clone leave the original and sibling snapshots unchanged.

## Simulation

- `Citizen::new(hunger)` starts an idle citizen with explicit finite hunger, tiredness zero, and the standard rate of 100 hunger per 24 simulation hours. `Citizen::with_hunger_rate(hunger, hunger_per_hour)` accepts a custom finite, nonnegative hunger rate and also starts tiredness at zero. `Citizen::with_needs(hunger, tiredness)` uses the standard hunger rate and explicit finite starting needs, clamping tiredness to a minimum of -100.
- Negative hunger represents satiation. Positive hunger represents a need for food. Neither side is clamped.
- Tiredness grows by 100 per 24 simulation hours, including while sleeping. Negative tiredness represents being rested; it has a minimum of -100 and no upper cap.
- `citizen.personal_wellbeing()` calculates `-max(hunger, 0) - 4 * max(hunger - 100, 0) - max(-hunger - 100, 0) - max(tiredness, 0)` from current state. Higher is better: hunger from -100 to 0 scores zero; each unit below -100 costs one point for overfull discomfort. Positive hunger costs one point per unit up to 100, and each unit beyond 100 costs five. Each positive unit of tiredness costs one point; negative tiredness gives no immediate bonus. `agent.personal_wellbeing()` delegates to its kind. Both return `Result<f64, SimulationError>`, rejecting score overflow.
- `universe.with_citizen(name, citizen)` returns a new universe and the new agent's UUID. Existing IDs and the original universe remain unchanged.
- `citizen.start_action(action)` returns a new citizen with an action started without advancing time. Wait and Forage take 30 simulation minutes; Sleep takes eight hours. Sleep gradually removes 100 tiredness, subject to the -100 floor. Waiting supplies no recovery. `citizen.action_duration_ms(action)` gives the duration using current inventory; `active_action.duration_ms()` gives the duration fixed when the action started.
- All citizens start with zero berries. `citizen.berries_grams()` reads the inventory; `citizen.with_berries(grams)` creates an idle snapshot with an explicit finite, nonnegative quantity for setup. Inventory replacement while an action or plan is active returns `CitizenBusy`.
- Berries provide 0.5 nutrition per gram and take one second per gram to eat. Eat chooses up to 100 grams (50 nutrition) from the actual inventory at action start. Smaller meals consume whatever is available; empty meals are skipped. Positive durations round up to the nearest millisecond. The selected portion is consumed gradually alongside its hunger reduction. A 100-gram meal lasts 100 seconds; halfway through it, 50 grams have been consumed and 25 hunger removed, alongside normal need growth.
- Forage adds a uniform random 5–15 grams of berries only on completion (10 grams on average). Predictions use exactly 10 grams. Each citizen snapshot owns its random generator state, so prediction consumes no randomness and advancing independent clones cannot interfere with one another. Splitting an advance preserves actual forage outcomes; repeated advancement from the same snapshot reproduces them.
- `agent.start_action(action)` delegates to its kind. `universe.start_action(id, action)` returns a new snapshot with the specified agent's action started, preserving the clock and other agents. Starting another action while busy returns `CitizenBusy`; an unknown ID returns `AgentNotFound`.
- `citizen.advance(elapsed_ms)` returns a new citizen, growing both needs and applying the active action's recovery proportionally to time spent in it. Half a full 100-gram meal removes 25 hunger, alongside normal hunger growth. Actions grant no extra recovery on completion. Unplanned citizens spend any time after completion idle, while planned citizens continue their batch. The original citizen is preserved, and no universe is required. Time is `u64` milliseconds; need calculations use `f64`.
- `agent.advance(elapsed_ms)` delegates to its kind and returns an updated agent. `universe.advance(elapsed_ms)` advances its clock once and delegates to each agent in a cloned universe.
- Clock overflow, nonfinite need results, or planning errors return a `SimulationError`, leaving the source universe unchanged. Small advances agree with an equivalent large advance within floating-point tolerance for the same action choices.
- Read state through `current_time_ms()`, `agents()`, and the citizen's `hunger()`, `hunger_per_hour()`, `tiredness()`, `berries_grams()`, and `active_action()` getters. An active action exposes its `action()`, `duration_ms()`, and `remaining_ms()`.

```rust
use learning_lord_simulation::{Citizen, CitizenAction, SimulationError, Universe};

fn main() -> Result<(), SimulationError> {
    let citizen = Citizen::new(50.0)?.with_berries(100.0)?;
    let (original, id) = Universe::default().with_citizen("Ada", citizen);
    let eating = original.start_action(id, CitizenAction::Eat)?;
    let after_meal = eating.advance(100_000)?;
    println!("Wellbeing: {}", after_meal.agents()[&id].personal_wellbeing()?);
    Ok(())
}
```

## Planning

- `planning::plan(&citizen)` explores Eat, Wait, Sleep, and Forage sequences until a completed action reaches or crosses four simulation hours (`HORIZON_MS`). It returns a variable-length `Plan` with `actions()` and `average_wellbeing()`, without changing the citizen or advancing the world. Inventory and mean forage yields determine predicted meal durations. Empty meals and later meals below the replan threshold are excluded. Sleep is simulated to completion even across the horizon, so predictions can extend beyond four hours.
- Within a prediction, Eat becomes eligible again four simulation hours after its previous completion (`EAT_PLAN_COOLDOWN_MS`). Every new plan starts with Eat eligible, regardless of actual recent meals. This permits at most one meal in the current four-hour horizon while allowing repeated meals in longer predictions. The cooldown exists only during prediction; it is not a persistent citizen restriction.
- Plans are ranked by average wellbeing across their completed actions, including the action crossing the horizon. This does not measure wellbeing between completions or weight scores by action duration. Search exhaustively considers the permitted sequences, trying Wait, Eat, Sleep, then Forage. Any optimal tied plan is acceptable. Extra berries have no direct wellbeing value unless used within the prediction, so foraging can tie with waiting.
- `citizen.start_planning()`, `agent.start_planning()`, or `universe.start_planning(id)` returns a new snapshot with planning enabled and the first action started, without advancing time. The citizen must be idle. Busy citizens return `CitizenBusy`; unknown IDs return `AgentNotFound`.
- Execution commits to two simulation hours (`COMMITMENT_MS`). `advance()` starts the next action immediately on completion, continuing the selected plan until elapsed time reaches that limit. If an action crosses the limit, it finishes before replanning, including an eight-hour sleep. The executor then searches from the actual citizen state and starts a new batch, including when a single tick crosses several batches. Interruption is not implemented.
- Before a non-first Eat, the **replan check** compares actual available nutrition to 20 (`REPLAN_MIN_NUTRITION`). Below 20 means discard the remaining plan and replan from actual state; exactly 20 is enough to continue. The first action is exempt, allowing a newly chosen small meal without a replan loop. Otherwise Eat adapts to actual inventory and execution continues without reassessing after every forage. A meal already in progress is never interrupted by this check. If changed durations exhaust the plan before the commitment, replan rather than indexing past its end.
- `citizen.active_plan()` returns the stored `ActivePlan`, exposing its chosen `plan()`, zero-based `action_index()`, and `elapsed_ms()` since the current batch began, including partial actions. The active action holds its remaining time. Manual actions and restarting planning are rejected while a batch is executing.
- Prediction only advances citizen snapshots. During actual universe advancement, every agent advances normally. Search is synchronous and propagates numeric errors; failed planning or execution leaves the source snapshot unchanged.

```rust
use learning_lord_simulation::{Citizen, SimulationError, Universe};

fn main() -> Result<(), SimulationError> {
    let (original, id) = Universe::default().with_citizen("Ada", Citizen::new(50.0)?);
    let planning = original.start_planning(id)?;
    let after_two_hours = planning.advance(2 * 60 * 60 * 1000)?;
    println!("Wellbeing: {}", after_two_hours.agents()[&id].personal_wellbeing()?);
    Ok(())
}
```

## Run

The initial target is Linux with a Wayland or X11 desktop session and a working Vulkan graphics driver. Building requires a native C/C++ toolchain, `pkg-config`, and development libraries for Wayland and XKB. Bevy's [Linux dependencies guide](https://github.com/bevyengine/bevy/blob/v0.19.1/docs/linux_dependencies.md) lists distribution-specific packages.

```sh
cargo run --locked
```

The desktop starts paused with Ada at hunger and tiredness zero, no berries, and planning enabled. The clock shows elapsed days, hours, minutes, and seconds, starting at `Day 0 | 00:00:00`. Citizen readouts show berry grams, hunger, tiredness, wellbeing, the current action and its remaining hours/minutes/seconds, the rest of the planned actions in order, and remaining commitment time. Normal replanning waits for action completion; insufficient food can trigger an earlier replan. Future meal durations depend on actual forage outcomes, so the commitment countdown is not an exact forecast of replanning time. Actions beyond the commitment are forecasts and may change when replanning occurs.

- **Run / Pause** (or **Space**) controls automatic advancement.
- **1x, 2x, 3x, 5x, 10x, 20x** select simulation speed without unpausing.
- **Advance 30 minutes** (or **Right arrow**) advances exactly 1,800,000 simulation milliseconds while paused, independent of speed. The button is unavailable while running.
- At **1x**, one real second advances the universe by **60,000 ms**. Automatic update starts are capped at 60 per real second by default. The worker measures elapsed monotonic time, including processing delays, and increases subsequent time jumps when work takes longer. It retains fractional milliseconds and does not queue fixed catch-up ticks.
- Pausing lets an in-progress update finish. Paused time does not accumulate; unapplied time accrued while running is retained for the next automatic update after resuming. Simulation errors pause advancement and appear in the window.

To use a different maximum automatic update rate, set a positive integer at launch:

```sh
LEARNING_LORD_MAX_UPDATES_PER_SECOND=30 cargo run --locked
```

This changes update frequency, not simulation speed. Rendering and input remain on the UI thread; at most one simulation advance runs at a time. Close the window normally to stop the worker after its in-progress update. Relationships, trading, and parallel prediction workers are not implemented yet.

## Development

Use Rust 1.98.1 with rustfmt and Clippy, as specified in `rust-toolchain.toml`.

```sh
cargo test --workspace --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo build --workspace --locked
```

Run just the simulation checks without compiling the frontend:

```sh
cargo test -p learning-lord-simulation --locked
```
