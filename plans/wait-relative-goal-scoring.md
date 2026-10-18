# Wait-relative goal scoring experiment

## Agreed scope

Rank all ordinary goal candidates by average planning wellbeing improvement over
passive waiting for the candidate's complete duration. Both forecasts start from
the same original state. A two-goal sequence keeps one uninterrupted waiting
baseline. Preserve eligibility, execution durations, listing priority, and the
wellbeing model; clarify displayed score labels.

## Step 1

Implement a passive baseline alongside each prediction, integrating both at the
same action and consumption boundaries. Add regression coverage for waiting,
production, and whole-sequence baselines; update existing score expectations.
Run workspace tests, formatting and Clippy, inspect actual simulation waiting,
and obtain an independent code review. Resolve valid medium/high findings and
obtain a fresh review before committing this step.

## Outcome

- Implemented passive baseline scoring at matching action/consumption boundaries,
  preserving one baseline across complete two-goal sequences. Wait scores zero.
- Workspace validation: 702 tests passed, seven ignored; formatting and Clippy
  passed. The consumption-boundary regression was rerun after a readability edit.
- Independent review approved with no high or medium findings. Its low
  documentation finding was resolved by documenting that old saved absolute
  diagnostics remain until each citizen replans.
- Same fresh-town snapshot, one simulation day: actual waiting fell from four
  citizens waiting two hours each to none. Production rose from 137.98 to 145.47
  agent-hours; sleep rose from 66.18 to 69.21. Effects are mixed: Dara starts sleep
  at elapsed 20.81 rather than 16.41 hours and peak tiredness rises from 2.24 to
  21.04; her 2.97 observed sleep hours end at the day cutoff. Ada eats an hour
  later, with peak hunger increasing from 5.91 to 10.07. Hale now sleeps 7.95
  hours rather than zero, reducing peak tiredness from 33.40 to 0.28.
- Retain as the agreed experiment and watch delays in addressing needs. Probe
  logs: `/tmp/ll-goal-score-before.log` and `/tmp/ll-goal-score-after.log`.
