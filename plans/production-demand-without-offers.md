# Production demand without current offers

Desired production must remain profitable at the fixed lowest-quote/reference snapshot even when ingredients are absent from the market. Missing offers must not erase restock and import demand. Actual buying retains snapshot ceilings and requires real offers. When a qualified route can already obtain its inputs, prefer it over a hypothetical unavailable buy route, retaining complete make-versus-buy chains.

1. Separate projected and currently available route supply, preserving feasible route preference. Add regressions for empty-market weaver restock demand, executable intermediate manufacture, and fixed no-offer purchase ceilings. Independently review and validate before committing.
2. Re-run the latest Xena save diagnostics and full simulation checks; record results.

Step 1 complete: projected route valuation permits absent offers while current-route qualification still requires available supply. A qualified available manufacturing route takes precedence over an unavailable buy route. Purchase ceilings are unchanged. Regression tests cover market-demand propagation and executable make routes.

Validation: 576 simulation tests passed (388 unit and 188 integration), 6 ignored; formatting and workspace all-target checks passed. Independent review cleared the implementation and final fixture corrections with no findings.

Step 2 complete: release-build diagnostics loaded `saves/universe-1792730886.997440530.ron.gz` and replanned Xena in a temporary copy. With zero flax listings, her weavery projects 17 spins and 7 weaves and requests 3,400g flax. The same quantity reaches blocked requests, accepted market requests, and affordable unmet demand. There is also a 200g blocked thread request from attempted production preparation.

Advancing that copy to the next caravan visit produces 3,200g of caravan-owned flax listings, confirmed by the current period's 3,200g imported activity. Thus demand now causes real imports rather than depending on pre-existing listings. The original save is untouched. Local diagnostic evidence: `target/xena-demand-fixed-imports.log`.

Implementation commit: `c9fb005`. Both plan steps are complete; no outstanding review findings.
