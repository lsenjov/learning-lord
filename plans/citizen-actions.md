# Timed citizen actions

## Agreed scope

- Standard hunger growth is 100 nourishment per 24 simulation hours, balanced by two 50-nourishment meals.
- Starting hunger remains explicit. Retain an explicit custom-rate constructor for tests and future variation.
- Eating and waiting each take 30 simulation minutes. A citizen can perform only one action at a time.
- Hunger advances throughout either action. Eating subtracts 50 hunger on completion; waiting has no completion effect.
- Advance through action completion in time order, then spend any remaining elapsed time idle. Do not automatically choose another action.
- Apply a wellbeing penalty of one point per hunger unit below -100. Preserve the existing hunger and starvation penalties.
- Preserve source snapshots for both successful operations and errors. Keep citizen calculations independent of the universe.
- Leave food inventory, trading, relationships, planning, and frontend controls for later.

## Implementation step

1. Add the standard hunger rate, overfull scoring, action state and immutable action-start/advance APIs on citizens, with agent and universe delegation. Update documentation and tests for timing boundaries, daily nourishment balance, busy/missing-agent errors, numerical errors, and snapshot isolation. Run workspace tests/build, formatting, and Clippy. Obtain an independent review, resolve high/medium findings and low documentation findings, and commit the completed step.

## Progress

- Plan recorded before implementation.
- Complete: added standard-rate citizen creation, overfull scoring, 30-minute eating/waiting actions, and immutable action-start APIs on citizens, agents, and universes.
- Advances apply meal nourishment at completion and account for all remaining elapsed time. Busy and unknown-agent requests return explicit errors without changing their source.
- Updated README and design notes. Added eight action tests and extended existing scoring coverage.
- Validation passed: all 23 tests, workspace build, formatting, Clippy with warnings denied, and diff checks.
- Independent review found no high, medium, or low issues.
