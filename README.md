# Learning Lord

A local, single-player town simulation for learning Rust. The current scope is a minimal scaffold; the game design is in [plans/ticking.md](plans/ticking.md).

## Structure

- `crates/simulation`: engine-independent world data using `imbl` persistent collections.
- A Bevy desktop frontend will read the current universe. Prediction branches will remain simulation data, independent of rendering.
- The universe starts empty. Agent IDs and names are the only agent data currently defined.
- Cloning a universe shares collection storage. Changes to an owned clone leave the original and sibling snapshots unchanged.

## Development

Use Rust 1.98.1 with rustfmt and Clippy, as specified in `rust-toolchain.toml`.

```sh
cargo test --workspace --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
```
