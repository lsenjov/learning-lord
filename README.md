# Learning Lord

A local, single-player town simulation for learning Rust, with a desktop inspector for time and citizen behaviour. The game design is in [plans/ticking.md](plans/ticking.md).

## Structure

- `crates/simulation`: engine-independent world data using `imbl` persistent collections.
- `crates/simulation/src/planning.rs`: citizen prediction and execution of committed action batches.
- `crates/desktop`: a Bevy desktop executable with time controls, a simulation clock, and citizen readouts. A single simulation worker owns the current universe and publishes its latest completed snapshot to the UI.
- Prediction branches will remain simulation data, independent of rendering.
- The universe starts empty at simulation time zero. Agents have UUID v4 IDs, names, and a kind; citizens have hunger, a constant hourly hunger rate, tiredness, berry and pebble inventories in grams, coins, a gathering random generator, an optional action in progress, and an optional active plan.
- Cloning a universe shares collection storage. Changes to an owned clone leave the original and sibling snapshots unchanged.

## Simulation

- `Citizen::new(hunger)` starts an idle citizen with explicit finite hunger, tiredness zero, and the standard rate of 100 hunger per 24 simulation hours. `Citizen::with_hunger_rate(hunger, hunger_per_hour)` accepts a custom finite, nonnegative hunger rate and also starts tiredness at zero. `Citizen::with_needs(hunger, tiredness)` uses the standard hunger rate and explicit finite starting needs, clamping tiredness to a minimum of -100.
- Negative hunger represents satiation. Positive hunger represents a need for food. Neither side is clamped.
- Tiredness grows by 100 per 24 simulation hours, including while sleeping. Negative tiredness represents being rested; it has a minimum of -100 and no upper cap.
- `citizen.personal_wellbeing()` calculates `-max(hunger, 0) - 4 * max(hunger - 100, 0) - max(-hunger - 100, 0) - max(tiredness, 0) + 10 * wealth` from current state. Higher is better: hunger from -100 to 0 scores zero; each unit below -100 costs one point for overfull discomfort. Positive hunger costs one point per unit up to 100, and each unit beyond 100 costs five. Each positive unit of tiredness costs one point; negative tiredness gives no immediate bonus. `agent.personal_wellbeing()` delegates to its kind. Both return `Result<f64, SimulationError>`, rejecting score overflow.
- `universe.with_citizen(name, citizen)` returns a new universe and the new agent's UUID. Existing IDs and the original universe remain unchanged.
- `citizen.start_action(action)` returns a new citizen with an action started without advancing time. Wait, Forage, and Find rocks take 30 simulation minutes; Sleep takes eight hours. Sleep gradually removes 100 tiredness, subject to the -100 floor. Waiting supplies no recovery. `citizen.action_duration_ms(action)` gives the duration using current inventory; `active_action.duration_ms()` gives the duration fixed when the action started.
- Citizens start with zero coins and pebbles as well as zero berries. Coins and weights use `f64`; coins may be negative for accounting. The marketplace independently randomizes berry prices uniformly over 1–2 coins/kg and pebble prices over 1–4 coins/kg at universe creation and at 04:00 each simulation day. `citizen.wealth()` adds coins and the current market value of both inventories; each coin of wealth adds 10 wellbeing. `with_coins` and `with_pebbles` provide idle-snapshot setup, with finite coins and finite nonnegative pebble grams. The marketplace has unlimited stock and coins.
- Find rocks takes 30 minutes and adds uniform random 2.5–7.5 grams of pebbles on completion; predictions use 5 grams. It shares the citizen-owned gathering random stream with Forage.
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

