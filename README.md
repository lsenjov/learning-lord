# Learning Lord

A local, single-player town simulation for learning Rust, with a desktop inspector for time and citizen behaviour. The game design is in [plans/ticking.md](plans/ticking.md).

## Structure

- `crates/simulation`: engine-independent world data using `imbl` persistent collections.
- `crates/simulation/src/planning.rs`: citizen prediction and execution of committed action batches.
- `crates/desktop`: a Bevy desktop executable with time controls, a simulation clock, and citizen readouts. A single simulation worker owns the current universe and publishes its latest completed snapshot to the UI.
- Prediction branches will remain simulation data, independent of rendering.
- The universe starts empty at simulation time zero. Agents have UUID v4 IDs, names, and a kind; citizens have hunger, a constant hourly hunger rate, tiredness, a carried inventory of berries, wheat, flour, wood, water, bread, berry pie, flax, thread, cloth, flax blocks and flax garments in integer quantities (grams for bulk goods, loaves for bread, pies for berry pie, and whole blocks or garments for clothing items), integer coins, a gathering random generator, an optional action in progress, and an optional active plan.
- Cloning a universe shares collection storage. Changes to an owned clone leave the original and sibling snapshots unchanged.

## Simulation

- `Citizen::new(hunger)` starts an idle citizen with explicit finite hunger, tiredness zero, and the standard rate of 100 hunger per 24 simulation hours. `Citizen::with_hunger_rate(hunger, hunger_per_hour)` accepts a custom finite, nonnegative hunger rate and also starts tiredness at zero. `Citizen::with_needs(hunger, tiredness)` uses the standard hunger rate and explicit finite starting needs, clamping tiredness to a minimum of -100.
- Negative hunger represents satiation. Positive hunger represents a need for food. Neither side is clamped.
- Tiredness grows by 100 per 24 simulation hours, including while sleeping. Negative tiredness represents being rested; it has a minimum of -100 and no upper cap.
- `citizen.personal_wellbeing()` calculates `-clothing_need - max(hunger, 0) - 4 * max(hunger - 100, 0) - max(-hunger - 100, 0) - max(tiredness, 0) + 0.1 * wealth + food_reserve_bonus` from current state. Higher is better: hunger from -100 to 0 scores zero; each unit below -100 costs one point for overfull discomfort. Positive hunger costs one point per unit up to 100, and each unit beyond 100 costs five. Each positive unit of tiredness costs one point; negative tiredness gives no immediate bonus. `agent.personal_wellbeing()` delegates to its kind. Both return `Result<f64, SimulationError>`, rejecting score overflow.
- `universe.with_citizen(name, citizen)` returns `Result<(Universe, AgentId), SimulationError>`. Citizens receive a UUID v4 at construction; insertion preserves it and rejects an already-present ID with `AgentAlreadyExists`. Snapshot clones preserve citizen and place IDs.
- `citizen.start_action(action)` returns a new citizen with an action started without advancing time. Wait and Forage take 30 simulation minutes; Sleep takes eight hours. Sleep gradually removes 100 tiredness, subject to the -100 floor. Waiting supplies no recovery. `citizen.action_duration_ms(action)` gives the duration using current inventory; `active_action.duration_ms()` gives the duration fixed when the action started.
- Standalone citizens start with empty inventory and zero coins; desktop citizens start with 310 g berries (100 nutrition). Every desktop citizen receives 300 coins (100 base coins plus 200 coins for purchases), and the miller, baker, weaver and tailor also receive four hours of recipe inputs. Quantities use `u64` and coins use `i64`; coins may be negative for accounting. Prices and valuations remain floating point. Every catalogue good has a fixed initial reference price. At 04:00, observed market supply and affordable unmet demand adjust active prices by at most 10%; inactive prices stay unchanged. Sell orders hold finite individually owned stock with asking prices updated to the reference price at 04:00. `citizen.wealth()` includes coins, carried goods and owned listed goods valued at current reference prices, plus the uneaten portion of an active meal and an equipped garment valued at its remaining condition; property is excluded. Each coin of wealth adds 0.1 wellbeing. Carried edible food adds 0.10 wellbeing per nutrition up to 100, then 0.02 per nutrition up to 300 (maximum 14). Listed food adds no reserve bonus.
- `Citizen::new` starts with zero berries; the desktop supplies each citizen with 310 g (100 nutrition). `citizen.berries_units()` reads the inventory; `citizen.with_berries(units)` creates an idle snapshot with an explicit integer quantity for setup. Inventory replacement while an action or plan is active returns `CitizenBusy`.
- Berries provide 100/310 nutrition per gram; a 100 g loaf provides 50 nutrition and a 125 g pie provides 60. Eating takes one second per gram of weight. Eat selects one food by lowest current quoted market value per nutrition among foods that can individually supply a meal, using catalogue order for ties. Meals use whole units and can exceed 50 nutrition (one pie supplies 60); if no food can supply a full meal, it selects a partial meal. The whole meal leaves inventory at the start, hunger falls gradually, and interruption spoils the unfinished portion. Future needs beyond hunger may require revisiting this meal selection rule.
- Forage adds a uniform random 5–15 grams of berries only on completion (10 grams on average). Predictions use exactly 10 grams. Each citizen snapshot owns its random generator state, so prediction consumes no randomness and advancing independent clones cannot interfere with one another. Splitting an advance preserves actual forage outcomes; repeated advancement from the same snapshot reproduces them.
- `agent.start_action(action)` delegates to its kind. `universe.start_action(id, action)` returns a new snapshot with the specified agent's action started, preserving the clock and other agents. Starting another action while busy returns `CitizenBusy`; an unknown ID returns `AgentNotFound`.
- `citizen.advance(elapsed_ms)` returns a new citizen, growing both needs and applying the active action's recovery proportionally to time spent in it. Half a full 155-gram berry meal removes 25 hunger, alongside normal hunger growth. Actions grant no extra recovery on completion. Unplanned citizens spend any time after completion idle, while planned citizens continue their batch. The original citizen is preserved, and no universe is required. Time is `u64` milliseconds; need calculations use `f64`.
- `agent.advance(elapsed_ms)` delegates to its kind and returns an updated agent. `universe.advance(elapsed_ms)` advances its clock once and delegates to each agent in a cloned universe.
- Clock overflow, nonfinite need results, or planning errors return a `SimulationError`, leaving the source universe unchanged. Small advances agree with an equivalent large advance within floating-point tolerance for the same action choices.
- Read state through `current_time_ms()`, `agents()`, and the citizen's `hunger()`, `hunger_per_hour()`, `tiredness()`, `berries_units()`, and `active_action()` getters. An active action exposes its `action()`, `duration_ms()`, and `remaining_ms()`.

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

