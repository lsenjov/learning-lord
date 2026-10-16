# 03 — Evaluate the stocked production period

## Hypothesis and scope

Bulk investment should be assessed against the later work it funds. Extending evaluation can reveal that payback without pretending inputs are finished products. It does not remove real negative margins.

## Proposed algorithm

- Keep the immediate executable goal and replanning behaviour unchanged. Add a clearly separate forecast of a fixed common period, initially twelve simulated hours, to evaluate procurement alternatives.
- Use a bounded deterministic continuation of existing rules rather than expanding the combinatorial goal tree. Include food, thirst and sleep responses; do not assume twelve uninterrupted productive hours.
- Compare every competing first goal over that same period using the same continuation policy. Evaluating only production for twelve hours against a short sleep/food candidate creates unequal comparisons.
- Score exactly the same twelve-hour interval for every alternative. A boundary-crossing action remains partially complete in the forecast; do not award its completion output outside that interval. Execution may still finish the real action normally. Do not count terminal-product value on top of ordinary inventory.
- Commit only the first existing goal. Predicted prices and other agents remain approximate as today.

## Critique

This is broader than 'look longer at production': fair comparison requires extending alternatives too. Duration-averaged wellbeing may still prefer short comfortable plans when durations differ. Long sleep crossings exacerbate this. More affordable investment does not necessarily produce higher average wellbeing if payback is late.

The simulation previously experienced search scaling problems. A deterministic continuation bounds branches but introduces heuristic bias: a continuation that fails to eat or poorly selects recipes can condemn a good initial purchase. Future stock and price assumptions grow weaker. Parallelism does not remove total CPU work.

## Implementation plan if chosen

1. First replay a fixed, manually inspected procurement/work sequence without changing selection, to verify payback within twelve hours. This is diagnostic evidence only.
2. Define equal-duration competitor evaluation, boundary handling and bounded needs-aware continuation; instrument action counts and CPU before integrating scoring.
3. Add an isolated experimental evaluator and compare same-save choices, performance, hunger/sleep outcomes and predictions versus actual execution. Review before adopting.

## Evaluation and recommendation

Pass requires consistent comparison durations, affordable feasible continuations and measured performance. No 'production wins' assertion should be required. A simpler alternative is a terminal value for useful stock, but that is a different valuation policy covered in plans 07/08. Defer this until shorter procurement experiments establish that payback duration is decisive; it is not a small horizon constant tweak.

## Review resolution: strict boundary and staged gate

The first deliverable is diagnostic replay only. An integrated evaluator is not implementation-ready until the following contract is demonstrated:

- Split forecast advancement from scoring: sample or integrate predicted state only through the exact common boundary, including partial eating, sleep, travel and work. Already-started work longer than twelve hours gets no premature output. Test crossing actions and comparison duration independently of real commitment rules.
- Deterministic continuation chooses at completed-goal boundaries. Permit repeated goals after cooldowns. Use existing legal one-goal candidates, not nested two-goal search: satisfy the most urgent positive need above documented thresholds first (initial experiment hunger/thirst 20, tiredness 50; compare normalised excess, stable goal-order ties), otherwise choose the best one-goal wellbeing candidate, including sleep. This heuristic is a declared new evaluator policy, not a claim that existing rules already define it; do not tune it to force Finn to work.
- Stop on no progress and mark the forecast incomplete. Cap at 64 continuation decisions and 256 primitive actions. If any required competitor is incomplete/capped, abort the experimental comparison and fall back to the unchanged existing planner; never compare a short partial result against a complete twelve-hour result.
- Instrument continuation decisions separately from candidate generation, caps and same-state CPU. Before integration, compare the fixed diagnostic sequence and this heuristic's actual continuation; if they disagree about feasibility/payback, keep the feature in research rather than quietly overriding needs.
