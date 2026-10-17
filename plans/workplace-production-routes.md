# Complete-batch production methods at one workplace

Compare two production methods for the same complete final batch: buy every direct ingredient, or manufacture every eligible intermediate using this worker at this physical workplace and buy the remaining leaves. Method estimates ignore current raw/intermediate inventory and cash. This avoids changing the preferred method merely because hypothetical intermediate stock accumulates during projection.

## Implementation step

- Build finite method estimates from current market listings and aggregate leaf purchases before rounding actual seller payments. Require enough priced supply for the complete method; unavailable supply has no reference-price fallback within this estimate.
- Include every intermediate batch, skill-adjusted work, net taxed intermediate output and workshop fee. Count final revenue once; consumed intermediates earn no separate revenue. Any unconsumed intermediate surplus retains existing reference valuation.
- Compare profit per total production hour. Current cash does not invalidate the comparison: actual procurement separately limits spending to real missing supplies.
- Use actual accessible owned ingredients first when scheduling and executing work. Four blocks reduce remaining manufacturing; eight allow immediate assembly even when the complete manufacturing method is preferred.
- Leave ordinary stock-ratio scheduling responsible for intermediate work. Add no supplier jobs, route commitments, reservations or preference flags. Defer a final requiring preferred manufacturing while its suppliers remain ordinary eligible; otherwise retain direct-input fallback behavior.
- Derive purchases from the ordered production projection, including only net intermediate output produced before consumption. Keep the two-hour shortage threshold and twelve-hour restock target.
- Label diagnostics as complete-batch method estimates independent of inventory/cash, with linked goods/workplaces. Report actual ordered target requirements separately.
- Verify method invariance across inventory/cash changes, finite supply, rounded depth, taxes, skills, owned inputs, actual purchase budgets and chronological requirements. Check both historical Finn saves through supported universe planning and obtain a fresh independent review before committing.

## Boundaries

Future work by other workers, other workplaces, crops and fermentation cannot supply a method. Existing accessible household inventory remains usable. Existing direct-input economics, speculative trials and unavailable-supply demand estimates remain fallback behavior when no distinct complete manufacturing method can be quoted. Market references, wealth valuation, wellbeing scoring and planning horizons remain unchanged. Profitable complete manufacturing does not make an otherwise rejected intermediate recipe attractive.

## Acceptance evidence

The superseded draft compared only missing inputs. On the older day-14 save that switched to buying as projected blocks accumulated: `[make block × 3, assemble garment] × 6`, resulting in thirty imported blocks and 450 g cloth bought up front. The revised complete-batch comparison prevents that inventory-driven switch without retaining a route choice across replans.

Checks on unchanged historical saves, using in-memory clones with Finn's current action/plan cleared and then the supported `Universe::start_planning` path:

- Day 14: the twelve-hour projection makes twenty-two blocks and two garments, with 550 g cloth and zero imported blocks. The short executable candidate buys that cloth and makes three blocks; it does not yet assemble a garment. Production wins this snapshot at 321.41 versus the prior sleeping choice at 314.19. This is one observed decision, not a guarantee of later garment completion.
- Day 16: the tailory projection also makes twenty-two blocks and two garments with no imported blocks. Listing excess remains the immediate selected goal, so no root production candidate is claimed for this decision.
- Method estimates are identical with zero, three or four held blocks in the saved-state probe. Both source universes and fresh plans load and round-trip successfully. Original save files were not changed.

## Validation and review

- Workspace tests: 660 passed, seven ignored. Focused regressions include complete-method equality across zero/four/eight blocks, zero/200 g cloth and zero/1,000 coins; actual owned-input readiness and reduced work; cash-limited purchases of real shortfalls; insufficient full-method supply fallback; finite depth, seller rounding, taxes, skills, fees and chronological requirements.
- Diagnostic tests cover explicit unavailable estimates, complete-batch wording and typed goods/workplace links. Per-site projections reuse the target calculation already performed for that candidate; UI refreshes perform no production projection.
- Formatting, whitespace checks, workspace build and Clippy with warnings denied pass.
- Fresh independent review of the complete-batch implementation: approved with no high, medium or low findings.
- Supported-path performance on the same older-save clone: three alternating warmed benchmark runs, twenty-one samples per version. Baseline `16eddaa` median 12.18 ms, p95 13.82 ms; revised median 15.01 ms, p95 18.09 ms. The isolated baseline uses the same optimisation level, debug assertions and dependency artifacts. The selected goal changes from sleep to production, so these measurements include changed search behavior as well as method estimation; they are a small local sample.

The single implementation step is complete and independently reviewed.
