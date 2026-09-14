# Curriculum and tooling setup

Authorized scope: document the agreed town simulation, create a self-paced Rust curriculum without solutions or hints, and prepare minimal local tooling. Do not implement lesson exercises.

Human-facing documentation uses HTML, as required by `/home/logan/AGENTS.md`.

## Steps

1. Write the game rules and curriculum, including work units, acceptance criteria, primary documentation links, and a first-lesson entry point. Review scope and links; commit the completed documentation step.
2. Install and verify Rust tooling; create a minimal workspace and optional Neovim setup. Use a separate implementation agent as required by `/home/logan/AGENTS.md`. Keep frontend application scaffolding for the relevant lesson. Have a fresh agent review the implemented code, fix findings, repeat until no high/medium findings remain, and commit the completed tooling step.

## Validation

- Check curriculum coverage against every agreed feature; distinguish prototype requirements from later extensions.
- Check HTML navigation and documentation URLs.
- Run formatting, Clippy, native tests/build, and the applicable WebAssembly target check.
- Verify the optional Neovim configuration starts and attaches rust-analyzer without changing the user's editor configuration.
- Report low findings before proceeding; fix documentation findings.

## Progress

- Planning recorded. Repository started with an empty `.gitignore` and no source files. Rust was not on PATH; Neovim 0.12.4 was present.
- Step 1 complete: HTML home, prototype rules, and 45 work units in 14 modules. All 75 initial external documentation links and local navigation targets checked successfully. Independent review found no high, medium, or low issues.
- Step 2 complete: user-local Rust 1.98.1, pinned toolchain with rustfmt/Clippy/rust-analyzer and the Wasm target; minimal Cargo workspace; optional plugin-free Neovim configuration; HTML tooling guide. No game exercises implemented and no sudo used.
- Validation passed: formatting, Clippy with warnings denied, native tests (zero initial tests), native execution, Wasm build, and headless Neovim rust-analyzer attachment. A fresh review agent independently repeated these checks and reported no findings. Final documentation adds explicit editor commands and two checked official setup references.
