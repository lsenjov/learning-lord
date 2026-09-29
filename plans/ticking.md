Okay, so my current thinking on how I want to structure things

A universe is a game state, made of immutable data structures.
A universe has agents. Agents may be Animals, Citizens, or Collectives (collectives include families, guilds, businesses, foreign traders, and kingdoms)
Agents may have loyalties/relations to each other

Agents have needs. Needs may increase over time (hunger, energy), or may be static (clothing need. Is constantly at e.g. 40 for a peasant. Peasant clothing may reduce this by 50. As the clothing degrades over time, the amount it reduces decreases, so over time the need ticks up).

An agent has a personal wellbeing score - calculated from needs and other attributes. Hunger over 0 means it's time for a meal. Hunger over 100 means they're starving and will count heavier. Sleep and clothing are important. Total wealth is a secondary calculator. For collectives, the needs of their members might be most important, or maybe money is the only thing important to them.

An agent has actions. Each action has an estimated time, and a result. When choosing actions to take, it selects from available actions, then creates universes for each of these outcomes (the universes are immutable and designed to be quickly iterated on), and continues to do so until a certain number of time has passed for each. The wellbeing score after each action is recorded. The plan with the highest average wellbeing score at the end is chosen.
Other agents are frozen while predicting. We can make some assumptions (we know the price we paid last time for people to work for us) and the price shouldn't fluctuate so much that this is a problem.
If a plan tries to execute and fails, we can recalculate then.

Time is calculated though ticks, in milliseconds in a u64. If we ask a single agent to tick, it will advance that agent's needs and actions a certain amount of time in the universe. If we ask the universe to tick, it will tick all agents. Generally, ticks will be less than a second. If an agent is idle for a small time because they finished their task, that's not a problem.
A universe has a current time. Individual agents don't need activating in the middle of an action.

Agents have loyalties to each other. In some cases (like families) loyalties start high. Otherwise, loyalties grow as they work together, and decrease as one takes advantage of another.

Agents negotiate. A family needs a new building built, and calculates how much it would be worth to construct it. Then it puts out an offer of work, which other agents can accept. Alternatively, the agent can off work directly to another agent for them to consider.

Goods have a global price.
Agents put out sell orders on goods they own, and other agents put out buy orders on goods they want to buy.
More buys then sells? Price rises proportional to the difference (110 buys and 100 sells might raise it 1%. 120 buys and 100 sells, 2%), with a max cap to prevent spiking at small numbers.
Prices are updated from supply and demand daily. When goods are completely sold out, or demand exceeds supply, price of that good/service rises. If demand is close to supply, they remain stable, else prices drop.
Purchases are tracked throughout the day, and prices rise or fall from these general totals.
After prices are recalculated, all orders are cleared. Then, agents create new sell orders for the day.

Buys are done by whichever agent gets there first. If a buyer could buy from multiple sources, they choose the seller they have the highest proportion of loyalty to.

Loyalty modifies the wellbeing calculation. By default, an agent has a loyalty to themselves of 1. If they have loyalty to their family of 0.5, then 2/3 of their wellbeing is their own needs, and 1/3 is personal wellbeing of the family. This shouldn't cascade (one jump is enough).

As we know the general going rate of things, we can put a price on trades. Some collectives will offer their members work for food/board on top of (usually reduced) money. This is often going to be a better deal than renting a room/paying for meals, even without factoring in loyalty.
Some businesses will just be paying their staff to get work done for them.
In exchange for X hours of labour, a family will give food and board to their members for some pocket money, and will ensure actions are taken for the food.

Citizens have skills. Some tasks are done at different rates depending on how skilled the user is. Hauling is fairly universal. Something like carpentry is not and will be incredibly slow if the user is unskilled.