- `planning::plan(&citizen)` searches hunger, tiredness, and wealth goals. Actions expose `effects()` and `input_for(...)`; the planner recursively obtains missing berries, coins, or pebbles through their suppliers. Gathering uses average yields, while trades use the same prices and quantities as execution.
- A hunger goal compares 30- and 50-nutrition acquisition targets (60 g and 100 g of berries). These are acquisition thresholds, not portion limits: Eat still consumes available berries up to 50 nutrition, and Buy berries still tops up to 100 g when affordable. Existing smaller meals are also considered, subject to the existing food replan check. The planner can discover foraging, buying, selling and gathering prerequisite chains, including mixed supply paths, without predefined recipes.
- Each goal's backward search allows up to four hours of preparation (`goals::GOAL_HORIZON_MS`), including resource acquisition and travel. The final activity is excluded from that budget and finishes in full, so walking home followed by eight hours of sleep is valid.
- Alternatives are simulated from the predicted state at the goal's start. Keep the variant with the highest duration-weighted minute-sampled wellbeing, then explore the other goals from that variant's resulting state. This deliberately chooses a locally best variant, not a globally optimal primitive sequence.
- Start another goal while the combined plan is under four hours (`HORIZON_MS`), then simulate the entire final goal even if it crosses the horizon. Overall scores use wellbeing sampled every simulation minute, weighted by represented time. The schedule continues across action and goal boundaries; a final partial minute uses its endpoint wellbeing and proportional weight. Completion effects at a sample time are included.
- Future tuning: revisit minute-sampling accuracy and planning performance. Endpoint sampling can attribute completion benefits up to one minute early.
- Prediction-local cooldowns begin at completion: Eat waits four hours, Buy berries and Sell pebbles each wait two hours independently. Cooldowns carry across goals in the same prediction and reset for each new plan. They do not restrict manually started actions or carry across replanning.
- `Plan` exposes `actions()` and `average_wellbeing()`. Execution uses the flattened primitive sequence. `citizen.start_planning()` starts the first action and stores the plan in `ActivePlan`; busy citizens reject replacement. Execution commits for two hours (`COMMITMENT_MS`), finishing the current primitive action before replanning, even if a compound goal is unfinished.
- Before a non-first Eat, actual food below 20 nutrition triggers replanning; the first action is exempt. Empty meals and trades are skipped, and exhausting a sequence starts a fresh plan. Real gathering outcomes determine actual meal and trade quantities.
- `ActivePlan` exposes its `plan()`, zero-based `action_index()`, and `elapsed_ms()`. Forecast actions beyond the commitment may change during replanning. Predictions leave the original citizen and its random stream unchanged.

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

The desktop starts paused with Ada at hunger and tiredness zero, no berries, and planning enabled. The clock shows elapsed days, hours, minutes, and seconds, starting at `Day 0 | 00:00:00`. Citizen readouts show berry and pebble grams, coins, total wealth, hunger, tiredness, wellbeing, the current action and its remaining hours/minutes/seconds, the rest of the planned actions in order, and remaining commitment time. Normal replanning waits for action completion; insufficient food can trigger an earlier replan. Future meal durations depend on actual forage outcomes, so the commitment countdown is not an exact forecast of replanning time. Actions beyond the commitment are forecasts and may change when replanning occurs.

- **Restart universe** creates a fresh paused universe at 1x with a new Ada, randomized map and prices, clearing errors and accumulated pacing. Current prices are displayed above the controls.
- **Run / Pause** (or **Space**) controls automatic advancement.
- **1x, 2x, 3x, 5x, 10x, 20x** select simulation speed without unpausing.
- **Advance 30 minutes** (or **Right arrow**) advances exactly 1,800,000 simulation milliseconds while paused, independent of speed. The button is unavailable while running.
- At **1x**, one real second advances the universe by **60,000 ms**. Automatic update starts are capped at 60 per real second by default. The worker measures elapsed monotonic time, including processing delays, and increases subsequent time jumps when work takes longer. It retains fractional milliseconds and does not queue fixed catch-up ticks.
- Pausing lets an in-progress update finish. Paused time does not accumulate; unapplied time accrued while running is retained for the next automatic update after resuming. Simulation errors pause advancement and appear in the window.

