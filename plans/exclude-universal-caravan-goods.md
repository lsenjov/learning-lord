# Exclude universal goods from caravan trade

Approved scope: berries and water remain locally gatherable and tradable, with core prices retained as reference. They cannot be imported or exported. Commercial gathering stops at the local stock cap; gathering for direct needs and ingredients remains available.

1. Add authoritative good eligibility to caravan policies, settlement, stock cleanup, thresholds and UI; cover unavailable controls, ownership-safe cleanup, local trade and gathering with regressions; validate and obtain a fresh code review, fix findings, then commit.

Validation: the full simulation suite passed (188 unit tests, all integrations, four ignored); all 110 desktop tests passed. A fresh Sol-6.1 review reported no high, medium or low issues. Workspace formatting, strict Clippy, build and whitespace checks passed. The implementation step is complete.
