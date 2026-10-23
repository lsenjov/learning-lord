# Production demand without current offers

Desired production must remain profitable at the fixed lowest-quote/reference snapshot even when ingredients are absent from the market. Missing offers must not erase restock and import demand. Actual buying retains snapshot ceilings and requires real offers. When a qualified route can already obtain its inputs, prefer it over a hypothetical unavailable buy route, retaining complete make-versus-buy chains.

1. Separate projected and currently available route supply, preserving feasible route preference. Add regressions for empty-market weaver restock demand, executable intermediate manufacture, and fixed no-offer purchase ceilings. Independently review and validate before committing.
2. Re-run the latest Xena save diagnostics and full simulation checks; record results.

Step 1 complete: projected route valuation permits absent offers while current-route qualification still requires available supply. A qualified available manufacturing route takes precedence over an unavailable buy route. Purchase ceilings are unchanged. Regression tests cover market-demand propagation and executable make routes.

Validation: 576 simulation tests passed (388 unit and 188 integration), 6 ignored; formatting and workspace all-target checks passed. Independent review cleared the implementation and final fixture corrections with no findings.

Step 2: checking the latest Xena save against the release build.
