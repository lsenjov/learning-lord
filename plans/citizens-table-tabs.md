# Citizens table and detail tabs

## Agreed behavior

Open Citizens to a dense table with name, profession, coins, wealth, and needs wellbeing. Household members display the shared account with a tooltip indicator. Selecting a row, citizen link, map marker, or number shortcut opens that citizen's details in the same floating window. A compact return control restores the table.

Details have four tabs: Needs, Belongings, Planning, and Daily history. Preserve linked entities and all existing planning and history diagnostics. Needs excludes wealth and dims only components with zero actual wellbeing contribution. Belongings includes household members' carried goods and shared stored goods, separating available, tax-reserved, committed production stock, and town-owned quantities. Put explanatory rules in tooltips. Skills expand inside Planning.

## Implementation step

- Reuse native Bevy UI primitives, floating windows, navigation, plan rendering, and history selection.
- Add compact table/detail containers, four tabs, shared reusable readout/table helpers, and native tooltips.
- Expose the existing needs wellbeing getter without changing simulation arithmetic.
- Cover table/detail navigation, tab selection, zero contributions, and shared stock accounting with desktop tests.
- Run formatting, desktop tests, and workspace checks. Capture and inspect native rendering using a temporary harness if the display permits it.
- Obtain independent review, fix valid high and medium findings and request a fresh review, then commit the reviewed implementation as one step.

## Verification

- `cargo fmt --all` and `cargo build --locked` passed.
- Desktop suite passed: 146 tests, 1 ignored manual performance probe. Coverage includes table/detail/return, four tab visibility, number shortcuts, native links and map navigation, shared-account values, zero-contribution needs including overfull hunger, and household carried/stored/work-reserved goods. Targeted checks also passed after the final first-frame roster fix, explicitly covering populated/empty/populated replacement, map/link/shortcut detail navigation, and production readouts.
- `cargo check --workspace --all-targets` and `cargo clippy --workspace --all-targets -- -D warnings` passed.
- Independent review completed; valid performance and documentation findings were fixed and the fresh review found no unresolved high or medium findings.
- An isolated source-copy Bevy harness captured the actual native app using a fresh universe and the latest available saved universe. Inspected overview, shared-account tooltip, neutral and active Needs, household Belongings, Planning, deep per-site planning diagnostics, Daily history, and narrow resize states.
- Native pointer hit testing opened a table row. Actual Shift + wheel scrolling exposed the rightmost Wealth and Wellbeing columns in the narrow window. Returning to details preserved access to the header and all four tabs.
- Captures and harness remain outside the repository in `/tmp/citizens-native-capture/`; no permanent capture code or dependencies were added. User `AGENTS.md` was not edited or staged.