- `planning::plan(&citizen)` searches hunger, tiredness, clothing, food reserve replenishment and production goals, with a standing excess-listing policy. Actions expose `effects()` and `input_for(...)`; the planner recursively obtains missing food and production inputs through gathering, purchases, owned-order withdrawals and available production. Gathering uses average yields, while trades use the same prices and quantities as execution.
- A hunger goal acquires 50 nutrition (155 g of berries). Eat selects whole units of one available food toward 50 nutrition, and immediate partial meals remain candidates subject to the existing food replan check. Buying tops up to 50 nutrition when affordable. The planner discovers foraging, purchases and mixed food supply paths.
- Replenish reserves compares carried food targets of 100, 200 and 300 nutrition using the existing acquisition and travel paths. Exact purchase baskets preserve the selected target during execution. It competes through complete-plan scores without forced priority. Excess listings retain up to 300 nutrition in addition to production inputs. Food demand covers the remaining stops of the executing shopping trip, including travel within the selected goal; discarded forecasts and future goals create no food requests. Production input shortages remain standing demand.
- Each goal's backward search allows up to four hours of preparation (`goals::GOAL_HORIZON_MS`), including resource acquisition and travel. The final activity is excluded from that budget and finishes in full, so walking home followed by eight hours of sleep is valid.
- Alternatives are simulated from the predicted state at the goal's start. Compare complete single-goal candidates and ordered pairs of distinct goals, including each goal's prerequisite actions. There is no overall simulated-time horizon: an eight-hour sleep can be followed by another predicted goal. Choose the highest duration-weighted action-endpoint wellbeing among these candidates.
- Each action contributes the average of its starting and ending wellbeing, weighted by its duration. Goal and full-plan scores divide the sum of these contributions by total duration. Ending wellbeing includes completion effects, so gathering's wealth benefit is effectively spread across its action for scoring; usable inventory still arrives at completion. Candidates may cover different durations.
- Food and ingredient gathering compare quantity-driven orders within the four-hour prerequisite bound: currently up to eight half-hour segments. Production goals including foraging target a single two-hour block, including travel and input preparation; at the forest this gives four half-hour segments. Each order travels to its site once when needed. Adjacent orders of the same gathering family are forbidden across goal and nested resource boundaries; repeated primitive segments inside one order are allowed. Another gathering family or a non-gathering activity permits that family again; automatic travel alone does not. This restriction resets at replanning.
- Orders execute as the existing half-hour primitive actions, with goods arriving at each segment completion from an independent random draw. Prediction uses the existing average yield and endpoint score for every segment.
- Each top-level goal can appear at most once in a predicted pair. This resets at replanning; repeated primitive actions within a goal and recursive resource acquisition remain allowed. Foraging is a skill-free forest production recipe with expected output of 10 g berries per half hour; commercial targets apply to foraging, while hunger, food reserves and ingredient gathering can continue after those targets are met. Production follows a queue ordered by current profit per production time, advancing to the next recipe when its remaining target is exhausted or cannot be supplied. For each acquisition path, retain the first completed batch reaching the two-hour target, including travel and input preparation; when work runs out earlier, retain its last shorter prefix. Earlier goals do not shorten the production block. Its duration setting is separate from execution's goal-completion boundary.
- Production prefixes prepare their combined input requirements before the first batch, buying only enough for that candidate in one basket. Repeated food production for reserve targets uses the same preparation and accounts for edible ingredients consumed. Every batch remains a separate action with its own skill gain, duration and input consumption; the two-hour purchase cooldown begins after the final stop of the shopping trip.
- Starting-goal tasks share a lazy, process-wide FIFO queue with `max(logical CPUs - 4, 2)` long-lived workers (two when CPU detection fails). Each citizen request enqueues its roots together in the original goal order. Once those roots have been claimed, free workers can start another citizen's roots while earlier roots are still running. Each task owns immutable predictions and compares its single-goal candidates and possible second goals. Results reduce in original order, preserving ties, diagnostics and error order. Canceled queued tasks are skipped; running canceled tasks finish and their results are ignored. Worker panics become planning errors, and pool initialization failure uses sequential planning. Development builds optimize the simulation crate at level 2; prerequisite search remains a source of variable planning cost.
- Future tuning: revisit action-endpoint scoring accuracy and planning performance. The approximation assumes wellbeing changes linearly within each action, including completion rewards; needs can cross thresholds and goods actually arrive at completion. Later review should also examine whether averaging over different durations favors shorter sequences that omit later low wellbeing.
- Prediction-local cooldowns begin at completion: Eat waits four hours, shopping trips and listings each wait two hours independently; consecutive stops of the same trip share one purchase cooldown. Cooldowns carry across goals in the same prediction and reset for each new plan. They do not restrict manually started actions or carry across replanning.
- `Plan` exposes `actions()`, predicted `action_durations_ms()`, `goals()` with contiguous action ranges, and `average_wellbeing()`. Goal boundaries preserve the actual search choices, including their prerequisite actions; they do not change scoring. Execution uses the flattened primitive sequence. `citizen.start_planning()` starts the first action and stores the plan in `ActivePlan`; busy citizens reject replacement. Execution commits to the first complete goal, then replans. A second goal is a forecast and is never automatically entered. Existing shortage checks can still trigger earlier replanning.
- Background requests begin only during the final primitive action of the committed goal. A final action lasting at most thirty simulated minutes submits immediately when it starts; a longer one submits when thirty minutes remain. An eight-hour sleep therefore starts planning at seven and a half hours. Unexpected later-action shortages retain actual-state replanning at completion. The request forecasts completion of the current final action with expected forage and frozen market context.
- `Universe::advance_with_planner` uses a separate `PlanningRuntime` for background requests, leaving cloned universes and exported snapshots free of shared task handles. Keep one runtime per authoritative universe lineage and reset it before switching branches. At a replan boundary, simulation time waits for the result while desktop restart and shutdown remain responsive. Adoption applies only the selected plan to actual state, checking the first action and falling back to actual-state planning when the forecast is no longer executable; projected inventory, coins, needs, market and random state never replace live values. Existing `Universe::advance`, manual actions and planning initialization remain synchronous.
- Before a non-first Eat, actual food below 20 nutrition triggers replanning; the first action is exempt. Empty meals and trades are skipped, and exhausting a sequence starts a fresh plan. Real gathering outcomes determine actual meal and trade quantities.
- `ActivePlan` exposes its `plan()`, zero-based `action_index()`, and `elapsed_ms()`. The forecast second goal may change during replanning. Predictions leave the original citizen and its random stream unchanged.

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

