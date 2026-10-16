# 09 — Compare buy-input and make-input production routes

## Hypothesis and scope

Recognise profitable production chains without exploring every combination. Compare two deterministic ways to supply each output recipe. This can identify cloth → blocks → garment as more attractive than importing every block, but does not fix reference wealth loss or short candidate scoring by itself.

## Proposed algorithm

- Route A uses owned usable inputs, then purchases missing direct inputs cheapest-first.
- Route B uses owned usable inputs, then recursively makes missing inputs only where the agent has the required skill and accessible facility; purchase unavailable-to-produce leaves. It is 'make what this agent can', not autarky across all professions.
- Initially support the current unique supplier relation for immediate same-site recipes, reject cycles with an active recursion stack and cache immutable topology only. Multiple suppliers and delayed upstream production are outside this first experiment.
- Track finite listings and inventory quantities; deduplicate routes that resolve to the same actions. No enumeration of arbitrary partial make/buy mixtures.
- Calculate chain revenue minus initial-owned opportunity value, purchases, fees and taxes once. Include all work and necessary acquisition/travel time. Intermediate output value is not additional final revenue. Batch surplus remains ordinary stock, valued consistently with competing alternatives.
- Select A versus B for the same output quantity using profit/time, then preserve stock_fraction/rate ranking between outputs; execute a bounded prefix in quoted route order. On replan use actual inventory, no permanent earmarking or committed long-term route.

## Critique

Two route labels do not guarantee cheap search. Rounded batch sizes and acquisition travel still cost work to resolve. Possible later extensions to multiple suppliers or multi-output recipes would introduce further choices, where a greedy supplier choice could miss the best chain; those extensions are excluded initially. Daily horizon and short-prefix execution can change ratios and abandon earlier work; finite stock assumptions can fail across multiple agents.

Initial 'make all' can be inferior to a mixed route, which is deliberately excluded. Existing owned intermediates mean the make route may naturally need fewer stages, but comparing profit/time from different starting stocks must use the same opportunity valuation. A profitable whole chain can still lose to sleep under the short wellbeing score. Blocks need not be reserved for garments; whether blocks remain a standalone commercial product is an independent catalogue/policy decision, not silently removed here.

## Implementation plan if chosen

1. Build pure route quotes for current single-output recipes; explicitly reject cycles and unsupported catalogue shapes. First compare manual Finn chains without changing selection.
2. Only after diagnostic evidence justifies selection integration, add the two deterministic routes and route diagnostics at one workplace, preserving normal procurement and action execution. Test finite quote exhaustion, existing inputs, duplicate routes, rounding, taxes and travel.
3. Measure plan count and route construction time, then check executed prefixes across replans and whether useful work actually wins wellbeing. Review before expanding multi-site chains.

## Evaluation and recommendation

Useful later for deeper professions, but larger than the immediate bug. It addresses route quality, not misleading inventory valuation. Defer until the narrow procurement experiment shows whether route search is needed. The four-hour/eight-block examples are approximate and omit some travel/rounding; do not turn them into guaranteed production outcomes.

## Review resolution: two executable, bounded routes

- Initial scope is immediate single-output recipes at one chosen physical PlaceId. Exclude crops and fermentation as upstream suppliers and buy their current available output instead; do not use expected crop/brew stock as usable input. Preserve existing urgent care/collection precedence outside the route experiment.
- Current supported catalogue has a unique producer per good. Precompute that immutable relation; reject cycles using an active recursion stack. Do not add a multi-supplier optimisation rule now. Fix final quantity to one output batch and site, build exactly two independent routes, and reject rather than shrink any infeasible route. Enforce caps of 128 expanded production batches and 256 primitive actions during construction, including preparation; over-limit is explicit unsupported, never an apparently profitable partial chain.
- Reuse prediction preparation and execution for a route ledger of actual cash, location, usable owned stock, finite listings and net tax output. No speculative future sales fund purchases. Collect/withdraw remote owned stock with real travel and fees. Round payments per seller per actual purchase action. Batch counts must supply net usable inputs after conservative tax carry. Cache topology only, not quotes keyed solely by recipe while inventory changes.
- Choose A versus B only for the same final product/quantity. Feed chosen route rate into the existing stock_fraction/rate ordering between outputs, preserving export and limited trial policies. A route can make a previously negative direct-input recipe eligible, but that is explicitly the chain-recognition change. Blocks remain standalone eligible outputs. Count consumed initial stock once, final target revenue once and only unconsumed surplus at reference; do not credit intermediate blocks consumed by garments.
- The candidate's bounded executable prefix follows its quoted route order; do not rerank after its restock. The next real replan remains free. This limited execution change is explicit. Actual prefix wellbeing contains only actual state changes; no future garment profit is inserted. Log abandoned routes across replans.
- First implementation stage, if later chosen, is diagnostic quotes without changing selection. Integration remains contingent on observed need and prefix/score tests; it is not a promised solution to sleep winning.
