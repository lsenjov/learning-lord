# Category windows

1. Implement and validate the complete category-window revision:
   - Keep exactly Citizens, Locations and Market windows; select entities in their category and route map clicks there.
   - Put all citizen cards in a scrollable left sidebar beside existing complete details and history. Preserve market charts and caravan controls, inventory transfers and tax editing.
   - Remove obsolete entity inspector and scoped chart machinery.
   - Lower restrictive window minimum sizes, preserve readable content with horizontal scrolling, and expose resizing through a visible corner grip and edge cursors.
   - Verify selection, map navigation, window reuse and drag/resize with regression tests, workspace checks and native screenshots; fix independent review findings before committing.

Validation completed:
- Workspace tests pass, including 95 desktop tests and all simulation suites (two existing ignored simulation tests).
- Formatting, strict workspace/all-target Clippy and production `cargo build --locked` pass.
- Independent review reports no high or medium issues; its stale roster documentation issue is fixed.
- Native Bevy screenshots show Citizens, Locations and Market sidebars/details, the ASCII corner grip, and horizontal scrolling at the 420 px minimum width. Synthetic pointer input is rejected by the host compositor; real-system regression tests verify focus, drag, resize, close/reopen and selection/window reuse.