The desktop starts paused with 24 citizens: four farmers, millers, woodcutters, bakers, weavers and tailors, including the original Ada, Bram, Cleo, Dara, Eira and Finn. Each starts at hunger -50 and tiredness -66.6 with 310 g berries (100 nutrition), planning enabled, 300 coins (100 base plus 200 for purchases), and a garment at 50% condition. Millers, bakers, weavers and tailors also receive four hours of recipe inputs (each weaver: 1,600 g flax; each tailor: 200 g cloth). Berries initially cost 5 coins/kg. Roles are starting configuration metadata and do not restrict actions. Each citizen owns a private home, and each farmer, miller, baker, weaver and tailor also owns their respective workplace. Forest, river and market are public. Production recipes use each profession’s skills and property; bread and berry pie supplement berries as food. Four pages of six cards, sorted by name, show coins, wealth, wellbeing and the current action with its remaining time. Hunger and sleep use signed bars centered at zero, with qualitative labels for satiation/rest, rising needs and urgency above 100. Overfull hunger below -100 is flagged separately. Bars clamp at either end; exact needs remain available in the detail panel. Clothing uses a separate bar from zero need to its 20-point maximum, with Clothed, Worn, Worn out or Absent labels.

Click a card or press **1-6** to select a citizen on the current page. **Previous/Next** changes roster pages and selects the first citizen on that page. Selection follows their ID across snapshots; restarting returns to page one and selects the new Ada. The map shows everyone, highlights the selected citizen and displays their travel route. Nearby citizen labels are offset to remain distinct. The scrollable detail panel shows inventory, exact needs, location, the goal-completion replanning boundary, saved planning scores and the active plan grouped by goal. Completed actions remain dimmed, the current action is marked **NOW**, and future actions retain predicted durations. The second goal is labelled as a forecast after replanning. Normal replanning waits for the first goal to finish; insufficient food or other failed prerequisites can trigger an earlier replan.

