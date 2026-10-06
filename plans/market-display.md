# Market display

Add a Market tab to the native desktop interface while preserving the simulation controls and Citizens view. Market quantities describe completed transactions plus current affordable requests and listed stock; no economic behaviour changes.

1. Expose read-only current and previous market-period statistics from existing accumulators and closing history. Cover 04:00 boundaries, inactive closed periods and fulfilled demand in tests.
2. Add Citizens/Market tabs, a seven-good price list, period selection, selected-good quantities and current sell orders grouped by seller. Preserve the native colors, scrolling and conditional UI writes. Test display data and interactions.
3. Run formatting, relevant tests and Clippy; complete native visual checks and independent Sol-6.1 review, fixing findings before committing.

Existing economy changes are pending separately and the town trade acceptance test is known to fail. Preserve that test and all unrelated work. The coordinator handles review, native QA and commits each completed step without including the pending economy draft.

## Progress

- Step 1: current and previous period statistics are implemented. Independent Sol-6.1 review found no issues. Tests cover fulfilled requests counted once, the first 04:00 close, period reset, and inactive periods without stale history.
