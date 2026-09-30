# Ticking and agent behaviour

## Universe and agents

- A universe is a game state made of immutable data structures.
- A universe contains agents: animals, citizens, and collectives.
- Collectives include families, guilds, businesses, foreign traders, and kingdoms.
- Agents can have relationships with one another.
- Agent IDs are UUID v4s, generated when an agent is created and preserved when universes are cloned.
- Agent kinds hold their specific data. Citizens have hunger and tiredness; collectives do not have metabolic needs of their own.

## Numeric representation

- Continuous simulation measurements, including hunger, tiredness, rates, and wellbeing, use `f64`.
- Simulation time and elapsed durations use `u64` milliseconds.
- Coins and wealth use `f64`; small rounding errors are acceptable. Coins may be negative for accounting.
- Inventory goods are measured in grams as `f64`. Berries are the first good implemented.
- Discrete counts and tile coordinates remain integers.

## Needs

- Agents have needs.
- Some needs increase over time, such as hunger and the need for rest.
- Citizen hunger is a signed energy deficit: below zero is satiation, zero is the boundary, and above zero is a need for food.
- Hunger increases with elapsed simulation time. The standard rate is 100 hunger per 24 game hours, balanced by 200 grams of berries (two full meals of 50 nutrition each). Starting hunger is explicit; custom constant hourly rates remain available.
- Starting hunger must be finite. Hunger rates must be finite and nonnegative. Hunger is not clamped at zero or at the starvation threshold.
- Citizens start with no berries. Berries provide 0.5 nutrition per gram and take one second per gram to eat. Eat selects up to 50 nutrition from current inventory when starting, using smaller portions when needed and skipping empty meals. The portion determines duration, rounded up to the nearest millisecond, and is consumed gradually alongside its hunger reduction.
- Find rocks takes 30 minutes and adds uniform random 5–15 grams of pebbles on completion; predictions use 10 grams. Pebble inventory uses grams as `f64`.
- Forage takes 30 minutes and adds a uniform random 5–15 grams of berries on completion. Average yield is 10 grams: 20 forage actions (10 hours) yield the 200 grams needed per day on average.
- Tiredness grows by 100 per 24 simulation hours, including during sleep. Below zero represents being rested; tiredness has a minimum of -100 and no upper cap.
- Sleep takes eight simulation hours and gradually reduces tiredness by 100 over that duration, subject to the floor. Eight hours of sleep balances one day of tiredness growth when recovery is not lost at the floor. Hunger keeps growing during sleep.
- Existing citizen constructors start tiredness at zero. `Citizen::with_needs(hunger, tiredness)` accepts finite starting needs with the standard hunger rate and clamps starting tiredness at -100.
- Other needs have a constant baseline, such as a peasant's clothing need of 40.
- Goods can reduce a need. Peasant clothing might reduce clothing need by 50.
- As clothing degrades, its reduction becomes smaller, so the effective clothing need increases.

## Personal wellbeing

- An agent's personal wellbeing score is calculated from its needs and other attributes.
- Currently, `Citizen::personal_wellbeing()` calculates a hunger, tiredness, and wealth score on demand without a universe or stored score. `Agent::personal_wellbeing()` delegates to its kind.
- Higher scores are better. The formula is `-max(hunger, 0) - 4 * max(hunger - 100, 0) - max(-hunger - 100, 0) - max(tiredness, 0) + 10 * wealth`.
- Hunger from -100 to 0 scores zero. Satiation delays future hunger rather than granting an immediate wellbeing bonus.
- Hunger below -100 represents being overfull, costing one point per excess unit with no jump at the threshold.
- Hunger above 0 costs one point per unit up to 100. Each unit beyond 100 costs five points, with no jump at the starvation threshold.
- A score outside the finite `f64` range returns `SimulationError::WellbeingOverflow`.
- Each positive unit of tiredness costs one wellbeing point. Negative tiredness delays future tiredness without granting an immediate bonus.
- Clothing will be an important factor later.
- Total wealth is coins plus inventory valued at current market prices, contributing 10 wellbeing per coin. Coins and wealth use `f64`; citizens start with zero coins and pebbles. The placeholder marketplace prices berries at 1 coin/kg and pebbles at 2 coins/kg; the marketplace has unlimited stock and coins.
- A collective's wellbeing could depend primarily on its members' needs, or it might depend only on money. This is still undecided.

