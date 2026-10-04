# Four-agent interface

1. Create four identically initialized citizens sharing the existing locations. Add stable selectable summaries with name, coins, wealth, wellbeing, visual hunger/tiredness and current action. Show the selected citizen's inventory, exact needs, location, execution details and full plan grouped by actual goal boundaries, including durations and current/completed action states. Preserve goal boundaries in the planner without changing search or scoring. Display all four citizens on the map and highlight selection. Keep the existing controls and debug export. Validate multi-agent simulation, plan metadata and selection, inspect the running native interface, obtain independent review, resolve findings and commit.

Verification:

- `cargo fmt --all --check`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo build --workspace` pass.
- Tests cover equal initial needs/inventory across four citizens, shared map/prices, advancing without mutating the source, truthful contiguous goal ranges and predicted durations, metadata retention during execution, selected details/completed actions, selection across snapshots/restart, signed/clamped/urgent/overfull need bars, and distinct overlapping map markers.
- Native inspection at the default desktop size and a smaller window verified summaries, signed need bars, citizen selection, advancement, goal headings and scrolling through the complete plan and decision report. Font work is deferred at the user's request.
- Independent Sol-6.1 review of the final implementation and tests found no high, medium or low issues.
