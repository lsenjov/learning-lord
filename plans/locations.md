# Locations and travel

- Generate a 2 km × 2 km map for each universe, with the house at (0, 0), distinct forest, river and market locations, and Ada starting at home.
- Sleep requires the house, Forage the forest, Find rocks the river, and trades the market. Eat and Wait are available anywhere.
- Travel is an explicit action carrying a destination. Walk in a straight line at 600 ms/metre; interpolate position continuously from a fixed origin and finish exactly at the destination. Needs advance normally.
- Predictions insert travel before activities using each preceding predicted position. Travel contributes duration, cooldown progression, and completion wellbeing. Repeated activities at one site do not repeat travel.
- Preserve the two-hour execution commitment, finishing the current primitive action before replanning. Show travel in sequences and the decision report.
- Display a map with labelled locations, Ada's moving marker, and the current route; restart regenerates it.
- Approved clarification: preparation (resources and travel) has a four-hour limit; the final goal activity does not consume that budget and always finishes. Travel home followed by eight-hour sleep is valid.

## Implementation step

1. Implement location data, travel execution and prerequisites, planner integration, map display and documentation. Test movement, site restrictions, travel scoring/bounds, plan execution, branching and restart; run checks and independent review, fix findings, then commit.

## Completed

- Implemented map generation, continuous travel, location prerequisites, planner preparation accounting, and desktop map/route display.
- Added coverage for movement and snapshot preservation, location restrictions, route sequencing and scoring, preparation limits, commitment completion, price-boundary travel, map refresh and restart.
- Validation: 94 workspace tests passed; formatting, Clippy with warnings denied, build and diff checks passed. Native app map and route inspected.
- Independent review found no high or medium issues. Fixed the low finding: a test name still described the retired horizon rule.
