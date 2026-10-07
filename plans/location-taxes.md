# Location taxes and town storage

Implement the agreed first tranche: weekdays, a zero-balance town treasury, independently owned goods stored at locations, and player-configurable private-location taxes. Employment, public-location taxation and warehouse hauling are deferred.

## Steps

1. Add calendar and storage foundations, a warehouse, and town ownership/account state. Validate ownership, location references and whole-unit transfers. Review, verify and commit.
2. Add stacked location tax rules and authoritative collection: weekly flat fees and goods asset assessments, production socage, and income tax at the sale location. Preserve fractional obligations, accrue unpaid coin taxes as arrears, and leave production valuation gross. Review, verify and commit.
3. Add desktop location/storage/tax controls and treasury visibility, weekday readouts, export coverage and documentation. Run workspace checks, inspect the native interface, obtain independent review and commit.

## Rules

- Monday is day zero. Desktop universes start at 06:00; weekly settlement first occurs on day seven at 06:00 and repeats weekly.
- Stored goods have an owner independent of the property's owner. Carried goods remain distinct. Collected goods remain at their production site under town ownership.
- Multiple rules may apply at a private location; no active rules is Frankalmoigne. Percentage rules select goods and rates independently.
- Socage applies at production completion, asset tax assesses stored goods at current reference prices weekly, and income tax applies to gross sale revenue at the sale location. Flat fees name a responsible citizen.
- Stacked rates apply to the original taxable amount. Total socage for a good cannot exceed 100%. Fractional obligations carry across collections, with whole coins or goods transferred only when a whole unit is owed.
- Taxes do not reduce predicted production profitability or targets. Authoritative collection changes the actual state used by subsequent plans.
- Coin taxes cannot create negative spendable balances; unpaid amounts remain owed. The town does not tax itself. Building values are excluded.
- New universes start with no tax rules and zero treasury coins.

## Validation

Cover calendar rollover and multi-week jumps; immutable snapshots; owner-isolated storage; production and sale collection; stacked rates and fractional carries; insufficient funds and arrears; rule edits/removal; prediction isolation; and worker/UI command and restart behavior. Final checks include workspace tests, formatting and strict Clippy.

## Progress

- Step 1 complete: calendar helpers, town warehouse/treasury, persistent owner-separated location stock, local transfers and stored-wealth accounting. Transfers preserve stationary work and protect its inputs. Stored stock does not count as immediately accessible food or ingredients. Owner/good totals are indexed to keep stock scanning out of prediction's wellbeing calculations.
- Step 1 validation: simulation tests passed, all 47 desktop tests passed after updating warehouse map counts, strict workspace Clippy and formatting passed. Independent Sol-6.1 review has no remaining findings.
- Step 2 complete: configurable stacked private-location taxes, authoritative production/sales collection, weekly asset and flat assessments, fractional accounts, archived obligations and bounded receipts. Taxed stock remains at its production location and affected speculative planning requests are cancelled.
- Step 2 validation: all simulation tests passed, including ten tax integration tests for fractional collection, stacked batch limits, multi-week settlement, ownership/escrow assessment, rule changes, gross predictions, and rollback after collection begins. Independent Sol-6.1 static review found no issues; final workspace checks follow the desktop integration.
