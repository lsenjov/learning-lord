# Clothing implementation

Approved scope: six citizens; flax → thread → cloth → eight clothing blocks → garment; continuous ten-day wear with a 20-point maximum clothing need.

1. Implement the catalog, recipes, workshops, roles, worn condition, wealth and five-minute equip action; competing clothing goal and persistent intermediate production; desktop displays, exports and README. Independently review, resolve high/medium findings, validate, then commit. These exhaustive enum matches form one cohesive workspace change.

Equip consumes on completion anywhere and discards the previous worn garment. Carried garments with positive clothing need participate in ordinary goal comparison rather than forcing excess listing first. No spare reserve or commitment extension was introduced. The original fixed production queue remains intact.

Validation:
- Simulation: 188 tests passed, including wear partitioning, snapshot isolation, worn wealth, replacement, whole-garment trade, exact production chain and planner persisted blocks/failed purchase replanning.
- Desktop: 41 tests passed, including six roles/fourteen places, clothing details, mixed units and exports.
- Native window screenshot: `/tmp/learning-lord-clothing.png`; six citizen cards, clothing bars, 50% condition, twelve goods and workshops verified.
- Final locked workspace tests: 229 passed. Strict workspace clippy and formatting checks passed.
- Fresh independent review found no high/medium issues; low README findings corrected and verified. Low clothing meter geometry corrected: clothing uses the full 0–20 unsigned range, with focused geometry and citizen tests passing (four tests). Desktop strict clippy and workspace formatting passed again.

Performance: an unnecessary expansion of recipe-count combinations was identified and removed, restoring the approved queue. The subsequent bounded three-day town run passed in 18.74 seconds; the final desktop suite passed in 27.52 seconds. This is slower than the former 3.56-second desktop-suite baseline. No search pruning or horizon/goal-policy optimization was applied. The town produced thread, cloth, blocks and garments autonomously; garment trading/equipping are verified separately by domain/planner tests.

Status: integrated implementation step complete and ready to commit. Full workspace validation passed 229 tests before UI-only meter polish; all four focused citizen tests passed afterward, including one new meter test. No full town rerun was needed for that UI-only change. Independent review and focused meter re-review found no high, medium or remaining low issues.
