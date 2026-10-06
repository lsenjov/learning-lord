# Learning Lord

A local, single-player town simulation for learning Rust, with a desktop inspector for time and citizen behaviour. The game design is in [plans/ticking.md](plans/ticking.md).

## Structure

- `crates/simulation`: engine-independent world data using `imbl` persistent collections.
- `crates/simulation/src/planning.rs`: citizen prediction and execution of committed action batches.
- `crates/desktop`: a Bevy desktop executable with time controls, a simulation clock, and citizen readouts. A single simulation worker owns the current universe and publishes its latest completed snapshot to the UI.
- Prediction branches will remain simulation data, independent of rendering.
- The universe starts empty at simulation time zero. Agents have UUID v4 IDs, names, and a kind; citizens have hunger, a constant hourly hunger rate, tiredness, a carried inventory of berries, wheat, flour, wood, water, bread and berry pie in `f64` grams, coins, a gathering random generator, an optional action in progress, and an optional active plan.
- Cloning a universe shares collection storage. Changes to an owned clone leave the original and sibling snapshots unchanged.

## Simulation

- `Citizen::new(hunger)` starts an idle citizen with explicit finite hunger, tiredness zero, and the standard rate of 100 hunger per 24 simulation hours. `Citizen::with_hunger_rate(hunger, hunger_per_hour)` accepts a custom finite, nonnegative hunger rate and also starts tiredness at zero. `Citizen::with_needs(hunger, tiredness)` uses the standard hunger rate and explicit finite starting needs, clamping tiredness to a minimum of -100.
- Negative hunger represents satiation. Positive hunger represents a need for food. Neither side is clamped.
- Tiredness grows by 100 per 24 simulation hours, including while sleeping. Negative tiredness represents being rested; it has a minimum of -100 and no upper cap.
- `citizen.personal_wellbeing()` calculates `-max(hunger, 0) - 4 * max(hunger - 100, 0) - max(-hunger - 100, 0) - max(tiredness, 0) + 10 * wealth + food_reserve_bonus` from current state. Higher is better: hunger from -100 to 0 scores zero; each unit below -100 costs one point for overfull discomfort. Positive hunger costs one point per unit up to 100, and each unit beyond 100 costs five. Each positive unit of tiredness costs one point; negative tiredness gives no immediate bonus. `agent.personal_wellbeing()` delegates to its kind. Both return `Result<f64, SimulationError>`, rejecting score overflow.
- `universe.with_citizen(name, citizen)` returns `Result<(Universe, AgentId), SimulationError>`. Citizens receive a UUID v4 at construction; insertion preserves it and rejects an already-present ID with `AgentAlreadyExists`. Snapshot clones preserve citizen and place IDs.
- `citizen.start_action(action)` returns a new citizen with an action started without advancing time. Wait and Forage take 30 simulation minutes; Sleep takes eight hours. Sleep gradually removes 100 tiredness, subject to the -100 floor. Waiting supplies no recovery. `citizen.action_duration_ms(action)` gives the duration using current inventory; `active_action.duration_ms()` gives the duration fixed when the action started.
- Standalone citizens start with empty inventory and zero coins; desktop citizens start with 310 g berries (100 nutrition). Every desktop citizen receives 3 coins (1 base coin plus 2 coins for purchases), and the miller and baker also receive four hours of recipe inputs. Coins and weights use `f64`; coins may be negative for accounting. Every catalogue good has a fixed initial reference price. At 04:00, observed market supply and affordable unmet demand adjust active prices by at most 10%; inactive prices stay unchanged. Sell orders hold finite individually owned stock with asking prices updated to the reference price at 04:00. `citizen.wealth()` includes coins, carried goods and owned listed goods valued at current reference prices; property is excluded. Each coin of wealth adds 10 wellbeing. Carried edible food adds 0.10 wellbeing per nutrition up to 100, then 0.02 per nutrition up to 300 (maximum 14). Listed food adds no reserve bonus.
- `Citizen::new` starts with zero berries; the desktop supplies each citizen with 310 g (100 nutrition). `citizen.berries_grams()` reads the inventory; `citizen.with_berries(grams)` creates an idle snapshot with an explicit finite, nonnegative quantity for setup. Inventory replacement while an action or plan is active returns `CitizenBusy`.
- Berries provide 100/310 nutrition per gram; bread provides 0.5 and berry pie 0.6. All food takes one second per gram to eat. Eat combines carried foods in catalogue order up to 50 nutrition; a berry-only meal uses up to 155 g and takes 155 seconds. Food is consumed gradually alongside its hunger reduction. Smaller meals use available food; empty meals are skipped. Positive durations round up to milliseconds.
- Forage adds a uniform random 5–15 grams of berries only on completion (10 grams on average). Predictions use exactly 10 grams. Each citizen snapshot owns its random generator state, so prediction consumes no randomness and advancing independent clones cannot interfere with one another. Splitting an advance preserves actual forage outcomes; repeated advancement from the same snapshot reproduces them.
- `agent.start_action(action)` delegates to its kind. `universe.start_action(id, action)` returns a new snapshot with the specified agent's action started, preserving the clock and other agents. Starting another action while busy returns `CitizenBusy`; an unknown ID returns `AgentNotFound`.
- `citizen.advance(elapsed_ms)` returns a new citizen, growing both needs and applying the active action's recovery proportionally to time spent in it. Half a full 155-gram berry meal removes 25 hunger, alongside normal hunger growth. Actions grant no extra recovery on completion. Unplanned citizens spend any time after completion idle, while planned citizens continue their batch. The original citizen is preserved, and no universe is required. Time is `u64` milliseconds; need calculations use `f64`.
- `agent.advance(elapsed_ms)` delegates to its kind and returns an updated agent. `universe.advance(elapsed_ms)` advances its clock once and delegates to each agent in a cloned universe.
- Clock overflow, nonfinite need results, or planning errors return a `SimulationError`, leaving the source universe unchanged. Small advances agree with an equivalent large advance within floating-point tolerance for the same action choices.
- Read state through `current_time_ms()`, `agents()`, and the citizen's `hunger()`, `hunger_per_hour()`, `tiredness()`, `berries_grams()`, and `active_action()` getters. An active action exposes its `action()`, `duration_ms()`, and `remaining_ms()`.

