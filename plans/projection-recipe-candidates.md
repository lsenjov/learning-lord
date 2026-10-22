# Projection recipe candidates

1. Compute eligible recipes once per projection. For a specified site, check skill duration and accessible production capability directly; otherwise retain global eligibility. Keep stocks, inputs, profit, quotas, and capacity decisions inside the projection loop. Validate candidate equivalence across roles and owned, inaccessible, public, town-owned, and missing sites, run simulation tests, review the change, and compare the existing release benchmark before committing.

Validation: formatting complete; simulation tests passed (557 passed, 6 ignored). Candidate equivalence covers every starting role and the listed site/access cases. Independent review found no issues.

The existing 24-hour, 30-citizen release benchmark took 48.154345s, 49.527401s, and 48.699756s after the change, versus baseline runs of 54.360363s, 52.571184s, and 52.313464s. Median wall time fell from 52.571184s to 48.699756s (7.36%). The updated profile attributes 86.13% inclusive samples to workplace refresh, 84.81% to projection, 62.32% to production-place lookup, and 53.33% to object access (43.79% self); ordered planning accounts for 3.44%. Supplier-route global scans remain outside this change's scope.