## Relationships

### Objectives

- **Care about others' outcomes.** An action can be worthwhile because it improves another agent's wellbeing, even if it is not the best action for the acting agent personally.
- **Allow personal sacrifice while retaining self-interest.** An agent might accept less money or spend time helping family. Its own needs still matter; relationships determine how much weight others' interests receive.
- **Give collective interests influence over individual decisions.** A citizen should have a reason to contribute to its family, guild, business, or kingdom beyond the payment it receives. This is an inferred objective of allowing relationships with collectives.
- **Make established relationships matter from the beginning.** Family members should start with reasons to support one another, without first needing a history of profitable interactions.
- **Let cooperation build relationships.** Agents that work together should become more inclined to help one another in future. Relationships can emerge during the simulation rather than remaining fixed at creation.
- **Let exploitation weaken relationships.** Taking advantage of another agent should reduce that agent's willingness to favour or sacrifice for the exploiter.
- **Influence who agents trade with.** When multiple sellers can fulfil a purchase, buyers should favour the seller they have the strongest relationship with. Relationships affect where money flows, even with a global price.
- **Support several attachments at once.** An agent can care about itself, its family, and other agents simultaneously. Decisions should reflect the relative strength of those attachments. Conflicting attachments are an inferred consequence; specific examples are still undefined.

### Boundaries

- Ordinary material incentives remain sufficient. Food, board, and wages can make an arrangement attractive on their own; relationships add another reason to choose it.
- Concern stays direct. Caring about an agent does not automatically mean caring about everyone that agent cares about.

### Open question

- What does it mean to help a collective? Feeding a hungry family member, increasing the family's treasury, and expanding its business can favour different actions. Which outcomes should a citizen's relationship with the family encourage?

## Actions and planning

- Agents have actions, each with an estimated duration and a result.
- Currently, citizens can Eat for a duration determined by available berries, Wait, Forage, or Find rocks for 30 minutes, or Sleep for eight hours. Only one action can be active at a time; both needs continue growing during all actions.
- Starting an action returns a new snapshot without advancing time. Eat gradually consumes its selected berry portion and restores hunger; Sleep gradually grants 100 tiredness recovery; Wait grants no recovery. Completion gives no extra need reduction; Forage grants its berry yield on completion. Unplanned citizens remain idle after finishing their manually started action. Interruption is not implemented.
- Planning starts from hunger, tiredness, and wealth goals. Actions describe inputs and effects; missing resources are obtained by recursively finding supplier actions, rather than enumerating every primitive ordering or defining recipes.
- Hunger targets a full meal, with an immediate smaller meal also considered if food is already available. Gathered resources use average yields; trading uses the same prices as execution.
- Each backward goal search has a four-hour limit. One action may cross it, but no more prerequisites can be added afterward; incomplete branches are discarded. Sleep can take eight hours.
- Simulate alternatives forward and retain the best average completion-wellbeing variant per goal. Explore subsequent goal choices from its predicted state. This is a local heuristic, not a global optimum over primitive actions.
- Goals may start while the combined plan is under four hours. Complete the entire final goal, even beyond that horizon. Score the full plan using average wellbeing across all primitive action completions, without time weighting.
- Eat has a four-hour prediction-local cooldown; Buy berries and Sell pebbles have separate two-hour cooldowns. All start at completion, carry across goals, and reset on each new plan. They do not restrict manual execution.
- Execute the flattened primitive actions for a two-hour commitment, completing the current primitive action before replanning. A goal sequence can span the commitment boundary, but the remaining steps are forecasts.
- Before a non-first Eat, the replan check triggers if actual food provides less than 20 nutrition. First actions are exempt. Empty meals/trades are skipped and an exhausted sequence causes replanning. Already-running actions are not interrupted.

- Actual forage yields can change meal sizes and durations. Follow the remaining plan unless the replan check triggers, the time commitment is reached, or the action list is exhausted.
- The citizen stores an optional `ActivePlan`, available through `active_plan()`, with the selected plan, current action index, and elapsed batch time including partial actions.
- Continue immediately into the next committed action or batch, accounting for all elapsed time even when a tick crosses multiple boundaries.
- Prediction does not advance other agents or the world clock. Actual universe advancement still advances every agent.
- Each citizen snapshot owns its forage random generator. Cloning preserves the stream, so branches can advance independently and tick partitioning preserves yields. Prediction always uses average yield and leaves the random stream unchanged.
- Numeric errors during search or execution return an error and preserve the original snapshot.
- Future predictions can use assumptions, such as the price last paid to hire workers. Prices are expected to fluctuate slowly enough for this to be useful.
- Future execution failures caused by world interactions can trigger replanning.

