# Balanced diets, fishing, and cabbage

1. Extend the goods, skills, recipes, crop lifecycle and demand allocation; start new towns with one of each of seven professions. Retain existing resources and use existing labor pricing.
2. Track four dietary categories with gradual meal intake, bounded decay and diminishing wellbeing bonuses. Choose complete food portions by predicted wellbeing and cost; preserve food alternatives in hunger acquisition without factorial search.
3. Expose diet status in citizen inspection, add behavioral regression tests, and validate simulation, desktop and deterministic planning performance.

Commit each completed step after independent review. Existing stress fixtures remain larger than the seven-citizen starting town.

Food supplier search uses one order beginning with each food, followed by catalogue order, to preserve five primary alternatives without enumerating 120 permutations.

## Step 1 completed

Added whole Fish/Cabbage goods, qualified time-based fishing, cabbage care/harvest and per-crop demand normalization, and seven starting citizens. Independent review found missing exhaustive integration fixture cases, which were fixed; subsequent review reported no high/medium issues. Validation: workspace check, all simulation integration tests compile, 31 focused simulation integration tests plus 8 farming units, and desktop starting resources and seven-role/sixteen-place tests pass.

## Step 2 completed

Added Meat/Carbs/Fibre/Sweets dietary values starting at zero, gradual actual food intake, 50/day bounded decay, and category bonuses of +5 at 50 and +7 at 100. Complete meals compare predicted final wellbeing including duration, diet, hunger and consumed food value/reserves; mixed partial meals remain available. Daily history integrates diet net rates exactly across zero/cap/bonus thresholds. Validation: full simulation library and integration suite pass, including food choice/cost, each food's proportional intake, analytical history averages, and meal/drink chunk invariance.

## Step 3 completed

Selected citizen details display all four dietary bars with numerical values and per-category bonuses; the readout includes total dietary wellbeing, daily decay and thresholds. Updated README, seven-citizen fixtures and the three-crop tax catalogue expectations. Added regressions proving selection updates diet bars, Fish/Cabbage hunger purchase alternatives survive search, and cabbage demand normalizes whole-head inventory into plot equivalents. Independent review identified low singular quantity labels, fixed in charts and inventory with label assertions. The field inspector now labels cabbage yields in whole cabbages; its regression covers eight and five heads.

Validation: full desktop suite passes (103 tests), and the subsequently added cabbage-yield label regression passes; full simulation library/integration suite passed after step 2 and additional cabbage demand/food supplier tests pass. Formatting and strict all-target Clippy pass. An isolated native visual harness captured `/tmp/learning-lord-diet.png`; all four bars and explanatory text fit the selected detail panel and the active plan remains scrollable below.

The fixed-position, fixed-citizen-ID/forage-seed seven-profession probe advanced ten days in about 4.8 seconds in the development test profile. It produced 71 fish and consumed 34 fish, 81 bread, one pie and 7,455 g berries. It produced/consumed no cabbage, and final mean dietary values were approximately Meat 26.58, Carbs 30.40, Fibre 0 and Sweets 16.21. A repeat preserved all production/consumption counts; dietary means differed only at floating-point rounding scale. Existing demand-driven farming did not choose cabbage in this scenario; no dietary goal or allocation tuning was added.
