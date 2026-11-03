# Household population and property

## Scope

Implement units 1-4, then stop for user inspection. Each unit receives an independent code review, resolution of valid high/medium findings followed by fresh review, appropriate validation, and its own commit. Fix low documentation findings; report other low findings. Keep the UI compact and put explanatory text in tooltips.

Old saves may be invalidated. Do not add compatibility migration. Stop for discussion if an unexpected complication materially changes behavior or architecture, or a small specification change would significantly simplify implementation.

## Authorized units

1. **Halve degradation rates.** Halve condition loss globally for buildings, built objects, and clothing, including usage wear. Do not alter needs growth, diet decay, construction speed, or repair speed. Check all condition-loss paths and update relevant expectations/documentation.
2. **Universal households and household ownership.** Every citizen in a universe belongs to exactly one nonempty household, including single citizens. Private buildings, built objects, funds, and stored goods belong to households; individual members still carry items and wear clothes. Preserve town/public and guild ownership. Update creation, economic access, histories, UI ownership display, and persistence validation consistently. Reject old save versions. Preserve existing simulation behavior apart from the agreed ownership change.
3. **Mixed residential/workplace buildings.** In new-world generation combine eligible private household workplaces and accommodation in one building. A weaver can live where they work. Keep guild premises and large/public workplaces such as fields and forests separate. Reuse existing rooms, object capacity, and location capabilities where possible; validate residents can sleep and perform their profession. Do not silently discard assets or create duplicate workplaces.
4. **Dev mode and citizen creation.** Add a local UI preference revealing dev menus. Add a member to a chosen household with a selected existing profession, and create a household with its first member's profession and chosen starting funds. Initially require existing suitable accommodation, with explicit selection and capacity validation. Do not silently create workshops or tools for selected professions. New entities and funds persist in simulation saves independently of the toggle. Integrate changes through the simulation worker and refresh household/economic/history state safely. Provide useful validation errors without dense permanent explanation text.

## Validation

Use focused behavioral tests for each simulation change, formatting and relevant compilation checks, and broader existing tests where cross-cutting ownership or building changes warrant them. Check persistence round trips and rejection of old saves. Test new-world household/ownership invariants and mixed-use sleep/work access. Test dev creation with existing and new households, insufficient accommodation, live simulation state, and save/load. Record commands and results below as work proceeds.

## Deferred units

5. Household departure and dev removal of a member or whole household. At member planning boundaries sample average household wellbeing; below zero starts/continues a timer, zero or above clears it. Leave at the first below-zero sample after 24 hours. Departing households take all portable belongings and coins; buildings and built objects transfer to town. Dev removal bypasses the timer and uses departure cleanup. Individual removal's asset division remains to be settled.
6. Town grants and property listings: free grants, fixed prices, current value, or percentage discount from current value.
7. Homelessness and inns: beds and meals for resident citizens; no travellers. Allow dev creation without accommodation then.
8. Town site creation and development: town chooses and pays for development before granting or listing property.
9. Autonomous purchase and development: only propertyless households buy; compare land or developed buildings according to needs and development costs, then build to suit. Land ownership alone does not provide housing.

## Progress

- Plan recorded; implementation pending.
