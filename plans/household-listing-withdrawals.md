# Retrieve owned listings across locations

## Scope

- Household and individual provisioning must consider their own listings at every location before purchasing replacement goods.
- Withdraw at the listing's actual location, including travel and the existing withdrawal duration. Retrieve nearby owned supplies first and buy only the remaining shortfall.
- Apply the same rule to production ingredients, food, drink, clothing and reserve replenishment.
- Preserve ownership checks, physical inventory transfers, quote limits and existing action/replanning rules. Withdrawing owned stock costs no coins and is not a sale.

## Evidence

- Latest reported save: `universe-1792073216.384572422.ron.gz`, day 24 at 04:00.
- Bram and Dara own 3,060 g of flour listed at Bram's mill, mostly around 106.72 coins/kg. The household repeatedly buys caravan flour at 239.6875 coins/kg instead.
- `Citizen::withdrawable_units` checks only `selling_place()`, which is Dara's bakery. `Withdraw` also fixes its destination to that location. Household flour at the mill is excluded from withdrawal and correctly excluded from external purchases, leaving caravan stock as the apparent source.

## Work unit

1. Make withdrawals location-specific and share the owned-listing source lookup across provisioning. Update execution, validation and presentation as required. Add regressions, independently review, resolve findings, verify and commit.

## Verification

- Household flour is retrieved from the mill for the bakery without paying for replacement flour.
- Multiple owned listings and locations are considered, travel is included, and only an actual shortfall is purchased.
- Other accounts' goods cannot be withdrawn. Withdrawal does not create trades or consume coins.
- Food, drink, clothing and reserve provisioning use the same ownership/location rule.
- Save round trips and action validation; formatting, appropriate simulation/desktop tests, build and strict Clippy.
- Inspect the reported snapshot through an outside-repository diagnostic copy if a strict save-version change requires it. Original saves remain untouched.

## Status

- Implementation complete and independently reviewed; preserve the unrelated user edit to `balanced-diet-fishing-cabbage.md`.
- Added location-specific withdrawals while retaining the existing action for saved plans and local withdrawals. Version-8 saves remain loadable.
- Review identified and resolved a spare-garment purchase bypass and inconsistent canary bookkeeping on partial withdrawals. A fresh independent review found no high or medium issues.
- Actual reported save replayed for 24 hours: Dara's plan visits Bram's mill to withdraw flour, household listed flour drops from 3,060 g to 2,860 g, and the household purchases no flour. The resulting universe saves and reloads successfully; the original save is untouched.
- Simulation tests: 507 passed, six ignored manual tests. Desktop tests: 141 passed, one ignored manual test; the unrelated ten-day town smoke test was excluded in favor of the reported-save replay. Build, formatting, strict Clippy and diff checks pass.
- The restocking affordability prefilter retains its existing approximate purchasing origin. Actual acquisition routes still evaluate retrieval and buying; improving the estimate is outside this fix.
