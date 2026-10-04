# Inventory and property foundation

1. Generalise citizen inventory to berries, pebbles, wheat, flour, wood, water and bread in `f64` grams, preserving existing berry eating and placeholder trades. Replace fixed sites with individually identified places and ownership: four private homes, Ada's field, Bram's mill, Dara's bakery, and a public forest, river and market, all at separate random positions within the existing map and separation rules. Ada is the farmer, Bram the miller, Cleo the woodcutter and Dara the baker as starting configurations; skills and production remain for later steps. Every citizen starts with 200 g of berries. Citizens carry their inventory; property is excluded from wealth. Sleep uses the citizen's own home and travel targets a particular place. Update planning, map, selected details and debug representations, preserve snapshots and existing action/scoring behaviour, add meaningful regression coverage, run checks and native inspection, obtain independent review, resolve findings and commit.

Production rates, starting production goods and coins, bread nutrition/eating behaviour, progression, storage, carrying limits and spoilage will be discussed in later steps. Berries and bread are the intended edible goods; bread remains inactive until its parameters are agreed.

Verification:

- `cargo fmt --all` completed; `git diff --check` is clean.
- `cargo test --workspace` passed all 121 tests (desktop, simulation unit tests and integration regressions).
- After the map-label adjustment, all 24 desktop tests, formatting checks, Clippy and the workspace build passed again. The follow-up test covers bounded labels, selection visibility and retained markers.
- `cargo clippy --workspace --all-targets -- -D warnings` passed.
- `cargo build --workspace` passed; native executable is `target/debug/learning-lord`.
- New regression coverage checks every good's validation and snapshot independence, deferred goods remaining unpriced and inactive, berry eating/trading compatibility, multiple ownership, own-home sleep, foreign/invalid travel errors, citizen identity and duplicate insertion, planner home routes/durations/goal metadata, ten separated UUID v4 sites with consistent ownership and starting configuration, and expanded debug export. Existing route/selection/restart/all-citizen advancement checks remain passing.
- Native inspection verified starting inventories, all ten markers, selected ownership labels, profession/property details, citizen selection and time advancement into Sleep at each citizen's own home. Crowded map labels were corrected and recaptured.
- Independent Sol-6.1 review of the final implementation and the map-label follow-up found no high, medium or actionable low issues.

Implementation notes:

- `Citizen::new` remains an empty standalone fixture; desktop setup supplies 200 g berries and role metadata. Roles do not lock actions.
- Citizen IDs are created with the citizen and preserved at insertion. `with_citizen` returns a fallible result and rejects duplicate IDs.
- Travel uses `PlaceId`; action duration calculation is fallible for missing/private destinations. Each citizen keeps an identified home for Sleep, and ownership is read from the shared persistent place map.
- Goods and places use persistent maps to preserve cheap snapshot cloning. Prices/value are optional for unconfigured goods, which currently contribute nothing to market-valued wealth. Property is excluded.
- Production, field inputs/growth, skills, bread nutrition/speed, starting production stocks, starter coins, storage/capacity/spoilage and font changes remain deferred. Further implementation steps require discussion.
