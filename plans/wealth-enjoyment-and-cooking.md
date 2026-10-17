# Wealth enjoyment and cooking

## Status and priorities

- Discussion notes from 17 October 2026, not an agreed implementation specification.
- The user wants a richer economy through professions, recipes, ingredients, needs and locations. Businesses are deferred until larger economies make them useful; see [Businesses and employment](businesses-and-employment.md).
- The user supports exploring enjoyment and increased usable nutrition from cooking, including enjoyment from beer. Exact mechanics and tuning remain undecided.
- Wealth and asset valuation need further discussion. The user raised whether diminishing returns should apply only to coins and whether applying them to assets would discourage upgrades.
- Pause this work to discuss buildings and facilities. No wealth, enjoyment, food or facility mechanics were changed by recording these notes.

## Economic expansion options

- Cooking: a cook and kitchen connect fishing, farming, milling and woodcutting. Example goods are fish stew and vegetable dumplings. Prepared meals need a real benefit over consuming their ingredients directly.
- Tools: a more productive fishing recipe could consume thread or nets. Batch inputs provide a smaller initial experiment than durable equipment, wear and replacement planning. Carpentry and smithing could follow.
- Household comfort: a carpenter makes furniture that provides benefits while installed in homes. This introduces shared household benefits, placement and eventually wear.
- Alternative clothing: cotton crops, thread, cloth and garments create substitution and competition for fields and workers. Alternatives need meaningful differences or one route may dominate.
- Animal husbandry: feed, pasture and care produce wool, milk or meat. Multiple outputs, ongoing care and reproduction make this a larger feature.
- Hygiene and services: soap and bathhouses introduce manufactured consumption and paid facility use. Capacity, payments and queues require additional mechanics.
- Prefer connecting existing chains and creating competition for shared ingredients over simply increasing the number of goods.

## Why cook

- Combining ingredients into one good is not sufficient justification: citizens can already consume mixtures of foods for a balanced diet.
- Possible benefits are more usable nutrition, enjoyment, less eating time and preservation.
- Increased usable nutrition trades off against preparation labour, fuel and other inputs. It should be chosen per recipe rather than automatically increasing every dietary benefit.
- Enjoyment makes prepared foods desirable beyond survival. Raw foods can remain adequate alternatives.
- Time saved while eating may be too small to support a profession by itself; evaluate against actual action durations.
- Preservation becomes useful once spoilage exists. Do not silently introduce spoilage as part of a cooking experiment.

## Enjoyment proposals

- The user asked whether enjoyment should stack across sources or be set to a value by consumption.
- Proposed starting model: one enjoyment value per citizen, raised by enjoyable food and beer, with diminishing gains as it rises and gradual decay over time.
- Contributions scale with quantity consumed. Five pieces of a pie must have the same total effect as the equivalent whole pie under equivalent consumption timing; splitting items must not create extra rewards.
- Plain food should not erase enjoyment from a previous meal. Setting enjoyment to the latest food's quality would create that problem.
- Taking only the highest recently consumed quality could let one tiny bite provide the whole benefit.
- Unlimited additive bonuses could make repeated eating or drinking dominate other activities. Choose a bounded response and test it alongside hunger, thirst and overfull penalties; do not assume diminishing gains alone prevent excessive consumption.
- Expected behaviour: a treat is more attractive after a period without enjoyment; another treat soon afterward has less value; ordinary meals still satisfy needs.
- A shared enjoyment value allows pleasant food and beer to partly substitute for each other. Separate source values would encourage consuming both but introduce more needs to maintain. Start shared unless distinct behaviour is wanted.
- No enjoyment maximum, decay rate, dose curve or per-good contribution has been agreed.

## Wealth savings and spending

- User objective: additional savings should eventually contribute less to wellbeing, encouraging beneficial spending or investment rather than endless accumulation.
- Diminishing wealth wellbeing makes spending less costly in wellbeing terms; it does not create a benefit from spending. If needs are satisfied and nothing useful is available, saving may remain reasonable.
- If only coins have diminishing value while goods keep linear value, an agent could improve wellbeing simply by converting savings into stock. This risks replacing cash hoarding with inventory hoarding.
- If only coins contribute to economic wellbeing, buying ingredients and producing goods can look costly until sales occur. Short planning horizons could again reject worthwhile production.
- Assistant recommendation, not a settled decision: apply a smooth increasing curve with diminishing returns to total economic wealth, preserving inventory valuation. Avoid a hard cap or making additional wealth harmful.
- Intended tradeoff: poor agents lose more wellbeing from a luxury purchase; wealthy agents lose less, while both can benefit from enjoyment or comfort. This should not categorically forbid treats for poor agents.
- A future separate liquidity or security benefit could reward holding enough coins for expected purchases, distinguishing cash from unsold stock. Defer the extra reserve calculation initially.
- Any nonlinear scoring change must account consistently for individual and household wealth through existing ownership and allegiance rules, without double counting assets. The formula and household treatment still require specification.

## Assets upgrades and benefits from use

- User concern: diminishing total-wealth wellbeing could discourage upgrading a house or asset beyond some point.
- That is true if an upgrade's only benefit is a higher monetary valuation. Useful upgrades should also change what the asset provides.
- Separate resale or economic value from benefits while in use: home comfort, accommodation capacity, workshop productivity and useful storage capacity.
- Spending 1,000 coins on an upgrade valued at 1,000 coins leaves total economic wealth approximately unchanged, assuming that valuation is justified. Comfort or useful capacity supplies the reason to upgrade.
- A wealthy agent may accept an upgrade whose resale value is below its cost because the lost wealth has lower marginal wellbeing value.
- Exempting assets from diminishing wealth value would risk rewarding expensive purchases even when they provide no useful improvement.
- Productive investment remains a planning problem: benefits may arrive outside the current short horizon. A diminishing wealth curve does not by itself make those investments visible.
- Asset valuation, comfort, facility capacity and investment evaluation are not yet specified or implemented by this proposal.

## Pie portions recipes and meals

- The user suggested smaller berry pies so they can be combined with other foods, with enjoyment distinguishing them.
- Proposed conversion: split the current output into smaller portions while preserving total batch nutrition and economics, following the bread-roll precedent. Portion size and enjoyment remain undecided.
- Distinguish a production recipe, which creates a named good, from a meal, which can combine available foods. Explicit recipes do not require enumerating every possible meal combination.
- The user proposed cooking only from current supplies to limit search. Concern: forbidding procurement could leave a profitable cook idle. Prefer owned supplies first while retaining existing ingredient purchasing.
- The user also proposed generic recipes such as a proportion of carbs and protein plus wood. Current dietary categories are not measurements of ingredient composition; generic recipes would require substitution, quantity, pricing and admissibility rules.
- Assistant recommendation: initially use two or three explicit recipes with bounded search. Examples are fish, cabbage, water and wood for stew; cabbage, flour, water and wood for dumplings. Quantities, output and benefits are illustrative, not agreed.

## Suggested sequence when resumed

- Agree and test diminishing wealth wellbeing independently before adding further behaviour.
- Add enjoyment to beer and smaller pie portions, then observe whether citizens actually pay for it.
- Add a cook and a few explicit recipes only after the consumption incentives work.
- Confirm all formulas, constants, household effects, planner comparisons, diagnostics and save compatibility before implementation.
- Buildings and facilities are the next discussion topic. Their useful effects should provide concrete reasons for asset upgrades, independently of merely assigning those assets higher prices.