To use a different maximum automatic update rate, set a positive integer at launch:

```sh
LEARNING_LORD_MAX_UPDATES_PER_SECOND=30 cargo run --locked
```

This changes update frequency, not simulation speed. Rendering and input remain on the UI thread; at most one simulation advance runs at a time. Close the window normally to stop the worker after its in-progress update. Relationships and parallel prediction workers are not implemented yet.

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

## Trading

- Buy berries and Sell pebbles each take five simulation minutes. Needs continue growing.
- Sell pebbles sells all held pebbles at the current price. Buy berries tops up to 100 grams at the current price, buying less if funds are insufficient and never borrowing.
- Quantities and payment are fixed from inventory and coins at action start. Goods and coins transfer only on completion, preserving total market-valued wealth at the quoted prices apart from floating-point rounding. Market value may change while a trade is underway.
- Predictions use identical trade rules. Empty trades are skipped without consuming time, including during plan execution. Actual quantities may differ after random gathering; the existing low-food replan check still applies before later meals.

## Market updates

- The universe owns the market seed and prices, synchronizing the price context on citizen snapshots. Standalone citizens use `Prices::default()` (1 and 2 coins/kg) unless given `with_prices(...)`; inserting one into a universe uses that universe’s prices. `Universe::with_prices(...)` allows explicit setup prices until the next scheduled update.
- Split universe advancement at each 04:00 boundary. Active trades retain their quote; actions starting exactly at the boundary use the new prices. Revalue inventories immediately, but keep existing plans until normal replanning. Predictions assume current prices remain stable throughout the forecast.
- Market draws are addressed by seed and update period, preserving independent branches and tick partitioning. Empty universes can jump directly to the final applicable price update.

## Last planning decision

- The desktop shows the best starting Hunger, Sleep and Wealth sequences from the last replan, their duration and average minute-sampled wellbeing. Repeated actions are grouped for readability.
- Goal avg covers only the displayed goal sequence. Plan avg covers the best complete plan starting with that sequence; this determines the chosen first goal. An available but unchosen goal is distinct from an unavailable goal.
- Unavailable means no executable sequence was found within the four-hour prerequisite limit and action rules; the report does not attribute failure to a particular resource or cooldown.
- `Plan::decision()` exposes the saved candidates, selected starting goal and prices used. The report is captured during the existing search, shared across snapshot clones, and replaced on replanning. Price changes do not rewrite past decisions. No additional planning is performed for the display.

## Locations and travel

- Each universe generates a 2 km × 2 km map: house at the centre, with distinct forest, river and market sites. Ada starts at home. The desktop map shows the sites, Ada and the remaining current route.
- Sleep requires the house, Forage the forest, Find rocks the river, and trading the market. Eat and Wait are available anywhere. Starting an activity at the wrong site returns `WrongLocation`.
- The planner inserts `Travel(Location)` before an activity when needed; travel is not a standalone goal. Each trip begins at the preceding predicted position, so consecutive activities at the same site share the journey. Travel contributes to elapsed time, cooldowns and minute-sampled wellbeing scores.
- Walking follows a straight line at 1 km per 10 minutes. Position changes continuously, needs continue growing, and arrival lands exactly at the destination. Positive durations round up to milliseconds. Replanning waits for the current trip to finish when it crosses the two-hour commitment.
- Positions use metres as `f64`. `universe.map()`, `citizen.map()` and `citizen.position()` expose spatial state. Standalone citizens use `Map::default()`, with all sites at the origin, for nonspatial fixtures. `Map::new(...)`, `Citizen::with_map(...)`, `Citizen::with_position(...)` and `Universe::with_map(...)` support explicit setup.
- Inserting a citizen into a different map places them at its house and clears actions and plans tied to the old map. Insertion into the same map preserves their position and activity. Universe clones preserve their map.
