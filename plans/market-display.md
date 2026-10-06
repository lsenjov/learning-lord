# Market display

Add a Market tab to the native desktop interface while preserving the simulation controls and Citizens view. Market quantities describe completed transactions plus current affordable requests and listed stock; no economic behaviour changes.

1. Expose read-only current and previous market-period statistics from existing accumulators and closing history. Cover 04:00 boundaries, inactive closed periods and fulfilled demand in tests.
2. Add Citizens/Market tabs, a seven-good price list, period selection, selected-good quantities and current sell orders grouped by seller. Preserve the native colors, scrolling and conditional UI writes. Test display data and interactions.
3. Run formatting, relevant tests and Clippy; complete native visual checks and independent Sol-6.1 review, fixing findings before committing.

Existing economy changes are pending separately and the town trade acceptance test is known to fail. Preserve that test and all unrelated work. The coordinator handles review, native QA and commits each completed step without including the pending economy draft.

## Progress

- Step 1: current and previous period statistics are implemented. Independent Sol-6.1 review found no issues. Tests cover fulfilled requests counted once, the first 04:00 close, period reset, and inactive periods without stale history.
- Step 2: Market/Citizens tabs, all seven goods, period selection and live seller orders are implemented. Three desktop tests cover quantity definitions, seller identity and partial orders, selection, restart and idle UI writes. Independent review found no remaining issues.
- Step 3: native checks passed at 1280×800 and 1000×700, including tabs, good selection, previous-period figures, live seller orders and scrolling. Paused CPU use was zero measured ticks over two seconds. Formatting and Clippy passed. The exact isolated display changes passed the full workspace suite; the combined working tree passes 164 tests with the pre-existing town-trading acceptance failure excluded and preserved.

## Follow-up: current buy requests

1. Show each citizen's current request for the selected good, including requested and affordable grams even when affordability is zero. Identify duplicate names by UUID, sort by name and UUID, and keep requests live in both period views. Reuse conditional text updates and distinguish requests from completed trades.
2. Cover buyer identity, affordability, good filtering, request changes and period selection in desktop regression tests. Run formatting and focused tests, then independent Sol-6.1 review and commit the isolated follow-up.

- Step 1: implemented the current buy request readout without backend changes.
- Step 2: all four focused market tests, formatting and Clippy passed. Native checks confirmed funded and cashless buyers, empty states, scrolling and live requests alongside previous-period figures. Independent Sol-6.1 review found no issues.
