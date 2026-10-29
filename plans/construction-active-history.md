# Active construction and daily labour history

## Agreed accounting

Construction quotes currently combine materials and labour into one integer payment, paid only on completion of a portion. Failed portions pay nothing, including for work already done. The existing records cannot reconstruct actual work durations/dates or separate material value from labour payments.

Implemented accounting: preserve each new quote's material component at posting (including owned wood); on successful settlement split the actual payment using that component, and distribute the labour remainder over the contract's recorded work days in proportion to actual milliseconds worked. Failed work retains hours with zero earnings. In-progress work has hours and no earnings until settlement, at which point previous work days update. Migrated active contracts lack a recorded material split and complete work-day allocation, so their hours can be recorded from migration onward but their effective price must remain unavailable rather than fabricate a full hourly result. Daily price remains unavailable while any contributing work has pending settlement or unknown provenance. Market days begin at 04:00; retain 365 completed days and the current day. Version 16 loads and validates version 15 before migrating it.

## Step 1: Implement and verify the coherent change

- Persist a monotonically increasing contract sequence, compact latest outcome per unfinished project, and per-stage settled progress and existing daily financial accounting. Replace closed-contract dependencies in reservation, continuation, planning restart, validation and UI.
- Keep only open contracts and unfinished projects after settlement; remove their detailed payment records while retaining existing household/guild cash histories. Validate legacy state before migrating it, then validate compact state using explicit accounting rather than reconstructed closed contracts.
- Capture actual elapsed construction work across calendar-day boundaries (using action duration, including skill reduction). Retain 365 completed market days plus the current day of daily totals and only necessary active-contract day allocations. Record zero-work days without an effective price. Separate confirmed labour earnings from material reimbursement using the agreed accounting above.
- Migrate old saves without fabricated labour history. Preserve unfinished stage progress, installed material, latest outcome, escrow and cash accounting. Check the day-50 save.
- Add a compact carpenter market overlay with actual daily hours and labour price per actual hour, with explanations in tooltips only.
- Update affected tests for removal of settled records; add focused tests for failed/partial/reposted contracts, cross-day and skill-adjusted work, retention, zero work, migration, validation and the UI model.

This is one cohesive implementation step because lifecycle, persistence validation and desktop consumers must change together. Commit follows implementation, verification and independent review.

## Status

Step 1 complete. Independent simulation and desktop reviews are clear after fixes and fresh reviews. The review fixes cover linked-good navigation leaving Carpentry and rejection of missing retained work summaries before settlement. The low empty-state wording and missing migration-provenance issues were also fixed.

- Simulation: 446 passed, 0 failed, 6 ignored.
- Desktop: 171 passed, 2 pre-existing failures, 1 ignored; includes the ten-day town simulation and all new market/navigation cases. The failures are `details_distinguish_current_goal_from_forecast` and `decision_panel_shows_reachable_hunger_and_uses_saved_prices`: their old multi-goal expectations predate first-feasible-goal planning (commit `1492b5c`). No unrelated tests or production behavior were changed.
- Final desktop development build and diff whitespace checks pass. Native Carpentry empty-state layout checked in an isolated app using a copied save. Pending/settled data behavior covered by tests.
- Original day-50 save loads through migration with 372 open contracts and 397 unfinished projects, down from 13,697 contracts and 1,998 projects. After six simulated hours it saves, reloads and advances another hour successfully.
- Same-fixture headless development benchmark: six simulated hours took 45.387 seconds before and 9.149 seconds after (about 5x faster). Input reading, loading, saving and rendering are excluded. No competing build/simulation workload ran during the final measurement. First migration load took 8.102 seconds; subsequent compact-save reload took 7.225 seconds. These measurements describe this fixture, not desktop frame rate.
