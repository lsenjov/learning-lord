# Core prices and daily caravans

## Agreed behaviour

- Labour baseline: 10 coins per production hour, with a 30% markup at every recipe stage.
- Core price per output unit: `(base hours * 10 + input quantities * input core prices) * 1.3 / output quantity`.
- Use base recipe durations and mean gathering output. Core prices exclude skills, tax, travel, local price movements and purchase rounding; actual payments retain integer coin rounding.
- Core prices are starting market prices and stable references for caravan thresholds.
- At 04:00, close the old market period and update local prices, then run caravans.
- Below half core price, export half the listed local stock at existing asking prices, crediting sellers and applying normal income taxes.
- Above 1.5 times core price, import half the closed day's affordable unfulfilled demand, less remaining caravan stock. Caravan orders are at the public market and retain the fixed 1.5-times-core price.
- Threshold comparisons are strict. Half quantities round up once per good. Allocate exports proportionally between sellers with deterministic remainder allocation.
- Caravans are external: exported goods leave town, export payments enter town, and import purchase payments leave town.
- Imported stock counts as supply. Local trades, exports, import deliveries and purchases from caravans have distinct records and display; exports do not become local consumer demand for replenishing imports.
- Caravans are an outlet and supply intervention, not a hard price floor or ceiling.

## Implementation steps

1. [x] Add reusable core-price calculation, use it for initial prices, and record reference prices and assumptions. Workspace tests passed across the full sweep and targeted fixture reruns; independent review approved.
2. [x] Add external market ownership, daily caravan settlement, fixed import orders, tax/payment integration and separate history counters. Settlement, boundaries, rounding, immutable predictions, cancellation, overflow and daily progression tests pass; independent review approved.
3. [x] Display core prices, thresholds, caravan listings and distinct trade metrics/history. Documentation updated and independent review approved. Final workspace validation: 347 tests passed, two manual benchmarks ignored; formatting and strict Clippy passed.

## Validation focus

- Every current recipe and good resolves deterministically to a positive finite core price.
- Local reference prices and caravan asking prices remain distinct.
- Daily caravans execute once per boundary; long advances behave consistently with smaller steps.
- Export quantities and payments conserve listed goods, settle across multiple sellers and apply income taxes without inventing citizens.
- Imports do not duplicate outstanding stock or feed export demand into their own replenishment rule.
- Whole goods, fragmented orders and integer coin rounding retain the existing per-seller/per-good semantics.
- Citizen predictions can purchase caravan stock without authoritatively running caravans or crediting fictional residents.

## Core price reference

Prices below use the formula above, recursively valuing inputs. Bulk quoted prices are per kg; counted goods are per item. Values are rounded here for display only. Every recipe currently has one output.

| Good | Unit | Core price (coins) |
| --- | --- | ---: |
| Berries | kg | 650.00000000 |
| Wheat | kg | 65.00000000 |
| Flour | kg | 159.79166667 |
| Wood | kg | 65.00000000 |
| Water | kg | 32.50000000 |
| Bread | loaf | 20.05520833 |
| Berry pie | pie | 64.49895833 |
| Flax | kg | 65.00000000 |
| Thread | kg | 117.00000000 |
| Cloth | kg | 184.60000000 |
| Flax block | block | 12.49950000 |
| Flax garment | garment | 136.49480000 |
