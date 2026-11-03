# Caravan controls and tariffs

## Agreed behaviour

- Each good has an export toggle (default enabled) and minimum town reserve (default zero).
- Count resident- and state-owned carried inventory, stored goods and local sell orders, once each. Include Socage reservations; exclude caravan-owned stock and already consumed/equipped goods.
- Exports are limited to the lesser of half the local listed quantity rounded up and town stock above the reserve. Preserve proportional seller allocation.
- Global and per-good signed import and export tariffs default to zero and add without clamping. Rates represent the state's share of the final gross payment, using basis points and fractional obligation carry. An effective tariff at or above 100% disables trade in that direction.
- Export threshold: `core * 0.5 * (1 - export tariff)`. Local asking prices represent seller proceeds before ordinary income taxes; caravans fund the tariff on top, within their buying threshold.
- Import asking price and threshold: `core * 1.5 / (1 - import tariff)`. Collect tariff at actual purchase, not delivery. Existing caravan listings update when the tariff changes; an effective import tariff at or above 100% must also prevent purchases of existing caravan stock.
- Example: with a 100-coin untaxed export limit and 50% tariff, the local cutoff is 50; a 40-coin local sale costs the caravan 80, paying 40 to the seller and 40 to the state. An untaxed 100-coin import costs the resident 200, paying 100 outside and 100 to the state.
- Integer payments retain per-seller/per-good rounding; split actual gross payments into treasury and recipient amounts without creating coins. Document fractional carry and rounding.
- Export sale income taxes apply to seller receipts, excluding tariff funds. Imports do not create income-tax accounts for fictional residents.
- Controls apply through the existing generation-scoped worker mutation mechanism. Keep snapshot predictions immutable and update displayed effective prices, reserves and tariff revenues.

## Steps

1. [x] Implement simulation policy, reserve accounting, tariff pricing and settlement, history and immutable mutation APIs. Full simulation suite passed; independent review approved the backend and settlement regressions.
2. [x] Add market controls and effective-price/reserve/revenue displays, update documentation and tests. Independent review approved; 362 workspace tests passed with two manual benchmarks ignored. After UI signature cleanup, all 84 desktop tests, strict workspace Clippy, formatting and diff checks passed.

## Validation

- Export off, exact reserve, below reserve, reserve-limited and half-stock-limited exports, multiple ownership/custody forms and order fragmentation.
- Zero, fractional, 50% and 100% tariffs; gross payment conservation; treasury income; per-good/direction fractional carry; ordinary income tax on net export proceeds.
- Existing import orders reprice immediately, respect affordability, and remain unavailable at 100%; restoring a lower tariff re-enables them.
- Daily counters, histories and external coin-flow labels distinguish gross payments, resident receipts and tariff revenue.
- Direct, partitioned and cancelled advances preserve state and deterministic settlement. Policy edits reject invalid state without corrupting snapshots.
- UI editing suppresses shortcuts, preserves drafts on failures, and cannot apply stale settings after universe restart.

## Settlement rounding

Export settlement first rounds the accumulated local asking value up once per seller and good, preserving the existing integer asking payment. For nonnegative tariffs, it computes gross payment as `ceil(net_asking_payment * 10000 / (10000 - tariff_basis_points))` with integer arithmetic. This keeps a seller's receipt at least as large as their rounded asking payment. Integer rounding can add a small extra receipt. For subsidies, settlement computes the minimum additional gross payment that covers the cumulative rounded asking value, accounting for receipts already paid and the current signed carry. Treasury affordability uses the same calculation.

Both directions split actual gross payment using truncation toward zero of `(gross * effective_tariff_basis_points + carry) / 10000` for the treasury and the remainder for the recipient. The fractional numerator remainder carries between purchases separately for each good and direction. Carry is preserved when policies change. Positive tariffs come from the actual gross payment; negative tariffs pay a town-funded subsidy. Settlement limits quantity to the shared available treasury before moving goods, with no debt or fallback. Export income tax and citizen earned-income history use recipient coins. Import costs and external export inflows use gross coins; external import outflows subtract treasury tariff receipts.

An effective tariff at or above 100% blocks its direction. A per-good import switch also blocks deliveries and purchases of existing stock. Existing import orders keep finite quotes and physical stock, but do not count as available supply or affect market price pressure until imports are re-enabled. Threshold getters return `None` for disabled directions. Policy mutation refreshes existing import quotes and citizen market forecasts immediately.

Signed policy extension: see `plans/signed-caravan-policies.md`. Good saves include imports, exports, reserve and both per-good tariffs; global saves preserve those drafts. Existing saves default missing per-good rates to zero and imports to enabled for eligible goods. At -25%, untaxed import value 100 costs the buyer 80 plus treasury 20; export asking value 100 receives 80 from the caravan plus treasury 20. Rates above 100% remain editable and can be offset by negative rates.
