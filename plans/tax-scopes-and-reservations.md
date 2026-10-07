# Shared tax rules and weekly Socage reservations

## Approved scope

- Tax rules target one private location or all private locations of a type, including future matching locations. Both scopes stack. Socage totals for a good cannot exceed 100% at an affected location. Existing goods filtering and per-good rates remain.
- Socage reserves whole goods during production, carrying fractional obligations forward. Reserved goods remain agent-owned and included in wealth/wellbeing, but cannot be consumed, equipped, sold or used as ingredients. Goods and reservations can move without losing their identity.
- Sunday at 04:00 is weekly tax time, first on day six. Reserved goods become town-owned wherever they are, including when carried, before private goods asset assessments and weekly flat fees. Income tax remains on actual sales. No automatic warehouse transport.
- Production profitability and targets use expected retained output after combined Socage, minus full ingredient costs. Predicted wellbeing does not discount or confiscate output for future Socage. Subsequent plans start from the actual usable inventory and reservations.
- Market raw-material margins use recipe quantities and reference prices: (output value - material cost) / output value. Omit recipes with no material inputs, retain negative margins, and exclude taxes/time/travel/purchase rounding.
- New tax names default to the scope and tax kind, remaining editable. Type-wide rules and individual rules are visibly distinguished.
- Employment, public-location taxation and hauling remain deferred.

## Commit steps

1. Implement backend tax scopes, type-wide flat-fee owner resolution, per-location obligations, aggregate rate lookup and validation, including new locations. Verify, independently review and commit.
2. Implement reservations, independently owned carried goods, safe use/transfer restrictions, Sunday settlement ordering and after-Socage production profitability. Verify accounting, planning and history, independently review and commit.
3. Implement location-type UI, inherited/local rules, default names, reservation/state-owned stock readouts and market margins. Update exports/docs, run workspace checks and inspect native layout, independently review and commit.

## Validation

Cover both scopes together and future locations; owner-resolved flat fees and retained debts; fractional Socage and whole-item availability; agent wealth before/after Sunday; reserved goods moved between carried and stored stock; state ownership while carried; order of asset assessments; predictions and real execution; profitability versus gross wellbeing; price-based material margins; UI editing/naming/generation handling. Keep immutable snapshot, cancellation and overflow guarantees. Use targeted tests per step, final workspace tests, formatting and strict Clippy.

## Progress

- Step 1 complete: shared `TaxScope` rules, named or property-owner flat-fee payers, combined-rate validation and lookup, and per-location tax accounts. Existing individual rule APIs remain available. Matching sites settle in stable ID order so limited balances produce deterministic debts and receipts.
- Step 1 checks: all workspace tests passed, strict simulation Clippy passed, and independent Sol-6.1 review has no remaining findings. Desktop changes in this step only migrate the existing interface to the scoped API; the type selector follows in step 3.
- Step 2 complete: Socage reservations retain private ownership and wellbeing until Sunday 04:00, then transfer ownership in their existing carried/stored custody before asset assessment. Actions and procurement use available stock; transfers protect active inputs and carry reservation identities with the goods. Net output values affect recipe profitability and target eligibility, with cached rates for prediction; predicted future output and wellbeing remain gross.
- Step 2 review corrected active-job transfer availability, a shopping baseline mixing available and gross stock, deterministic multi-owner settlement, and imported citizens with missing reservation ledgers. All simulation tests and strict simulation Clippy passed. Additional focused tests cover Sunday chunk/cancellation behavior. Independent Sol-6.1 review has no remaining findings.
- Step 3 complete: individual/type navigation, inherited/local rule labels, editable scope-based default names, owner-resolved type fees, Sunday schedule and reservation/cargo readouts, and raw-material margins in the market overview and good details. Exports include the revised state. Independent desktop review corrected converting an existing shared percentage rule to an owner-paid flat fee; regression coverage includes all three percentage tax kinds.
- Final validation: 333 workspace tests passed; two manual benchmarks remain ignored. Formatting, diff checks and strict workspace Clippy passed. Both backend and desktop reviews have no remaining findings. Native Locations and Market layouts were inspected, including the first Sunday collection label and visible material margins; keyboard navigation worked, while mouse injection did not support a full native type-editor click-through. Automated interaction and worker tests cover scope editing, inheritance, names and payer conversion. Unsupported new separator glyphs were reported and replaced with ASCII pipes. Only the task-owned native app was stopped; an existing app was preserved.
