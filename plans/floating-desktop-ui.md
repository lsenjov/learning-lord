# Floating desktop interface

1. Implement and validate the complete approved interface, with these subtasks:
   - Add a window manager with remembered bounds, focus ordering, drag/resize/close interaction and input capture; replace the tab layout with fixed HUD controls and floating overview windows. Preserve the reactive worker scheduling.
   - Promote citizen, location and good selection to individually keyed detail windows. Scope detail state and refresh/control handlers to their owning window, preserving plans, history, inventories, tax editing, caravan controls and charts.
   - Expand the map to the background, preserving spatial scale, with pan/zoom and map selection opening details. Add interaction regression tests and perform native inspection.
   - Obtain a fresh independent code review, fix all high/medium findings, repeat review as needed, fix low documentation findings and report other low findings. Run formatting, desktop/workspace checks and relevant tests before committing the completed implementation step.

Implementation and validation are complete; the root agent will commit this cohesive step.

- Workspace tests passed (391 tests, including 94 desktop tests; two existing ignored tests).
- Strict workspace Clippy, formatting and whitespace checks passed.
- Fresh independent review found no remaining high, medium or low issues after fixes.
- Native screenshots verified map/HUD, citizen overview, independent citizen/good/location details and market policies/chart. Synthetic pointer events were rejected by the host XWayland session; headless interaction tests covered drag, resize, focus, close/reopen, capture and frontmost scrolling.
