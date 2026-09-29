# Learning Lord

A local, single-player town simulation for learning Rust. The current scope is a minimal scaffold; the game design is in [plans/ticking.md](plans/ticking.md).

## Structure

- `crates/simulation`: engine-independent world data using `imbl` persistent collections.
- `crates/simulation/src/planning.rs`: citizen prediction and execution of committed action batches.
- `crates/desktop`: a Bevy desktop executable that owns the current universe and displays its agent count.
- Prediction branches will remain simulation data, independent of rendering.
- The universe starts empty at simulation time zero. Agents have UUID v4 IDs, names, and a kind; citizens have hunger, a constant hourly hunger rate, an optional action in progress, and optional plan execution state.
- Cloning a universe shares collection storage. Changes to an owned clone leave the original and sibling snapshots unchanged.

## Simulation

- `Citizen::new(hunger)` starts an idle citizen with explicit finite hunger and the standard rate of 100 hunger per 24 simulation hours. `Citizen::with_hunger_rate(hunger, hunger_per_hour)` accepts a custom finite, nonnegative rate.
- Negative hunger represents satiation. Positive hunger represents a need for food. Neither side is clamped.
- `citizen.personal_wellbeing()` calculates `-max(hunger, 0) - 4 * max(hunger - 100, 0) - max(-hunger - 100, 0)` from current state. Higher is better: hunger from -100 to 0 scores zero; each unit below -100 costs one point for overfull discomfort. Positive hunger costs one point per unit up to 100, and each unit beyond 100 costs five. `agent.personal_wellbeing()` delegates to its kind. Both return `Result<f64, SimulationError>`, rejecting score overflow.
- `universe.with_citizen(name, citizen)` returns a new universe and the new agent's UUID. Existing IDs and the original universe remain unchanged.
- `citizen.start_action(CitizenAction::Eat)` or `CitizenAction::Wait` returns a new citizen busy for 30 simulation minutes. Starting an action does not advance time. Eating subtracts 50 hunger on completion; waiting has no completion effect. Two meals balance one day of standard hunger growth.
- `agent.start_action(action)` delegates to its kind. `universe.start_action(id, action)` returns a new snapshot with the specified agent's action started, preserving the clock and other agents. Starting another action while busy returns `CitizenBusy`; an unknown ID returns `AgentNotFound`.
- `citizen.advance(elapsed_ms)` returns a new citizen, advancing hunger by `hunger_per_hour * elapsed_ms / 3_600_000` and progressing its action. Hunger grows during eating and waiting. A completed action applies its effect at that point; unplanned citizens spend the remaining time idle, while planned citizens continue their batch. The original citizen is preserved, and no universe is required. Time is `u64` milliseconds; hunger calculations use `f64`.
- `agent.advance(elapsed_ms)` delegates to its kind and returns an updated agent. `universe.advance(elapsed_ms)` advances its clock once and delegates to each agent in a cloned universe.
- Clock overflow, nonfinite hunger results, or planning errors return a `SimulationError`, leaving the source universe unchanged. Small advances agree with an equivalent large advance within floating-point tolerance for the same action choices.
- Read state through `current_time_ms()`, `agents()`, and the citizen's `hunger()`, `hunger_per_hour()`, and `active_action()` getters. An active action exposes its `action()` and `remaining_ms()`.

```rust
use learning_lord_simulation::{Citizen, CitizenAction, SimulationError, Universe};

fn main() -> Result<(), SimulationError> {
    let (original, id) = Universe::default().with_citizen("Ada", Citizen::new(50.0)?);
    let eating = original.start_action(id, CitizenAction::Eat)?;
    let after_meal = eating.advance(30 * 60 * 1000)?;
    println!("Wellbeing: {}", after_meal.agents()[&id].personal_wellbeing()?);
    Ok(())
}
```

## Planning

- `planning::plan(&citizen)` explores eat/wait sequences until a completed action reaches or crosses four simulation hours (`HORIZON_MS`). It returns a variable-length `Plan` with `actions()` and `average_wellbeing()`, without changing the citizen or advancing the world. The current 30-minute actions produce eight-action sequences and 256 candidates.
- Plans are ranked by average wellbeing across their completed actions, including the action crossing the horizon. This does not measure wellbeing between completions. Any optimal tied plan is acceptable; the current search tries waiting before eating and keeps its first optimum.
- `citizen.start_planning()`, `agent.start_planning()`, or `universe.start_planning(id)` returns a new snapshot with planning enabled and the first action started, without advancing time. The citizen must be idle. Busy citizens return `CitizenBusy`; unknown IDs return `AgentNotFound`.
- Execution commits to two simulation hours (`COMMITMENT_MS`). `advance()` starts the next action immediately on completion, continuing the selected plan until elapsed time reaches that limit. If an action crosses the limit, it finishes before replanning. The executor then searches from the actual citizen state and starts a new batch, including when a single tick crosses several batches.
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

The window displays `Learning Lord` and `0 agents`; it does not create citizens or advance the simulation automatically. Close it with the window manager's close button or shortcut. Food inventory, relationships, and prediction worker threads are not implemented yet.

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