The selected citizen's **Daily history** view shows the live partial day and the last 30 completed days, closing at 04:00 alongside the market. Previous/Next selects a day. It records actual activity time, food-goal time (including preparation, travel and meals; overlapping the activity totals), goods produced/bought/sold/consumed, gross trade coins earned/spent, time-weighted average wellbeing and worst needs. Consumption includes recipe inputs, food reserved when a meal starts, and equipped garments. Cash totals reflect trades, independently of changes in goods' reference prices. The first period starts when the citizen joins the universe; predictions do not contribute to history.

Background replanning telemetry shows wall-clock request time (queue plus work) and blocked boundary waiting, attributed to the day a request is adopted. Speculative requests that finish early do not accumulate time while waiting for the boundary. Startup's synchronous plans and canceled requests are excluded; a rejected forecast's completed request is included when a fresh plan is adopted. Failed or canceled advancement preserves published telemetry; restarting clears it.

Switch between **Citizens** and **Market** with the tabs or **C/M**. Market lists all twelve goods with current prices; click a good or use **Up/Down** to inspect its quantities and current sell orders grouped by seller, with their selling location. **Current market day** starts at 04:00; **Previous market day** shows the latest closed period, including its closing stock and affordable unfulfilled demand. Buy volume is traded quantity plus affordable unfulfilled demand; sell volume is traded quantity plus listed stock. Unfulfilled demand can exist while stock is available. Sell orders and the current price remain live in either period view. Restart preserves the selected good and returns metrics to the current period.

