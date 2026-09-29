# Ticking and agent behaviour

## Universe and agents

- A universe is a game state made of immutable data structures.
- A universe contains agents: animals, citizens, and collectives.
- Collectives include families, guilds, businesses, foreign traders, and kingdoms.
- Agents can have relationships with one another.
- Agent IDs are UUID v4s, generated when an agent is created and preserved when universes are cloned.
- Agent kinds hold their specific data. Citizens have metabolic hunger; collectives do not have metabolic hunger of their own.

## Numeric representation

- Continuous simulation measurements, including hunger, rates, and wellbeing, use `f64`.
- Simulation time and elapsed durations use `u64` milliseconds.
- Money uses signed `i64` values in a defined smallest currency unit.
- Discrete counts and tile coordinates remain integers.

## Needs

- Agents have needs.
- Some needs increase over time, such as hunger and the need for rest.
- Citizen hunger is a signed energy deficit: below zero is satiation, zero is the boundary, and above zero is a need for food.
- Hunger increases with elapsed simulation time. The standard rate is 100 hunger per 24 game hours, balanced by two meals of 50 nourishment each. Starting hunger is explicit; custom constant hourly rates remain available.
- Starting hunger must be finite. Hunger rates must be finite and nonnegative. Hunger is not clamped at zero or at the starvation threshold.
- Eating takes 30 simulation minutes and reduces hunger by 50 on completion. Hunger continues growing during the meal. Food inventory and availability are not modeled yet.
- Other needs have a constant baseline, such as a peasant's clothing need of 40.
- Goods can reduce a need. Peasant clothing might reduce clothing need by 50.
- As clothing degrades, its reduction becomes smaller, so the effective clothing need increases.

## Personal wellbeing

- An agent's personal wellbeing score is calculated from its needs and other attributes.
- Currently, `Citizen::personal_wellbeing()` calculates a hunger-only score on demand without a universe or stored score. `Agent::personal_wellbeing()` delegates to its kind.
- Higher scores are better. The formula is `-max(hunger, 0) - 4 * max(hunger - 100, 0) - max(-hunger - 100, 0)`.
- Hunger from -100 to 0 scores zero. Satiation delays future hunger rather than granting an immediate wellbeing bonus.
- Hunger below -100 represents being overfull, costing one point per excess unit with no jump at the threshold.
- Hunger above 0 costs one point per unit up to 100. Each unit beyond 100 costs five points, with no jump at the starvation threshold.
- A score outside the finite `f64` range returns `SimulationError::WellbeingOverflow`.
- Sleep and clothing will be important factors later.
- Total wealth will be a secondary factor.
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
- Currently, citizens can eat or wait; each action takes 30 simulation minutes. Only one action can be active at a time, and hunger continues growing throughout both.
- Starting an action returns a new snapshot without advancing time. Eating grants 50 nourishment on completion; waiting has no completion effect. Unplanned citizens remain idle after finishing their manually started action.
- Planning searches all 256 sequences of eight actions over a four-hour horizon, using independent citizen snapshots. Current actions need no world interactions; future actions can branch immutable universes when those interactions exist.
- A plan's score is the average personal wellbeing at its eight action completions. Wellbeing between completions is not included yet.
- The highest-scoring plan wins. Search tries waiting before eating and retains the first sequence when scores tie.
- Planning is enabled explicitly on an idle citizen with `start_planning()`, also available through agents and universes. The search and batch executor live in the separate planning module and reuse citizen action and need calculations.
- Execute the first two hours (four actions) of the selected plan without reconsidering between actions. On finishing that batch, search again from the actual state and start the next batch.
- Continue immediately into the next committed action or batch, accounting for all elapsed time even when a tick crosses multiple boundaries.
- Prediction does not advance other agents or the world clock. Actual universe advancement still advances every agent.
- Numeric errors during search or execution return an error and preserve the original snapshot.
- Future predictions can use assumptions, such as the price last paid to hire workers. Prices are expected to fluctuate slowly enough for this to be useful.
- Future execution failures caused by world interactions can trigger replanning.

## Time and ticking

- Time is measured in ticks, with milliseconds represented by a `u64`.
- A universe has a current time.
- Advancing a universe returns a new snapshot, increments its clock once, and updates each citizen's hunger and action for the elapsed duration.
- Agents delegate advancement to their kind. Citizens advance their own needs and actions without requiring a universe; each advancement returns a new value and preserves its source.
- If an advance crosses action completion, advance needs to that point and apply the action's effect. Unplanned citizens spend the remaining time idle; planned citizens continue their committed actions, replanning every two hours.
- Invalid advances, including clock overflow or nonfinite hunger results, leave the source snapshot unchanged.
- Small advances should agree with one equivalent large advance within floating-point tolerance when no intervening action changes the rate.
- Ticking a single agent advances its needs and actions by a given amount of time within the universe.
- Ticking the universe ticks all agents.
- Ticks will generally be shorter than a second.
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
