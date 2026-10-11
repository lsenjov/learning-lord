# Guild workshops and hourly fees

## Agreed behaviour

- Add a Weavers' Guild with Eira and a second independent weaver, sharing Eira's existing weavery. Keep separate homes, economic accounts, production targets and goods ownership.
- Guild membership grants access to the shared workshop while preserving skill requirements, public customer access and private home restrictions.
- Members replenish their own workplace supplies under the existing two-hour trigger / twelve-hour target rules and sell their own output from the shared workshop.
- Charge one coin per actual production hour at the guild workshop, with fractional obligations carried between jobs. Travel, purchases and idle time are not charged.
- Pay the guild account; include workshop costs in production profitability and predictions. Guild membership does not pool goods or add household-style allegiance.
- Guild starts with zero coins and no goods. Owner-based property fees belong to the guild; production, income and goods taxes follow the relevant producer/owner. No membership dues or sales percentage in this step.
- Populate Collectives/Guilds with members, facilities, balances, fees and individually owned workshop stocks. Add typed guild links in citizens and locations.
- Persist guild ownership, membership and fee accounting with the existing strict save-version policy.
- Workers may continue working when they cannot pay. Collect available coins, accumulate unpaid fees and retry payment as funds become available. Outstanding fees reduce economic wealth so planning still sees the cost. Paid workshop fees are a separate household cash-history category.
- This prototype supports mills, bakeries, weaveries and tailories. Shared crop and brewing sites need separate deferred-production ownership work before guild access can be enabled.

## Step 1: simulation and accounting

- Add guild identity, membership, workshop permissions and collective account routing.
- Integrate fees into authoritative execution, planning and daily cash summaries without changing goods ownership.
- Cover separate stocks, simultaneous household/independent workers, fee carry and affordability, access, replenishment, trade, taxes and persistence.
- Independently review, resolve findings and commit.

## Step 2: starting guild and UI

- Create the starting guild and second weaver with the agreed supplies and one shared weavery.
- Show guild details, clickable membership/ownership and financial effects using the existing window system.
- Update documentation and relevant UI/startup tests.
- Run workspace tests, formatting, strict Clippy and build; independently review and commit.

## Progress

- Step 1 complete and committed as `a014171`; independent review and fresh review after fixes passed. Eight guild regression tests cover access, stock ownership, exact fee carry, shared accounts, debt settlement, tax ownership, persistence and prototype boundaries.
- Step 2 complete; independent review and fresh review after navigation/documentation fixes passed.
- Final checks passed: locked workspace tests, workspace build, strict Clippy for all targets, formatting and diff checks. Save format is version 5; earlier saves remain unsupported under the existing policy.