## Time and ticking

- Time is measured in ticks, with milliseconds represented by a `u64`.
- A universe has a current time.
- Advancing a universe returns a new snapshot, increments its clock once, and updates each citizen's needs and action for the elapsed duration.
- Agents delegate advancement to their kind. Citizens advance their own needs and actions without requiring a universe; each advancement returns a new value and preserves its source.
- If an advance crosses action completion, advance needs and continuous recovery to that point. Unplanned citizens spend the remaining time idle; planned citizens continue their committed actions, replanning at the first action completion at or after two hours, even when sleep crosses that boundary.
- Invalid advances, including clock overflow or nonfinite need results, leave the source snapshot unchanged.
- Small advances should agree with one equivalent large advance within floating-point tolerance when no intervening action changes the rate.
- Ticking a single agent advances its needs and actions by a given amount of time within the universe.
- Ticking the universe ticks all agents.
- The desktop starts paused and maps one real second to 60,000 simulation milliseconds at 1x. Speed controls multiply this by 1, 2, 3, 5, 10, or 20.
- A single simulation worker measures real elapsed monotonic time and starts at most 60 automatic updates per real second by default. The cap is configurable with `LEARNING_LORD_MAX_UPDATES_PER_SECOND`, including 30. Processing delays produce larger time jumps while preserving fractional milliseconds.
- Paused time is excluded. An in-progress update finishes when pausing, and unapplied running time is retained until resume. Manual stepping while paused advances exactly 30 simulation minutes independent of speed.
- The desktop displays elapsed days, hours, minutes, and seconds from the latest completed universe snapshot, plus berry grams, hunger, tiredness, wellbeing, current action, upcoming actions, and remaining commitment time. The latter is not an exact replanning forecast because meal durations vary and the replan check can trigger earlier.
- Planned citizens start the next action at completion rather than losing time to tick boundaries.
- Individual agents do not need activating in the middle of an action.

## Negotiation

- Agents can offer work for others to accept.
- A family that needs a new building calculates how much construction would be worth to it, then puts out an offer of work.
- An agent can also offer work directly to another agent for consideration.

## Goods, orders, and prices

- Goods have a global price.
- Agents place sell orders for goods they own and buy orders for goods they want.
- Purchases go to whichever buyer gets there first.
- Prices are updated daily from supply and demand.
- When goods sell out or demand exceeds supply, the price of the good or service rises.
- If demand is close to supply, prices remain stable; otherwise, prices drop.
- When buys exceed sells, the increase is proportional to the difference. For example, 110 buys against 100 sells might raise the price by 1%; 120 buys against 100 sells might raise it by 2%.
- Price increases have a cap to prevent spikes at small quantities.
- Purchases are tracked throughout the day, and prices rise or fall based on those totals.
- After prices are recalculated, all orders are cleared and agents create new sell orders for the day.

## Work and compensation

- General going rates allow trades to be assigned a price.
- Some collectives offer members food and board alongside a usually reduced money payment in exchange for work.
- These arrangements will often be a better deal than renting a room and buying meals, even before considering relationships.
- Some businesses simply pay staff money for their work.
- In exchange for an agreed number of labour hours, a family provides members with food, board, and pocket money, and ensures the actions needed to provide food are taken.

## Skills

- Citizens have skills.
- Some tasks take different amounts of time depending on the citizen's skill.
- Hauling is fairly universal.
- Carpentry is very slow for an unskilled citizen.

## Trading

- Buy berries and Sell pebbles each take five simulation minutes. Needs continue growing.
- Sell pebbles sells all held pebbles at 2 coins/kg. Buy berries tops up to 100 grams at 1 coin/kg, buying less if funds are insufficient and never borrowing.
- Quantities and payment are fixed from inventory and coins at action start. Goods and coins transfer only on completion, preserving total market-valued wealth apart from floating-point rounding.
- Predictions use identical trade rules. Empty trades are skipped without consuming time, including during plan execution. Actual quantities may differ after random gathering; the existing low-food replan check still applies before later meals.
