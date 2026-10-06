# Market history chart

One cohesive milestone: add a native recent-30-market-day chart to each good's detail view. Keep the existing figures and orders. Use existing closing history, fill inactive days with zero quantities and carried prices, and expose the first period/world start through a small read-only API.

The price line steps at 04:00. Candle centers follow price (coins/kg); body heights show traded grams; upper/lower extensions show affordable unmet demand/unsold stock, using one linear quantity scale for the visible window. Explain the separate units and mark the live partial day. Show exact figures and period on hover, with a stable reserved readout to avoid scroll/layout shifts.

Validate history reconstruction and quantity geometry, empty/single/constant/large-value cases, hover selection and paused updates. Run formatting, focused tests and Clippy, native screenshot/resize/scroll checks, then an independent Sol-6.1 review before the coordinator commits this milestone. Preserve the documented unrelated town WealthOverflow failure.

## Validation

- Implemented the read-only initial-period/world-creation API and the native 30-day chart with inactive-period reconstruction, step prices, opposing quantity wicks, a separate kg size key, exact hover figures and a cyan partial day.
- Five focused chart tests cover sparse history and price timing, initial short periods/world creation, recent-window limits, constant/large numeric scales, centered Bevy cursor coordinates, hidden/paused updates and preserving chart entities when only live time changes.
- All 36 desktop tests pass; workspace Clippy with `-D warnings` passes. Formatting is clean.
- Native captures checked the 30-day fixture at 1280×800 and 1000×700 logical pixels, both wicks, zero trades, scrolling and an exact closed-day hover. The actual startup world checked a single constant-price partial day with zero trades and live unmet demand, including a native partial-day hover. Temporary fixture code was removed; screenshots are in `/tmp/market-chart-*.png`.
- Independent Sol-6.1 review found a hover coordinate mismatch with Bevy's centered cursor coordinates. It was fixed and regression-tested; subsequent review, including the final width cap, found no high, medium or low issues. This milestone is complete. The existing simulation WealthOverflow issue remains outside this chart change.

- Final visual polish caps candle bodies and center ticks at 18 logical pixels while retaining 40% of narrow day slots. The actual startup native capture confirms alignment; focused tests, build, formatting and workspace Clippy pass. README describes the chart and its units.
