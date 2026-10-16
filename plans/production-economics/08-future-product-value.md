# 08 — Value ingredients through their eventual product

## Hypothesis and scope

Account for the future product enabled by input stock. Directly treating cloth or blocks as completed garments is rejected: it counts unpaid labour as wealth and may count the same flour/wood toward several products. A more defensible variant is bounded continuation value separate from displayed wealth.

## Proposed experiment

- Keep ordinary inventory wealth. For a diagnostic only, compare feasible immediate next batches across accessible workplaces and choose at most one per account, using actual available ingredients and cash.
- Measure unsold output reference value minus consumed owned-input reference value, actual missing-input purchases and incremental fees/taxes. Do not yet add a planning benefit; discount, remaining-work deduction and terminal weighting remain an integration gate.
- Each candidate has an isolated feasible resource ledger; choose only one batch, so no resource funds multiple credited batches. Do not combine a future terminal bonus with a rollout that already realises the same output.
- Use only the agent's actual skills/facilities. Include remaining fermentation/growth and care feasibility if such outputs are admitted; simplest first experiment excludes these recipes explicitly.
- Before any future score integration, compare alternatives at the same boundary and demonstrate that the bonus does not favour shorter candidates merely because they end before production costs are incurred.

## Critique

The hard part is the value of remaining labour. Zero makes unfinished work look earned; a fixed hourly deduction can reject valid work or ignore alternatives; deriving it from the agent's best recipe creates a recursive opportunity-cost problem. The original 'just count final products' shortcut is therefore not simple.

With a future terminal bonus but no completion deadlines or decay, an agent could hoard ingredients, receive the benefit, and repeatedly postpone the work. A later multi-batch extension would also face biased greedy allocation of scarce shared inputs. A terminal bonus for garments could double-count future clothing wellbeing. Counting profit while ordinary score already values intermediates needs a precise accounting identity.

## Implementation plan if chosen

1. First compare manually completed one-batch rollouts with current terminal wealth, without changing agent choices. Quantify the missing continuation value.
2. Specify labour deduction, discount, cap, common boundary and input allocation before implementing any bonus. Treat unresolved choices as a design gate, not programmer discretion.
3. Add single-counting, repeated-replanning/hoarding, unfinishable recipe, competing inputs and needs tests; compare against plan 03's explicit continuation before adoption.

## Evaluation and recommendation

Defer. It could represent strategic value, but the simple wording hides the most difficult new economics. Prefer actually evaluating useful continuation or offering a smaller procurement before inventing a terminal estimate. The original direct final-product valuation has no implementation recommendation; this document defines the safer research variant and its unresolved gate.

## Review resolution: accounting first, integration remains gated

For the initial diagnostic choose exactly one feasible immediate batch per account, not one per workplace. Enumerate finite supported site/recipe candidates, simulate real preparation and production on independent clones, and report the best feasible batch deterministically; do not combine batches or allow future sale proceeds to fund inputs. Existing inventory, cash, listing quantities, taxes, fees, travel and skills must support it. Exclude crops/fermentation suppliers and any recursive production-chain search.

Report an unsold-output outcome: output reference value minus consumed already-owned input reference value minus actual missing-input cash expenditure minus incremental fees/taxes. Do not additionally subtract the purchased inputs' reference cost. Previously booked liabilities are not charged twice. Compare against the simulated balance-sheet delta to avoid deducting tax outputs twice, recording incidental consumption separately to explain any residual. Report an actual sale separately only if executed; expected seller ask is not an unsold asset value. The distinction is material: one block at 12.50 plus seven imports costing 132 can look positive against 147.41 sale revenue but negative against 139.22 unsold output reference.

No terminal bonus enters agent selection in this first step. Before any integration, settle a common boundary, explicit coin-to-wellbeing conversion, terminal weight independent of action segmentation, and a remaining-labour/needs treatment. A symbolic test must show that buying just before the boundary cannot outperform doing the same work by retaining an unearned bonus, and repeated replanning cannot repeatedly claim it. These are unresolved design decisions, not implementation discretion. Until then the high-risk scoring proposal is deferred, not approved.

A future joint batch estimator would need one cash/listing/time/ingredient ledger and a finite continuation budget. The one-batch diagnostic intentionally avoids that scheduler. Full accounting and labour-score findings are resolved as scope boundaries for the diagnostic, not claimed solved for the deferred integration.
