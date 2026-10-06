# Individual production and finite markets

Implement the agreed prototype economy in reviewed, committed stages. If implementation exposes a material behavioural choice outside these defaults, stop and discuss it with the user before proceeding.

## Agreed behaviour

- Ownership remains individual: each citizen owns their goods, coins, properties, orders and production target.
- Remove pebbles and Find Rocks, including their old planner and interface paths.
- Finite sell orders hold goods until purchased or withdrawn. Partial purchases transfer goods and coins atomically at action completion. Buyers cannot buy from themselves. Unsold goods remain owned wealth and can be retrieved at the market.
- Listing and buying take five minutes at the market. Keep prediction-local trading cooldowns. New orders use reference prices; at 04:00, existing orders adopt the updated reference price for their good.
- A Buy action accepts a shopping list containing several goods. The entire purchase takes five minutes and starts one shared two-hour prediction-local buy cooldown at completion. It buys available quantities within the citizen's budget; it does not wait two hours between different ingredients. This was explicitly clarified and approved during step 1.
- Predictions clone market state, assume stable prices and do not reserve real stock, advance other citizens or write actual demand/sales history. Execution tolerates changed stock and replans when a shortage prevents the next action.
- Track executed trade quantity/value and historical market activity, as well as each citizen's current affordable purchase requirement. Repeated attempts replace requirements instead of accumulating fictitious demand.
- Replace random prices with bounded daily supply/demand adjustments at 04:00. No activity leaves prices unchanged. Explain the adjustment inputs in observable state.
- Recipes define inputs, outputs, required skills/property and skill-adjusted duration. Farmers produce wheat, millers flour, woodcutters wood, bakers bread or berry pie; anyone can fetch water or forage.
- Choose prototype numerical values to make the production chain generally more efficient than subsistence foraging. Keep them central and easy to tune.
- Production targets are revisable quantities, not strict timetables. Start with a small exploratory batch per eligible profitable product; use observed volume and profitability per hour to choose replenishment work within estimated daily capacity.
- Production targets aim for one day of expected own sales plus the full affordable unmet market demand. Do not divide unmet demand among producers: each may compete for it, constrained by its own time, funds, skills, property and inputs. Market requests are still counted only once. Subtract carried saleable output, listed output and output already underway; choose batches by profit per production hour within a 12-hour daily capacity. Account for personal food separately. Unsold exploratory output counts toward the target and prevents endless trial batches.
- Unsold stock is intentionally allowed to keep pushing reference prices down, even when producers maintain buffer stock. Keep the daily percentage bound and a small positive numerical floor. Outside traders and an economic export-price floor are future work, not part of this milestone. Daily price updates also reprice remaining standing orders without changing their owners or quantities.
- The four-hour action planner executes production intentions alongside hunger/sleep, retaining the two-hour commitment and finishing the current action. Targets are reconsidered daily or when essential inputs become unavailable.
- Reserve inputs for remaining target quantities plus personal food. Combine competing uses without counting the same inventory twice. Reserves restrict selling, not necessary eating.
- Listing excess is a standing policy. A minimum listing value avoids tiny dedicated selling trips. Listing gives no artificial wealth bonus and predicted sales are not guaranteed.
- Everyone starts with 200 g berries. Ada (farmer) and Cleo (woodcutter) receive 2 coins each. Bram (miller) and Dara (baker) receive four hours of production inputs and no coins; Dara begins with bread inputs. Initial prices are fixed and there are no initial orders/history.
- Show market activity, individual production intentions, reserves and sell orders in the interface and debug export.

## Steps

1. Introduce finite orders and market history; remove pebbles and Find Rocks. Coordinate actual action completions and trades through the universe, while citizen needs remain locally advanced. Add generic listing/purchasing/withdrawal and isolated prediction support. Begin with fixed reference prices. Verify conservation, competing buyers, partial fills, ownership, chronology and tick partitioning.
2. Define production recipes, skill capabilities, all reference-priced goods and edible bread/pie. Add production primitives and recipe-based starting inputs. Keep existing snapshot, needs and location contracts. Validate recipe arithmetic, skill/property requirements and remaining existing behaviour.
3. Implement affordable demand tracking and daily supply/demand pricing. Verify replacement of stale demand, correct interval history, inactive markets, bounded changes and prediction isolation.
4. Implement production quotas, input/food reserves, production execution and the standing excess-selling policy in planning. Verify multiple recipes, personal needs, changing stock, target completion and replanning without optimistic sales.
5. Apply starting conditions, expose economy details in the desktop/debug output, update current documentation and run economic acceptance scenarios. Check responsiveness and execution cost as well as goods/coin conservation and whether production/trade can emerge.

Each step must pass appropriate formatting, tests and Clippy, receive independent Sol-6.1 review, resolve high/medium issues, fix low documentation issues, report other low issues, and be committed before the next step.

## Implementation notes and verification

