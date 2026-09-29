# Minimal desktop reset

## Scope

- Replace the previous Rust implementation, browser/server setup, curriculum, and obsolete tooling.
- Keep the current game design in `plans/ticking.md` and repository instructions/history.
- Target a local, single-player Linux desktop application using Bevy.
- Keep the simulation in an independent Rust library using `imbl` for persistent collections.
- Start with an empty universe and a working window connected to it.
- Leave ticking, prediction workers, gameplay, maps, and persistence for later work.

## Steps

1. Replace the old workspace with a minimal simulation crate, remove obsolete files, and document the new structure. Verify that snapshots can be changed independently across threads. Review and commit.
2. Add the Bevy desktop executable and run instructions. Verify formatting, Clippy, tests, the build, and window startup/shutdown. Review and commit.

## Validation

- The simulation crate depends on `imbl` and has no Bevy dependency.
- Prediction snapshots preserve the original universe and sibling branches.
- The frontend owns a simulation value and reads its state.
- No game mechanics, networking, or background worker infrastructure are introduced.
- Run a separate agent review after each implementation step, fixing high and medium issues until none remain. Report low issues and fix documentation issues.

## Progress

- Implementation plan recorded.
- Step 1 complete: replaced the old workspace with the `imbl` simulation crate and removed obsolete implementation, browser/server files, documentation, and editor tooling.
- Step 1 validation: snapshot isolation test, formatting, Clippy with warnings denied, and diff checks passed. Independent review found no high, medium, or low issues.
- Step 2 complete: added a Bevy 0.19.1 desktop executable, a resource containing the simulation universe, and a startup screen showing the current agent count. Added run and development instructions.
- Step 2 validation: workspace build and tests, formatting, Clippy with warnings denied, and diff checks passed. Launched through `cargo run --locked` on Wayland/Vulkan, visually verified the empty-universe screen, and closed through the window manager with exit code 0. Independent review found no high, medium, or low issues.