Each good includes a recent 30-market-day chart. The blue line steps at 04:00 and candle centers show price in coins/kg for bulk goods, coins/loaf for bread and coins/pie for berry pie and coins/each for flax blocks and garments; body height shows traded quantity, the upper wick affordable unmet demand, and the lower wick unsold stock. Body and wicks share the separate quantity size key, in kg for bulk goods and whole loaves, pies, blocks or garments for counted goods. Cyan marks the live partial day; hover for its exact period, price and quantities. Inactive days carry their price with zero quantities, and the first day begins at world creation.

- **Restart universe** creates 24 fresh citizens in a paused universe at 1x with a randomized map and fixed initial reference prices, clearing errors and accumulated pacing. Current prices are listed in the Market tab.
- **Export universe** writes the displayed snapshot as readable Rust debug text to `debug/universe-<Unix seconds>.<nanoseconds>.txt`, relative to the launch directory. The timestamp is captured when the click is handled, in real-world UTC time. Exports include world, citizen, action, plan and decision data; they are diagnostic dumps, not reloadable saves. The app shows the resulting path or an export error, without changing simulation state. Export remains available while running or after a simulation error.
- **Run / Pause** (or **Space**) controls automatic advancement.
- **1x, 2x, 3x, 5x, 10x, 20x** select simulation speed without unpausing.
- **Advance 30 minutes** (or **Right arrow**) advances exactly 1,800,000 simulation milliseconds while paused, independent of speed. The button is unavailable while running.
- **Shift+Right arrow** advances to 04:00 on the following calendar day while paused, independent of speed. Actions, trades and daily price updates run normally throughout the interval.
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

- Listing and withdrawing goods take five simulation minutes at the seller's location: an owned field or workshop when available, otherwise the public market. All the citizen's listed goods use that location. Customers travel to stock and spend five minutes buying a basket per stop; needs continue growing. Planning visits the nearest location with useful stock, buys outstanding items available there, and repeats until filled or no useful stock remains.
- Sell orders hold finite stock owned by their sellers. Partial fills transfer stock and coins at completion, using live asking prices and the buyer's available funds. Citizens cannot buy from themselves or borrow. Withdrawals retrieve only the citizen's own listed goods.
- Settlement handles changed availability; competing completions use chronological order. Inventory units and actual coins are conserved exactly. Each purchase charges the ceiling of the quoted value per seller and good; fills across multiple orders from that seller for the same good are grouped before rounding. Buyers purchase only whole units they can afford.
- Predictions clone the market, use the same settlement rules and assume stable prices. They do not reserve actual stock, record actual demand or guarantee future sales. Execution replans when shortages prevent an action.

## Market updates

- The universe owns market prices and synchronizes the price context on citizen snapshots. Initial reference prices in coins/kg are berries 5, wheat 40, flour 100, wood 50 and water 10; bread costs 15 coins/loaf and berry pie 25 coins/pie. Flax, thread and cloth cost 40, 80 and 120 coins/kg; flax blocks and garments cost 11 and 96 coins each. Standalone citizens use `Prices::default()` unless given `with_prices(...)`; insertion uses the universe's prices.
- At 04:00, supply is traded units plus remaining listed units; demand is traded units plus affordable unfilled purchase requests. Active prices change by at most 10% toward the supply/demand balance, with a small positive numerical floor; inactive prices stay unchanged. Requests replace earlier intentions rather than accumulate repeated attempts.
- Completions exactly at 04:00 settle into the closing interval before prices update. Remaining standing orders are repriced to the updated reference prices; newly completed listings use current reference prices. Purchases already underway settle against live orders at completion. Inventories are revalued immediately, while plans remain until normal replanning. Predictions retain their starting reference prices.

## Daily production targets

