# Bread rolls, mixed meals and diet-aware reserve shopping

## Decisions

- Replace counted bread loaves with 20 g rolls providing 10 nutrition and 10 Carbs. Keep recipe inputs and time; batch output is ten rolls, preserving the existing two-loaf batch economics.
- Select whole-food combinations near 50 nutrition using the existing predicted meal wellbeing. Berries fill remaining nutrition; whole pies may provide 60. Partial meals remain possible.
- Preserve actual diet and existing food/water reserve wellbeing. Planning adds only the mean future dietary benefit of carried food relative to passive dietary decay with no food, minus future consumed-food wealth loss using the existing 0.1 wellbeing per coin, floored at zero.
- Forecast two meals per day over three days, starting now if no recent actual meal and otherwise continuing its twelve-hour schedule, using the same meal selector and exact dietary decay and caps. An ongoing meal contributes its remaining portion; a meal crossing the horizon is clipped. A forecast-only last-meal timestamp keeps meal start and completion continuous. Other needs remain fixed between meals; hunger and diet advance. Forecasts never mutate live inventories, diet, market or requests.
- Keep candidate shopping bounded: cheapest baseline, food-favoured alternatives and forecast-selected mixtures. Use carried food and withdraw own listings before buying. Respect actual rounded prices, funds, locations, taxes and cooldowns.

## Steps

1. Migrate the food catalogue, baking recipe, initial town stocks, units, fixtures and documentation; implement bounded mixed meal selection alongside smaller whole units. Verify economics, berry remainders, diet/cost tradeoffs and insufficient food. Independent review and commit.
2. Add diet-aware food forecasts and bounded reserve shopping, coherent planning scores and diagnostics/documentation. Test future value, affordability, withdrawals, read-only forecasting and demand lifecycle. Run workspace tests, formatting, clippy, build and a fixed-town performance probe. Independent review and commit.

## Step 1 completed

Implemented ten-roll baking batches, 70-roll starting stocks, whole mixed meals and whole-unit overfill. Eat reuses one selection for its duration and consumption. All simulation tests and 104 desktop tests passed; independent review found no remaining high or medium issues. Existing tax percentages and recipe inputs/time remain unchanged.

## Step 2 completed

Implemented a shared food forecast, bounded calorie and variety shopping, and retention of food chosen for future meals before filling the remaining nutrition reserve. Planning uses current wellbeing plus nonnegative future dietary gain after forecast consumed-food wealth loss. The twelve-hour forecast schedule follows a recent actual meal; pending consumption and horizon-crossing meals are integrated proportionally. Actual diet, food and market state remain unchanged by prediction.

Stocked variety paths obtain their favoured food from owned listings before purchasing it. Unhelpful baskets are rejected against waiting for the same duration, and speculative variety demand is gated before market search. The UI reports Future food benefit with the saved planning scores. The shared actual and forecast meal evaluator accounts for needs, consumption cost, garment wear and water reserve changes, and is checked against full consumption snapshots with food reserve wellbeing excluded. Food reserve bonuses remain in overall wellbeing and planning; excluding them from meal ranking keeps excess stock from changing which retained meals are chosen.

Validation: all workspace tests passed (104 desktop tests and 177 simulation unit tests, plus simulation integration suites); formatting, strict all-target clippy and workspace build passed. Independent review reported no remaining high or medium issues. Regressions cover mixed meals, a week of carried rolls, core-priced variety, excessive prices, owned listing withdrawals, unneeded calorie demand, selected purchase request lifecycle, dietary retention, read-only forecasting and meal-boundary continuity.

The seven-citizen ten-day probe run alone completed in 12.93 seconds. It produced 158 fish and 40 cabbages and consumed 89 fish and 10 cabbages; the highest citizen hunger at day ten was -10.48. Town outcome counts can vary with generated citizen/place IDs. Dietary decay and category bonuses were not retuned.
