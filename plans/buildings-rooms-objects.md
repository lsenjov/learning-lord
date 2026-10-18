# Buildings, rooms, and objects

Locations retain movement, tax, stock, market, and existing type labels. Buildings have independent owners and contain rooms with fixed object capacity. Indoor equipment and resident beds live in rooms; public resources and crop plots live directly at locations. Objects have UUID identities, capabilities, persistent crop/batch state, and separate active worker claims.

1. Implement the simulation hierarchy and initial contents; assign household residents separate bedrooms in their shared home building. Replace plot/batch storage with object state. Select accessible objects by capability and claim them only when an action starts; release on completion or abandonment and safely replan contested actions. Keep brewing preparation tied to its specific vessel. Bump the save version and validate hierarchy, owners, claims, and active actions. Adapt existing reads to the new model, run simulation/workspace checks, obtain an independent code review, fix material findings, and commit.
2. Render compact building/room/object hierarchy, owners, state, and worker occupancy in Locations while preserving inventory, navigation links, and tax controls. Add meaningful behavioral coverage and validate the desktop build and observable initial world. Obtain an independent review, resolve material findings, and commit.

Object availability is checked by planning without booking future work. Room changes have no travel cost; action claims identify meaningful indoor occupancy. Crops and fermentation persist after workers leave. Goods ownership and existing location taxes stay separate from building access. No construction, relocation, upgrades, new pathfinding, or comfort simulation is introduced.

## Step 1 evidence

The simulation hierarchy, borrowed crop/batch views, designated bedrooms, exact object claims, independent building access, object-scoped stock/wealth, guild fee/history attribution, and save version 9 are implemented. Brewing preparations and production trials retain their exact selected vessel. Save validation checks identities, capacities, owners, active object claims, crop/batch action state, and synchronized citizen maps before resolving actions.

Two independent review passes identified and resolved malformed-save validation, private-equipment fee/history attribution, accessible collection candidate selection, and exact brewing trial source issues. The fresh review reported no remaining high or medium findings.

`cargo test --workspace --locked` passed: 674 tests, 7 ignored manual workloads. `cargo build --workspace --locked` passed without warnings. Regression coverage includes exclusive and concurrent equipment, public resources, separate household bedrooms, multi-owner locations, abandonment, out-of-order brewing preparation, private/busy collection vessels, production trials, and save round-trips/corruption.

Preparation can use auxiliary equipment in another room at the same location without extra travel; `current_room` reports the main equipment room. Generated breweries keep all their equipment in one workroom.

The manual five-day fresh-town profile passed: 10.34 seconds advancement, 10.47 seconds including setup. The preserved HEAD profile took 33.39 seconds advancement. These are different randomized worlds and equipment contention changes work opportunities, so this is a regression smoke check rather than a controlled speed comparison.

## Step 2 evidence

Locations now render buildings and their owner links, nested rooms with installed object capacity and distinct worker counts, and individual objects with persistent state, worker occupancy/capacity, and citizen links. Household beds identify their designated residents. Public objects show unlimited worker capacity. Outdoor crop plots use compact rows with tending windows and readiness details in hover tooltips. This replaces the previous standalone plot/brewery sections; stock, transfer, tax controls, and navigation remain available below.

An independent UI review reported no findings. Targeted Locations tests passed (26), followed by the full desktop suite (148 passed, 1 ignored). Workspace build and strict Clippy across all targets passed. Two equivalent save-validation conditions were collapsed into let-chains to satisfy Clippy.

Native captures of the shared household home, guild weavery, field, brewery, and forest were inspected: the hierarchy fits and scrolls without overlap, with no application console errors. Additional captures verified occupied brewing equipment and vessel count as one room worker, fermentation persists with zero workers, crop hover tooltips visibly show tending windows and readiness time, and the 720 by 480 minimum layout wraps without overlap. Step 2 is complete and ready to commit.
