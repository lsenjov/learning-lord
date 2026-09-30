# Smaller meal acquisition target

- Compare 30- and 50-nutrition acquisition paths for hunger; preserve the four-hour preparation budget.
- Eat still consumes available food up to 50 nutrition, and buying still tops up to 100 g when affordable.
- Keep existing small meals, the 20-nutrition replan check, cooldowns and two-hour commitment.

## Implementation step

1. Add the target, cover empty-inventory planning and full-meal reachability after gathering, update docs, run checks and independent review, then commit.

## Completed

- Added both acquisition targets to the existing hunger variant comparison and documented their distinction from actual portions.
- Covered the three-hour forage path from empty, full-meal reachability after four predicted forages, partial buying and unchanged eating/buying quantities. Updated report expectations now that hunger is reachable from empty.
- All 99 workspace tests passed; formatting, Clippy with warnings denied, build and diff checks passed. Independent review found no issues.
- Four fixed-map debug timing cases averaged approximately 6–19 ms per plan (100 plans per case). The empty-citizen case selects travel, six forages and eating.