```rust
use learning_lord_simulation::{Citizen, CitizenAction, SimulationError, Universe};

fn main() -> Result<(), SimulationError> {
    let citizen = Citizen::new(50.0)?.with_berries(100.0)?;
    let (original, id) = Universe::default().with_citizen("Ada", citizen)?;
    let eating = original.start_action(id, CitizenAction::Eat)?;
    let after_meal = eating.advance(100_000)?;
    println!("Wellbeing: {}", after_meal.agents()[&id].personal_wellbeing()?);
    Ok(())
}
```

## Planning

- `planning::plan(&citizen)` searches hunger, tiredness, wealth, food reserve replenishment and configured production goals, with a standing excess-listing policy. Actions expose `effects()` and `input_for(...)`; the planner recursively obtains missing food and production inputs through gathering, purchases, owned-order withdrawals and available production. Gathering uses average yields, while trades use the same prices and quantities as execution.
- A hunger goal acquires 50 nutrition (155 g of berries). Eat consumes available food up to 50 nutrition, and immediate partial meals remain candidates subject to the existing food replan check. Buying tops up to 50 nutrition when affordable. The planner discovers foraging, purchases and mixed food supply paths.
- Replenish reserves compares carried food targets of 100, 200 and 300 nutrition using the existing acquisition and travel paths. Exact purchase baskets preserve the selected target during execution. It competes through complete-plan scores without forced priority. Excess listings retain up to 300 nutrition in addition to production inputs. Food demand comes from the executing purchase or its immediately preceding travel within the selected goal; discarded forecasts and future goals create no food requests. Production input shortages remain standing demand.
- Each goal's backward search allows up to four hours of preparation (`goals::GOAL_HORIZON_MS`), including resource acquisition and travel. The final activity is excluded from that budget and finishes in full, so walking home followed by eight hours of sleep is valid.
- Alternatives are simulated from the predicted state at the goal's start. Retain every executable variant and explore subsequent goals from each resulting state. Choose the highest duration-weighted action-endpoint wellbeing across complete plans in this goal-variant search space.
- Start another goal while the combined plan is under four hours (`HORIZON_MS`), then simulate the entire final goal even if it crosses the horizon. Each action contributes the average of its starting and ending wellbeing, weighted by its duration. Goal and full-plan scores divide the sum of these contributions by total duration. Ending wellbeing includes completion effects, so gathering's wealth benefit is effectively spread across its action for scoring; usable inventory still arrives at completion.
- Gathering candidates are orders of 1 through `floor(HORIZON_MS / base action duration)` primitive segments: currently 1–8 half-hour segments, or 1–4 if the base duration becomes one hour. The planner compares these lengths for wealth goals and resource suppliers. Each order travels to its site once when needed. Adjacent orders of the same gathering family are forbidden across goal and nested resource boundaries; repeated primitive segments inside one order are allowed. Another gathering family or a non-gathering activity permits that family again; automatic travel alone does not. This restriction resets at replanning.
- Orders execute as the existing half-hour primitive actions, with goods arriving at each segment completion from an independent random draw. Prediction uses the existing average yield and endpoint score for every segment.
- Each top-level goal can appear at most once within a planning window. This resets at replanning; repeated primitive actions within a goal and recursive resource acquisition remain allowed. Production compares every feasible prefix of a queue ordered by current profit per production time, advancing to the next recipe when its remaining target is exhausted or cannot be supplied. Include the first prefix reaching four hours, including travel and input preparation, then stop extending that path.
- Production prefixes prepare their combined input requirements before the first batch, buying only enough for that candidate in one basket. Repeated food production for reserve targets uses the same preparation and accounts for edible ingredients consumed. Every batch remains a separate action with its own skill gain, duration and input consumption; the two-hour purchase cooldown still applies.
- Exact primitive-prefix continuations and the set of used goals are cached within each planning call, while first-goal forecasts retain their own labels. Development builds optimize the simulation crate at level 2. Richer random town states can still produce occasional multi-second plans.
- Future tuning: revisit action-endpoint scoring accuracy and planning performance. The approximation assumes wellbeing changes linearly within each action, including completion rewards; needs can cross thresholds and goods actually arrive at completion. Later review should also examine whether averaging over different durations favors shorter sequences that omit later low wellbeing.
- Prediction-local cooldowns begin at completion: Eat waits four hours, purchases and listings each wait two hours independently. Cooldowns carry across goals in the same prediction and reset for each new plan. They do not restrict manually started actions or carry across replanning.
- `Plan` exposes `actions()`, predicted `action_durations_ms()`, `goals()` with contiguous action ranges, and `average_wellbeing()`. Goal boundaries preserve the actual search choices, including their prerequisite actions; they do not change scoring. Execution uses the flattened primitive sequence. `citizen.start_planning()` starts the first action and stores the plan in `ActivePlan`; busy citizens reject replacement. Execution commits for two hours (`COMMITMENT_MS`), finishing the current primitive action before replanning, even if a compound goal is unfinished.
- Before a non-first Eat, actual food below 20 nutrition triggers replanning; the first action is exempt. Empty meals and trades are skipped, and exhausting a sequence starts a fresh plan. Real gathering outcomes determine actual meal and trade quantities.
- `ActivePlan` exposes its `plan()`, zero-based `action_index()`, and `elapsed_ms()`. Forecast actions beyond the commitment may change during replanning. Predictions leave the original citizen and its random stream unchanged.

