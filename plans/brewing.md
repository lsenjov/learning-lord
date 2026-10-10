# Brewing and beer

## Agreed behavior

- Add integer-gram goods Hops and Beer, a Brewing skill, a Brewer profession and an owned Brewery.
- Hops use existing four-day planting/tending/harvesting behavior; full yield is 600 g per plot.
- Each beer batch consumes 200 g wheat, 20 g hops, 25 g wood and 1,000 g water. Consume ingredients when preparation starts.
- Preparation takes one hour, fermentation takes 48 unattended hours, and collection takes 15 minutes for 1,000 g beer. Skill reduces active labor only.
- A brewery has four slots, occupied from preparation start until collection completes. Finished beer waits without spoilage.
- Production planning remains location-based. Count expected batch output against stock targets and value unfinished batches with a 30% discount, without requiring a plan to wait through fermentation. Include slot occupancy and maturation in production capacity and input budgets.
- Core prices include inputs and active labor with the existing markup. Hops and beer participate in ordinary markets, caravans and taxes; assess socage on beer at collection only.
- A 200 g drink of beer restores 50 thirst, 10 nutrition and 10 carbs over one minute. Water remains 200 g for 50 thirst over one minute. Choose food and drink together so beer contributes to the meal target.
- Drinking reserves and shopping accept either drink; water-only recipe inputs must remain water. Preserve existing reserve retention and tax-reserved stock exclusions.
- Add one brewer with the usual home, coins and personal supplies, plus inputs for four batches. Show brewery slot stage and remaining fermentation time in location details.
- Persist batches and active work. Bump the save version; old saves remain untouched but unsupported, as agreed. No migration for this milestone.

## Steps

1. Implement hops and brewing state/actions, production planning and economics, taxes/history and persistence; update exhaustive frontend labels as needed. Test lifecycle, capacity, prediction and tax behavior, independently review, and commit.
2. Implement beer-aware drinking, combined meal selection, reserve retention and market shopping. Test progressive benefits, food/drink choices, planning parity and inventory/tax boundaries, independently review, and commit.
3. Add the starting brewer, brewery details and documentation. Run integration/performance checks and a locked app launch, independently review, and commit.

## Progress

- Step 1 complete: brewing lifecycle, planning, economics, persistence and brewery slot display implemented. Full simulation tests, location UI tests, formatting and strict workspace Clippy pass; independent reviews have no remaining high or medium findings.
- Step 2 complete: joint food/drink selection, progressive beer benefits, reserves, shopping, history and save resume implemented. Full simulation tests and strict Clippy pass; final independent review found no issues.
- Step 3 complete: Hale starts with an owned brewery and four batches of ingredients; startup and documentation independently reviewed. All 530 workspace tests pass (four existing tests ignored), formatting and strict workspace Clippy pass, and a bounded `cargo run --locked` launch initialized Vulkan and opened the window.
- An early town integration run reported `MissingInputs`. It did not recur after the subsequent reserve corrections: six independent ten-day runs and the final workspace run passed. Instrumented runs confirmed Hale collected 6,000 g and 5,000 g of beer. No speculative runtime fix or temporary diagnostic code was retained.
