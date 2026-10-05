# Individual production and finite markets

Implement the agreed prototype economy in reviewed, committed stages. If implementation exposes a material behavioural choice outside these defaults, stop and discuss it with the user before proceeding.

## Agreed behaviour

- Ownership remains individual: each citizen owns their goods, coins, properties, orders and production target.
- Remove pebbles and Find Rocks, including their old planner and interface paths.
- Finite sell orders hold goods until purchased or withdrawn. Partial purchases transfer goods and coins atomically at action completion. Buyers cannot buy from themselves. Unsold goods remain owned wealth and can be retrieved at the market.
- Listing and buying take five minutes at the market. Keep prediction-local trading cooldowns. New orders use reference prices; existing orders retain their asking prices.
- A Buy action accepts a shopping list containing several goods. The entire purchase takes five minutes and starts one shared two-hour prediction-local buy cooldown at completion. It buys available quantities within the citizen's budget; it does not wait two hours between different ingredients. This was explicitly clarified and approved during step 1.
- Predictions clone market state, assume stable prices and do not reserve real stock, advance other citizens or write actual demand/sales history. Execution tolerates changed stock and replans when a shortage prevents the next action.
- Track executed trade quantity/value and historical market activity, as well as each citizen's current affordable purchase requirement. Repeated attempts replace requirements instead of accumulating fictitious demand.
- Replace random prices with bounded daily supply/demand adjustments at 04:00. No activity leaves prices unchanged. Explain the adjustment inputs in observable state.
- Recipes define inputs, outputs, required skills/property and skill-adjusted duration. Farmers produce wheat, millers flour, woodcutters wood, bakers bread or berry pie; anyone can fetch water or forage.
- Choose prototype numerical values to make the production chain generally more efficient than subsistence foraging. Keep them central and easy to tune.
- Production targets are revisable quantities, not strict timetables. Start with a small exploratory batch per eligible profitable product; use observed volume and profitability per hour to choose replenishment work within estimated daily capacity.
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
