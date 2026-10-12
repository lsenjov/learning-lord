# Businesses and employment — deferred design

## Status and priority

- Discussion notes, not an implementation specification or authorization to build.
- Revisit buy/sell orders first. Return to this design once the market work is settled.
- Wage costs must affect business production profitability. Other details below are proposals to confirm when work resumes.

## Business and employee accounts

- A business owns its coins, ingredients, workplace stock and finished products.
- Employees contribute skills and labour. Wages go to their personal or household economic account.
- Employment grants access to the relevant workplace and permission to use business supplies for assigned work; it does not permit taking or personally selling those supplies.
- Employees can also be owners, but employment alone grants neither ownership nor household-style allegiance.
- Use the existing production profitability and stock-ratio policy to choose business production, with employee labour costs included.

## Ownership and capital

- Employees benefit through wages; owners benefit through their ownership stake and cash distributions.
- Support fixed ownership shares at business creation, potentially held by individuals, households, guilds or the town.
- Keep capital contributions, ownership shares and distributions distinct.
- Example: owners with 75% and 25% shares receive 75 and 25 coins from a 100-coin distribution.
- Initial capital provides business coins and goods. Decide how contributed goods are valued before deriving shares from contributions; explicit starting allocations are a simpler prototype option.
- Defer share trading, new share issuance, loans and negotiation.
- Additional funding must eventually have an explicit meaning: a loan, new equity or a contribution without changed ownership. It must not silently alter ownership percentages.
- An owner's wealth should include their share of business net assets. A distribution reduces business assets and increases recipient coins; it must not create a second wellbeing reward for the same wealth.
- Decide valuation and ownership-cycle constraints before supporting businesses owning other businesses or collectives that own each other.

## Employee planning and business maintenance

- Proposed first version: employment offers work alongside the employee's other goals, without mandatory shifts or a promised number of hours.
- The business supplies a production sequence for its workplace. The employee evaluates wages, elapsed time and changing personal needs.
- Business production decisions use business inventory, funds and profitability; employees do not receive the finished goods personally.
- Employees may also restock ingredients and list excess business goods using the business account.
- Coordinate maintenance at business-account level using the existing shared claims and cooldown approach; multiple employees must not independently duplicate the same task.
- Required travel and maintenance should be paid work as well as production. Define assignment boundaries carefully so personal activities are not charged to the business.

## Wages and ability to pay

- Proposed first version: a configurable fixed hourly wage agreed in the employment contract.
- Use 10 coins/hour, the production core-price labour baseline, as a provisional starting offer. Current market prices may still make work unprofitable.
- Carry fractional earnings forward and settle whole coins, following the existing currency model.
- Reserve the expected wages before starting an assignment. Do not begin an assignment the business cannot fund; allow an already funded assignment to finish.
- Confirm reservation duration, release on cancellation and treatment of unexpectedly longer assignments before implementation.
- Do not implicitly extend the guild's unpaid-fee policy to unpaid employee wages.
- Defer automatic wage negotiation. A later system could raise offers when profitable work remains unfilled and reduce offers for future contracts when labour costs make work unprofitable. Existing contracts retain their agreed wage.
- Automatic wage setting must consider business affordability, employee alternatives and unmet staffing needs; recipe profitability alone is insufficient.

## Production profitability

- Batch profit = expected output proceeds minus ingredient costs, labour costs and other applicable production costs, including taxes and workshop fees.
- Labour cost = agreed hourly wage multiplied by paid hours.
- Use the assigned employee's skill-adjusted production duration. Equal hourly wages can result in different unit costs for different employees.
- Recipe ranking includes wages for direct production work.
- Complete assignment estimates also include wages for required travel, ingredient collection and selling, without charging the same work twice.
- Recipes without material inputs still incur employee labour costs. Their local/export profitability must cover wages too.
- Employee planning values wages and personal needs; business planning values the resulting profit. Owner returns come after business costs.

## Retained funds and distributions

- Accounting profit is not necessarily spendable cash: unsold output and funds needed for the next production cycle must be distinguished.
- Proposed weekly policy: retain a configurable operating cash reserve, then distribute excess cash according to ownership shares.
- Initially estimate the reserve from one day's planned ingredient purchases and wages, accounting for the workers being supplied. Reuse production estimates rather than adding a long simulation lookahead.
- This is a cash-surplus distribution policy, not a rule to distribute all accounting profit. Decide whether unused initial capital may be distributed under it.
- Show retained funds and distributions in business history. Handle indivisible-coin rounding without creating or losing coins; choose the allocation rule when implementing.

## Suggested first observable prototype

- A town-owned bakery employing Dara, with separate business funds and ingredients.
- Dara remains in her household, which receives wages. Business distributions go to the town treasury.
- Confirm initial capital, property transfer, wage and operating reserve before creating this setup.
- Display estimated profit after wages and actual employee work-hours, as well as cash, output and distributions.
- Defer general hiring/firing, shifts, board and lodging, and a share market.

## Resume after buy/sell orders

- Recheck how business purchasing, listings and cash reservations should use the revised order system.
- Confirm the employment and ownership proposals above, especially optional work, funded assignments and cash distributions.
- Split implementation into small observable work units and discuss each before implementing.
