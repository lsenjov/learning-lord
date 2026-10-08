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
