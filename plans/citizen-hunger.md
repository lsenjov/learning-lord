# Citizen hunger and simulation time

## Agreed scope

- Allocate UUID v4 agent IDs and preserve them across universe snapshots.
- Add `AgentKind::Citizen` with a citizen-specific hunger value and hunger gained per game hour.
- Require explicit starting hunger and rate. Hunger may be negative (satiation), zero, or positive (need).
- Accept only finite hunger and finite, nonnegative hunger rates.
- Store the universe clock and elapsed durations as `u64` milliseconds; calculate hunger changes using `f64`.
- Advance the clock once per universe update and update every citizen for that duration.
- Return a new universe while preserving the source snapshot, including when an update fails.
- Leave meals, wellbeing, planning, other agent kinds, and frontend simulation controls for later.

## Implementation step

1. Add the validated citizen model, UUID allocation, and immutable time advancement. Adapt the desktop to read the encapsulated universe, update the design/run documentation, and cover numeric boundaries and snapshot behaviour with tests. Run formatting, workspace tests/build, and Clippy. Obtain an independent code review, resolve all high/medium findings and low documentation findings, and commit the completed step.

## Validation

- UUID creation produces version 4 IDs; existing IDs survive branching and ticking.
- Hunger changes proportionally to elapsed time, including subsecond durations and crossing zero.
- Separate citizens use their own rates; zero elapsed time and zero hunger rate behave correctly.
- Many small advances agree with one large advance within floating-point tolerance.
- Invalid numeric inputs, clock overflow, and nonfinite computed hunger are rejected.
- Independent branches can advance and create citizens on other threads without changing their source or siblings.
- The Bevy frontend still builds and the simulation remains engine-independent.

## Progress

- Plan recorded before implementation.
- Implementation complete: UUID v4 creation, validated `Citizen` data under `AgentKind`, encapsulated universe state, and immutable millisecond advancement with explicit numeric errors.
- Updated the desktop getter, README, and design notes. The frontend remains an empty scaffold without automatic ticking.
- Validation passed: all 10 integration tests, workspace tests/build, formatting, Clippy with warnings denied, and diff checks.
- Independent review found no high, medium, or low issues.
