# Bounded wealth lookahead

1. Implement, verify, independently review, and commit bounded wealth lookahead.
   - Compare gathering routes using completed gathering-only previews of at most two hours, including travel. Retain the first travel and gathering step for the existing outer search.
   - Verify travel amortization, first-step state retention, the preview budget, and isolation from prefix scores. Clarify the displayed goal scores and run formatting, tests, clippy, and builds.

## Verification

- Workspace tests, formatting, Clippy with warnings denied, workspace build, and diff whitespace checks pass.
- Fresh independent review reported no high or medium issues. Fixed its low documentation finding so unavailable explanations include the two-hour wealth preview limit.
- Implementation step completed after review and verification.
- Four fixed-map debug timing cases (100 plans each) averaged approximately 4.1–5.8 ms per plan. The extra search is confined to the wealth preview; only one first-step candidate enters the outer search.
