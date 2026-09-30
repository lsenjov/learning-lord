# Last planning decision

- Capture the best hunger, tiredness and wealth variants at the start of each real plan search, using the existing predictions without rerunning them.
- Store each sequence, duration, local average wellbeing and resulting full-plan score, plus the selected starting goal and prices used.
- Explain unavailable candidates as no executable sequence within the prerequisite horizon and action rules. Keep this distinct from an available but unchosen candidate.
- Keep the report unchanged during execution and price updates; replace it on replanning or restart.
- Show a compact last-decision panel for the current citizen, using grouped repeated actions and clear local-versus-full-plan score labels.

## Implementation step

1. Capture decision data during planning, display it in the desktop, test scoring/report lifetime and readouts, update docs, run checks, obtain independent review, fix findings, and commit.

## Progress

- Implemented an immutable shared report on each plan and a two-column desktop view with grouped sequences, local and full-plan scores, selected starting goal and planning-time prices.
- All 86 workspace tests pass, including search-result equivalence, report lifetime and readout tests. Formatting, Clippy with warnings denied, build, and diff whitespace checks pass.
- Independent review found no high or medium issues. Reported and fixed its low finding by sharing action labels across both displays; desktop tests and checks pass after the fix.
- Visually checked the native app at its default window size; both panels and controls fit.