- For each profitable recipe output, the stock target is the greater of estimated daily sales plus unmet demand and twelve hours of output. The minimum uses whole batches at the citizen's current skill, even on days with little demand.
- Carried and listed saleable stock and output already underway count toward that target. Personal food reserves remain separate. Each recipe independently considers twelve hours of future production against the same available inputs and funds, subject to skills and property. These targets are alternatives; planning tries recipes in profit-per-hour order and moves on when inputs cannot be supplied within its four-hour window. Completed work does not reduce the stock target. Ingredient reserves and standing purchase demand use the maximum requirement for each good across these alternatives. Inputs committed to an underway batch are added separately after subtracting that batch from its future alternative. Personal food is reserved after production ingredients; purchase shortages subtract carried and owned listed stock.

## Last planning decision

- The desktop shows the starting Hunger, Sleep, Clothing, Replenish reserves, Production and List excess candidates from the last replan, their duration and average action-endpoint wellbeing. Repeated actions are grouped for readability.
- Each displayed sequence leads to the best single-goal or two-goal plan for that first goal. Goal avg covers only that first sequence and is informational. Plan avg covers the whole chosen candidate; this determines the chosen first goal. An available but unchosen goal is distinct from an unavailable goal.
- Unavailable means no executable sequence was found within the four-hour prerequisite limit and action rules; the report does not attribute failure to a particular resource or cooldown.
- Each available recipe also shows decision-time estimated profit per hour, remaining batch target, and carried/missing batch inputs. Diagnostics record observed executable sequences, recipes in the selected full plan (including later goals), and competing complete plans that scored lower. Missing accessible supplies and observed four-hour preparation exclusions are reported separately; mixed failures retain a general explanation. Ordered production prefixes may never reach a recipe, which is explicitly distinguished from a tested failure. These are saved decision snapshots, not live inventory or an exhaustive independent feasibility check.
- `Plan::decision()` exposes the saved candidates, selected starting goal and prices used. The report is captured during the existing search, shared across snapshot clones, and replaced on replanning. Price changes do not rewrite past decisions. No additional planning is performed for the display.

## Locations and travel

- Day zero is Monday. The desktop starts at 06:00; weekly obligations use Monday 06:00, with the first settlement on day seven. The simulation's explicit starting-time APIs still support other times for fixtures.
- The town has a warehouse and a treasury initially containing zero coins. Location storage records the goods owner separately from the property owner, so citizen-owned and town-owned stock can coexist. Stored personal goods count toward wealth but must be withdrawn before consumption, production or listing. Automatic retrieval and warehouse hauling are not implemented yet.
- `Universe::deposit_goods` and `withdraw_goods` transfer only a citizen's own goods while physically at the location. Transfers preserve active stationary work and protect its required inputs; travelling citizens cannot transfer stock. `with_stored_good` configures initial stock without taking it from carried inventory.

- The desktop generates 48 distinct sites within the 2 km × 2 km map, separated by at least 200 m: 24 private homes, four each of fields, mills, bakeries, weaveries and tailories, a public forest, river and market, and the town warehouse. Every place has its own UUID v4, type, position, name and optional citizen owner. Citizens start at their own homes. The map shows all sites and citizens, selected ownership and the current route. Selected details show inventory, owned properties, location and the full plan.
- Sleep requires the citizen's own home, Forage the forest, Fetch water the river, and listing or withdrawal the citizen's selling location. Purchases require arrival at the stock's location. Eat and Wait are available anywhere. Starting an activity at the wrong site returns `WrongLocation`.
- The planner inserts `Travel(PlaceId)` before an activity when needed; travel is not a standalone goal. Each trip begins at the preceding predicted position, so consecutive activities at the same site share the journey. Travel contributes to elapsed time, cooldowns and duration-weighted wellbeing scores.
- Walking follows a straight line at 1 km per 10 minutes. Position changes continuously, needs continue growing, and arrival lands exactly at the destination. Positive durations round up to milliseconds. Travel inside a committed goal completes before its following activity; it no longer triggers replanning merely because two hours have elapsed.
- Positions use metres as `f64`. `universe.map()`, `citizen.map()` and `citizen.position()` expose spatial state. Standalone citizens use `Map::default()`, with all sites at the origin, for nonspatial fixtures. `Map::new(...)`, `Citizen::with_map(...)`, `Citizen::with_position(...)` and `Universe::with_map(...)` support explicit setup.
- Inserting a citizen into a different map creates their own home and places them there and clears actions and plans tied to the old map. Insertion into the same map preserves their position and activity. Universe clones preserve their map.

