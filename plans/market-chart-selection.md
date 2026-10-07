# Market chart selection and UI cleanup

One cohesive implementation step:

- Replace Current/Previous controls and duplicate statistics with chart day details. Hover previews a day; a click pins its timestamp; leaving restores the pin or current day. Keep the pin across goods, clear unavailable days and reset on simulation restart.
- Use one readout with unconstrained height to reserve space for wrapped details, preserving quantities, trade flows, units, legends and live buy requests/sell orders.
- Remove instructional reminders from desktop UI and use a neutral empty editor placeholder.
- Validate selection transitions and live order behavior with desktop tests, run formatting/workspace tests/clippy/build, inspect the native market readout, obtain an independent review, and commit the completed step.

Validation completed:

- `cargo fmt --all --check` and `git diff --check` passed.
- `cargo test --workspace` passed: 98 desktop tests, 137 simulation unit tests, and all integration suites; two pre-existing simulation tests remain ignored.
- `cargo clippy --workspace --all-targets -- -D warnings` and `cargo build --workspace` passed.
- Native Bevy screenshots from an isolated source copy confirmed wrapped details reserve their full height at wide and 880 px window widths, with separation from live buy requests and sell orders. The user's running app was left untouched.
