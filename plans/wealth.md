# Wealth and finding rocks

- Start coins and pebbles at zero, stored as `f64`; inventory weights remain grams.
- Add a price-only marketplace: berries 1 coin/kg, pebbles 2 coins/kg.
- Wealth is coins plus inventory at market prices. Add 10 wellbeing per coin of wealth.
- Find rocks takes 30 minutes and yields uniform random 5–15 grams of pebbles on completion; predictions use 10 grams.
- Preserve snapshot-owned randomness, continuous eating, and existing planning rules.

## Implementation step

1. Implement market valuation, wealth scoring, Find rocks, desktop readouts, and documentation. Test valuation, score tradeoffs, random/predicted yields, and planning. Run workspace checks, obtain independent review, fix findings, and commit.

## Progress

- Plan recorded before implementation.
- Completed market valuation, wealth scoring, Find rocks, desktop readouts, and documentation.
- All 67 workspace tests pass. Formatting, Clippy with warnings denied, workspace build, and diff whitespace checks pass.
- Independent review found no high or medium issues. Corrected its two low documentation findings in the state overview and wellbeing formula.
- Stocked-citizen planning benchmarks take approximately 34 ms per plan in a local debug build.