```rust
use learning_lord_simulation::{Citizen, SimulationError, Universe};

fn main() -> Result<(), SimulationError> {
    let (original, id) = Universe::default().with_citizen("Ada", Citizen::new(50.0)?)?;
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

The desktop starts paused with Ada the farmer, Bram the miller, Cleo the woodcutter and Dara the baker, each at hunger and tiredness zero with 310 g berries (100 nutrition) and planning enabled. Each receives 3 coins (1 base coin plus 2 coins for purchases); Bram and Dara also receive four hours of recipe inputs. Berries initially cost 0.05 coins/kg. Roles are starting configuration metadata and do not restrict actions. Each owns a private home; Ada also owns a field, Bram a mill and Dara a bakery. Forest, river and market are public. Production recipes use each profession’s skills and property; bread and berry pie supplement berries as food. Four cards, sorted by name, show coins, wealth, wellbeing and the current action with its remaining time. Hunger and sleep use signed bars centered at zero, with qualitative labels for satiation/rest, rising needs and urgency above 100. Overfull hunger below -100 is flagged separately. Bars clamp at either end; exact needs remain available in the detail panel.

Click a card or press **1-4** to select a citizen. Selection follows their ID across snapshots; restarting selects the new Ada. The map shows everyone, highlights the selected citizen and displays their travel route. Nearby citizen labels are offset to remain distinct. The scrollable detail panel shows inventory, exact needs, location, commitment, saved planning scores and the entire active plan grouped by the goals actually chosen by the search. Completed actions remain dimmed, the current action is marked **NOW**, and future actions retain predicted durations. Normal replanning waits for action completion; insufficient food can trigger an earlier replan. Future meal durations depend on actual gathering outcomes, so the commitment countdown is not an exact forecast of replanning time. Actions beyond the commitment may change when replanning occurs.

Switch between **Citizens** and **Market** with the tabs or **C/M**. Market lists all seven goods with current prices; click a good or use **Up/Down** to inspect its quantities and current sell orders grouped by seller. **Current market day** starts at 04:00 (the initial period starts at time zero); **Previous market day** shows the latest closed period, including its closing stock and affordable unfulfilled demand. Buy volume is traded quantity plus affordable unfulfilled demand; sell volume is traded quantity plus listed stock. Unfulfilled demand can exist while stock is available. Sell orders and the current price remain live in either period view. Restart preserves the selected good and returns metrics to the current period.

- **Restart universe** creates four fresh citizens in a paused universe at 1x with a randomized map and fixed initial reference prices, clearing errors and accumulated pacing. Current prices are listed in the Market tab.
- **Export universe** writes the displayed snapshot as readable Rust debug text to `debug/universe-<Unix seconds>.<nanoseconds>.txt`, relative to the launch directory. The timestamp is captured when the click is handled, in real-world UTC time. Exports include world, citizen, action, plan and decision data; they are diagnostic dumps, not reloadable saves. The app shows the resulting path or an export error, without changing simulation state. Export remains available while running or after a simulation error.
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

- Listing, purchasing and withdrawing goods take five simulation minutes at the market. Needs continue growing. A shopping list may buy several goods in one action.
- Sell orders hold finite stock owned by their sellers. Partial fills transfer stock and coins at completion, using live asking prices and the buyer's available funds. Citizens cannot buy from themselves or borrow. Withdrawals retrieve only the citizen's own listed goods.
- Settlement handles changed availability; competing completions use chronological order. Inventory and coins remain conserved apart from floating-point rounding. Requests too small to decrement an order transfer nothing and leave funds and history untouched.
- Predictions clone the market, use the same settlement rules and assume stable prices. They do not reserve actual stock, record actual demand or guarantee future sales. Execution replans when shortages prevent an action.

## Market updates

- The universe owns market prices and synchronizes the price context on citizen snapshots. Initial reference prices in coins/kg are berries 0.05, wheat 0.4, flour 1, wood 0.5, water 0.1, bread 1.5 and berry pie 2. Standalone citizens use `Prices::default()` unless given `with_prices(...)`; insertion uses the universe's prices.
- At 04:00, supply is traded grams plus remaining listed grams; demand is traded grams plus affordable unfilled purchase requests. Active prices change by at most 10% toward the supply/demand balance, with a small positive numerical floor; inactive prices stay unchanged. Requests replace earlier intentions rather than accumulate repeated attempts.
- Completions exactly at 04:00 settle into the closing interval before prices update. Remaining standing orders are repriced to the updated reference prices; newly completed listings use current reference prices. Purchases already underway settle against live orders at completion. Inventories are revalued immediately, while plans remain until normal replanning. Predictions retain their starting reference prices.

## Daily production targets

- For each profitable recipe output, the stock target is the greater of estimated daily sales plus unmet demand and twelve hours of output. The minimum uses whole batches at the citizen's current skill, even on days with little demand.
- Carried and listed saleable stock and output already underway count toward that target. Personal food reserves remain separate. Recipes compete for one twelve-hour daily work budget in profit-per-hour order, subject to available inputs, funds, skills and property.

## Last planning decision

- The desktop shows the starting Hunger, Sleep, Wealth, Replenish reserves, Production and List excess candidates from the last replan, their duration and average action-endpoint wellbeing. Repeated actions are grouped for readability.
- Each displayed sequence leads to the best complete plan for that first goal. Goal avg covers only that sequence and is informational. Plan avg covers its best complete continuation; this determines the chosen first goal. An available but unchosen goal is distinct from an unavailable goal.
- Unavailable means no executable sequence was found within the four-hour prerequisite limit and action rules; the report does not attribute failure to a particular resource or cooldown.
- `Plan::decision()` exposes the saved candidates, selected starting goal and prices used. The report is captured during the existing search, shared across snapshot clones, and replaced on replanning. Price changes do not rewrite past decisions. No additional planning is performed for the display.

## Locations and travel

- The desktop generates ten distinct sites within the 2 km × 2 km map, separated by at least 200 m: four private homes, Ada's field, Bram's mill, Dara's bakery and a public forest, river and market. Every place has its own UUID v4, type, position, name and optional owner. Citizens start at their own homes. The map shows all sites and citizens, selected ownership and the current route; labels show public sites and the selected citizen's properties. Selected details show all inventory, owned properties, location and the full plan.
- Sleep requires the citizen's own home, Forage the forest, Fetch water the river, and trading the market. Eat and Wait are available anywhere. Starting an activity at the wrong site returns `WrongLocation`.
- The planner inserts `Travel(PlaceId)` before an activity when needed; travel is not a standalone goal. Each trip begins at the preceding predicted position, so consecutive activities at the same site share the journey. Travel contributes to elapsed time, cooldowns and duration-weighted wellbeing scores.
- Walking follows a straight line at 1 km per 10 minutes. Position changes continuously, needs continue growing, and arrival lands exactly at the destination. Positive durations round up to milliseconds. Replanning waits for the current trip to finish when it crosses the two-hour commitment.
- Positions use metres as `f64`. `universe.map()`, `citizen.map()` and `citizen.position()` expose spatial state. Standalone citizens use `Map::default()`, with all sites at the origin, for nonspatial fixtures. `Map::new(...)`, `Citizen::with_map(...)`, `Citizen::with_position(...)` and `Universe::with_map(...)` support explicit setup.
- Inserting a citizen into a different map creates their own home and places them there and clears actions and plans tied to the old map. Insertion into the same map preserves their position and activity. Universe clones preserve their map.

- Citizens can own multiple properties. Private sites allow only the owner to travel there; public sites allow anyone. Invalid place IDs return `PlaceNotFound`; foreign private destinations return `PrivateProperty` from action start and duration calculation. `action_duration_ms` is fallible.
- Inventory is carried or held in owned market orders; there is no property storage, carrying capacity or spoilage. Berries, bread and berry pie supply food. Production recipes transform wheat, flour, wood, water and berries according to their input and output quantities.