- Initial inspection: existing universe advancement advances citizens independently over each price interval. Shared finite transactions require chronological action-completion coordination rather than independently spending a shared inventory snapshot.
- Step 1 complete: persistent finite orders, trade history, generic five-minute listing/withdrawal and shopping-list purchases, cheapest asking-price fills, chronological shared settlement, deterministic order IDs and isolated predictions. Pebbles/Find Rocks are removed. Fixed initial reference prices cover berries 1, wheat 0.4, flour 1, wood 0.5, water 0.1 and bread 1.5 coins/kg; production/edibility follow in step 2. Insufficient basket budgets fill goods in catalogue order.
- Step 1 verification: 132 workspace tests, formatting/diff checks and Clippy with warnings denied passed. Independent Sol-6.1 review reports no remaining high, medium or low findings. Coverage includes competing/chronological buyers, partial fills, one-action shopping lists, shared budgets/cooldowns, retained asking prices, ownership/withdrawal, conservation, numeric failure isolation, prediction history isolation and partition-independent transactions.

### Stage 2 prototype production values

- Fixed prices in coins/kg: berries 1, wheat 0.4, flour 1, wood 0.5, water 0.1, bread 1.5, berry pie 2.
- Baseline batches: grow wheat 1 h → 200 g; mill flour 1 h consumes 300 g wheat → 240 g flour; chop wood 1 h → 200 g; fetch water 30 min → 200 g.
- Bread 1 h consumes 100 g flour, 25 g wood and 100 g water → 200 g bread. Berry pie 90 min consumes 100 g flour, 25 g wood, 50 g water and 100 g berries → 250 g pie.
- Berries/bread provide 0.5 nutrition/g, pie 0.6 nutrition/g; all currently take 1 s/g to eat. Meals combine available edible goods in catalogue order, up to 50 nutrition.
- Starting professions grant their skill at level 1. Duration is baseline divided by `1 + 0.05 × max(level − 1, 0)`; completed skilled batches gain 0.1 level. Water requires no skill.
- Production validates input availability at start and transforms inputs/outputs atomically at completion. Single active actions prevent concurrent input consumption, so no work-in-progress escrow is needed yet.
- Startup cash and recipe-derived four-hour input supplies are applied in stage 2. Production targets and autonomous production execution remain stage 4.
- Food acquisition considers each edible good and distinct mixed shopping lists formed from all six orderings of the three edible goods, retaining the existing 30/50 nutrition prerequisites and full-plan comparison.
- Step 2 verification: 140 workspace tests passed; formatting/diff checks and Clippy with warnings denied passed. Independent Sol-6.1 review found no high or medium issues. Its low nonfood-helper panic was fixed and covered by the existing invalid-food test; all seven production tests passed again. The mixed-food four-hour search measured approximately 230–240 ms in debug mode, without changing pruning rules.

- Stage 2 measurement: mixed bread/pie market full four-hour plan searched in ~241 ms in the unoptimised test build; all 140 workspace tests completed successfully. No search pruning was added.

### Stage 3 price and demand accounting

- Actual shopping requests replace each agent's previous request. Filling goods reduces the remaining request; new actual actions/plans replace or clear it. Raw intention remains separate from affordability so later income can fund an existing requirement.
- Affordability shares one live coin budget across catalogue goods. Accessible order asking prices are used first; missing stock is estimated at the reference price. Predictions register no requests or history.
- At 04:00, supply is traded grams plus remaining listed grams; demand is traded grams plus current affordable unfilled requests. The next price is `price × (1 + 0.10 × (demand − supply) / (demand + supply))`, bounded below by 0.0001 coins/kg and above by finite f64 range. Inactive goods retain their price.
- Unsold buffer stock deliberately produces continued downward pressure, as confirmed by the user. Existing order asking prices remain unchanged; newly completed listings use the current reference price.
- Action completions at exactly 04:00 settle into the closing interval before prices update; subsequent actions see the new reference prices. History records active per-good intervals with traded volume/value, remaining supply/unmet demand and before/after prices. Omitted inactive periods still count as elapsed time for later sales-rate estimation.
- Standing supply and unmet demand use the boundary snapshot initially. No time-weighted stock integration is introduced in this prototype.
- Step 3 verification: 148 workspace tests, formatting/diff checks and Clippy with warnings denied passed. Independent Sol-6.1 review approved the logic and separately ran all 12 demand/price tests. The sole low documentation finding was corrected. Tests cover replacement/cancellation, live shared budgets, partial fulfilment, existing asking prices, interval history, daily limits/floor, snapshot isolation and boundary chronology.

### Standing-order repricing

- Daily updates at 04:00 now apply to existing orders, preserving IDs, owners and quantities. Earlier fixed-ask notes are superseded by this decision.
- Completions exactly at the boundary settle before repricing; purchases spanning the boundary use the updated live asks when they finish. Historical trades remain unchanged.
- Explicit `with_prices` fixture setup changes reference context without simulating a daily update.
- Independent review found no high, medium or low issues; all 27 demand, price and trading tests passed.

## Starting economy tuning

- Berries now start at 0.05 coins/kg. All four citizens receive 1 base coin plus 2 coins for purchases (3 coins total), preserving their berries and production inputs. Earlier diagnostic observations above describe the previous starting economy.