- Citizens can own multiple properties. Homes allow only the owner to travel there; fields, workshops and public sites allow customers. Production still requires ownership of private workplaces. Invalid place IDs return `PlaceNotFound`; foreign homes return `PrivateProperty` from action start and duration calculation. `action_duration_ms` is fallible.
- Inventory is carried or held in owned sell orders at their selling location; there is no general property storage, carrying capacity or spoilage. Berries, bread and berry pie supply food. Production recipes transform wheat, flour, wood, water and berries according to their input and output quantities.

## Location taxes

- Open the **Locations** tab to select a property, inspect stored goods by owner and see sell-order stock. Private locations support adding, editing and removing named tax rules. Percentage taxes default to **All** with a shared rate; choosing individual goods restricts the rule to those goods and allows separate rates. Socage offers only outputs of recipes supported by that location (for example, wheat and flax at fields); asset and income taxes offer every good. Flat fees select a payer and weekly coin amount. Public locations and the town warehouse are available for inspection but cannot be taxed yet.
- The same view shows the treasury, outstanding coin arrears and recent tax receipts. Deposit/Withdraw controls move a selected citizen's own goods only when that citizen is physically at the selected location. Invalid edits and transfers report an error without stopping the simulation. Text fields suspend simulation keyboard shortcuts while editing; Enter or Escape leaves the field.
- New universes have no active taxes (Frankalmoigne). Private locations can have multiple named rules, with separate rates for each selected good: Socage takes production output, Asset tax assesses stored goods weekly, and Income tax takes sales revenue. Flat fees name a citizen responsible for a weekly coin payment. Public-location taxes, employment and treasury spending are deferred.
- Weekly collection first occurs on day seven, Monday 06:00, then every seven days. Asset assessments use current reference prices for citizen-owned stored goods and sell-order stock at the location. Carried goods, buildings and town-owned stock are excluded. Sales are taxed at the actual sale location, against the seller's revenue.
- Percentage rates have 0.01-percentage-point precision. Stacked rules use the original taxable amount; combined Socage rates for a good cannot exceed 100%. Fractional goods and coins accumulate per rule, payer and good, rather than rounding up each small transaction. If several goods obligations become due together, collection cannot exceed that batch's output and outstanding quantities carry forward.
- Socage transfers goods to town ownership at the production site, without hauling them to the warehouse. Coin taxes transfer available coins to the treasury; unpaid amounts become arrears and are retried at weekly settlement. Removing a rule stops new assessments while preserving existing obligations and receipts.
- Production predictions and targets deliberately remain gross, without subtracting taxes. Authoritative execution collects taxes before the next actions and replanning; affected background predictions are invalidated. The actual retained inventory and balances are used by subsequent plans.
- Immutable `Universe::with_tax_rule`, `edit_tax_rule` and `without_tax_rule` configure rules. `tax_rules`, `tax_arrears`, `tax_history`, `storage` and `town_treasury` expose their state. Tax history keeps the latest 4,096 receipts; outstanding obligations remain independently recorded. Universe debug exports include this state.

## Clothing

- Farmers grow 200 g flax per hour. Weavers spin 200 g flax into 200 g thread and weave 200 g thread into 200 g cloth. Tailors turn 25 g cloth into one 25 g flax block and assemble eight blocks into one 200 g garment. Each processing batch takes 30 minutes before skill speed improvements.
- Equipping a carried garment takes five minutes anywhere, consumes one garment and replaces the worn garment with full condition. Equipped garments wear continuously from full to zero condition over ten simulation days. Clothing need is `20 * (1 - condition)`, with absent clothing counting as zero condition and a 20-point penalty. Clothing goods provide no nutrition. The inspector shows clothing need and equipped garment condition; debug exports include both inventory and garment state.
