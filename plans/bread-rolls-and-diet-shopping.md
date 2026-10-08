# Bread rolls, mixed meals and diet-aware reserve shopping

## Decisions

- Replace counted bread loaves with 20 g rolls providing 10 nutrition and 10 Carbs. Keep recipe inputs and time; batch output is ten rolls, preserving the existing two-loaf batch economics.
- Select whole-food combinations near 50 nutrition using the existing predicted meal wellbeing. Berries fill remaining nutrition; whole pies may provide 60. Partial meals remain possible.
- Preserve actual diet and existing food/water reserve wellbeing. Planning adds only the mean future dietary benefit of carried food relative to passive dietary decay with no food.
- Forecast six meals at hours 6, 18, 30, 42, 54 and 66 over three days, using the same meal selector and exact dietary decay and caps. Forecasts never mutate live inventories, diet, market or requests.
- Keep candidate shopping bounded: cheapest baseline, food-favoured alternatives and forecast-selected mixtures. Use carried food and withdraw own listings before buying. Respect actual rounded prices, funds, locations, taxes and cooldowns.

## Steps

1. Migrate the food catalogue, baking recipe, initial town stocks, units, fixtures and documentation; implement bounded mixed meal selection alongside smaller whole units. Verify economics, berry remainders, diet/cost tradeoffs and insufficient food. Independent review and commit.
2. Add diet-aware food forecasts and bounded reserve shopping, coherent planning scores and diagnostics/documentation. Test future value, affordability, withdrawals, read-only forecasting and demand lifecycle. Run workspace tests, formatting, clippy, build and a fixed-town performance probe. Independent review and commit.

## Step 1 completed

Implemented ten-roll baking batches, 70-roll starting stocks, whole mixed meals and whole-unit overfill. Eat reuses one selection for its duration and consumption. All simulation tests and 104 desktop tests passed; independent review found no remaining high or medium issues. Existing tax percentages and recipe inputs/time remain unchanged.
