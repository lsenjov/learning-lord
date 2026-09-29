# Learning Lord

A local, single-player town simulation for learning Rust. The current scope is a minimal scaffold; the game design is in [plans/ticking.md](plans/ticking.md).

## Structure

- `crates/simulation`: engine-independent world data using `imbl` persistent collections.
- `crates/desktop`: a Bevy desktop executable that owns the current universe and displays its agent count.
- Prediction branches will remain simulation data, independent of rendering.
- The universe starts empty at simulation time zero. Agents have UUID v4 IDs, names, and a kind; citizens currently have hunger and a constant hourly hunger rate.
- Cloning a universe shares collection storage. Changes to an owned clone leave the original and sibling snapshots unchanged.

## Simulation

- `Citizen::new(hunger, hunger_per_hour)` validates explicit starting values. Hunger must be finite; its hourly growth rate must also be finite and nonnegative.
- Negative hunger represents satiation. Positive hunger represents a need for food. Neither side is clamped.
- `universe.with_citizen(name, citizen)` returns a new universe and the new agent's UUID. Existing IDs and the original universe remain unchanged.
- `citizen.advance(elapsed_ms)` returns a new citizen with hunger increased by `hunger_per_hour * elapsed_ms / 3_600_000`, preserving the original citizen. It needs no universe. Time is `u64` milliseconds; hunger calculations use `f64`.
- `agent.advance(elapsed_ms)` delegates to its kind and returns an updated agent. `universe.advance(elapsed_ms)` advances its clock once and delegates to each agent in a cloned universe.
- Clock overflow or nonfinite hunger results return a `SimulationError`, leaving the source universe unchanged. Small advances agree with an equivalent large advance within floating-point tolerance.
- Read state through `current_time_ms()`, `agents()`, and the citizen's `hunger()` and `hunger_per_hour()` getters.

## Run

The initial target is Linux with a Wayland or X11 desktop session and a working Vulkan graphics driver. Building requires a native C/C++ toolchain, `pkg-config`, and development libraries for Wayland and XKB. Bevy's [Linux dependencies guide](https://github.com/bevyengine/bevy/blob/v0.19.1/docs/linux_dependencies.md) lists distribution-specific packages.

```sh
cargo run --locked
```

The window displays `Learning Lord` and `0 agents`; it does not create citizens or advance the simulation automatically. Close it with the window manager's close button or shortcut. Eating, wellbeing, and prediction workers are not implemented yet.

## Development

Use Rust 1.98.1 with rustfmt and Clippy, as specified in `rust-toolchain.toml`.

```sh
cargo test --workspace --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo build --workspace --locked
```

Run just the simulation checks without compiling the frontend:

```sh
cargo test -p learning-lord-simulation --locked
```
