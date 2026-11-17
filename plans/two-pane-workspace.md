# Two-pane desktop workspace

## Implementation step

Replace the floating window manager and integrate its pane identities with the existing inspectors, map, navigation, and HUD in one change. Existing category-wide selection resources couple the window manager and the page renderers. Both must change together for two detail panes of the same type to work.

- Use an implicit map when the workspace is empty. Open menus on the left and link destinations on the right. Preserve the source of a link.
- Keep manually minimised panes on the left and automatically displaced panes on the right. Restore the latest displaced pane into a vacancy. Explicit recovery from the right uses the entry's previous slot.
- Keep pane selections, editors, history controls, and scroll positions independent. Reuse existing detail content and entity identities.
- Place navigation and System at the top corners and the time controls along the bottom. Clip map rendering and input to its pane.
- Exercise navigation, recovery, and simultaneous details through the desktop systems. Run the test suite, review the implementation independently, resolve findings, then commit.

## Questions for testing

Check usable width with both columns populated. Revisit swapping sides and additional splits after using the two-pane workspace.
