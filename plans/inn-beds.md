# Paid inn beds

## Approved scope

Implement paid beds first, then stop for inspection. Meals and travellers are deferred. Property sales and autonomous property purchasing are deferred by user choice; the town allocates property through grants.

Inns are household-owned businesses operated by an innkeeper. Private household beds and supplies remain separate from guest services. Use existing buildings and beds, with explicit guest-bed configuration in normal town management; do not generate free premises or tools when selecting a profession. Support multiple competing inns without a special two-inn case.

## Pricing and bookings

- Each inn has its own eight-hour sleep-session price. Starting price covers estimated room/bed maintenance plus the innkeeper's daily earnings target, at 70% paid occupancy. Reuse existing construction/repair and household budgeting calculations where appropriate. Convert daily costs to session costs consistently with the existing eight-hour session and available bed-hours.
- Once daily, compare trailing three days of paid bed-hours with available guest-bed-hours. Above 80% increases price by 5%; below 60% decreases it by 5%; otherwise hold. Use actual observed availability, including partial days and closures, rather than assuming every bed existed for the entire window. No competitor undercutting.
- Minimum price covers maintenance per booking. Earnings target initializes price but is not a permanent floor. Preserve fractional price adjustments internally so integer coin rounding does not prevent adaptation; payments use existing coin conventions.
- Citizens compare available inns by price, travel and discomfort benefit, including outdoor sleep as an alternative. Equal alternatives should not systematically favor one inn. Prediction must not spend money, reserve live beds or consume live randomness.
- Accepted bookings atomically reserve a specific usable guest bed and lock the quoted payment. Recheck funds/availability at acceptance. Release the reservation on sleep completion or interruption; handle cancelled travel and departing guest/host without stranded claims. No automatic refund policy was agreed; use simple upfront session payment, disclose it in the relevant tooltip. Paid occupancy measures actual sleep time, not speculative reservations or empty travel time.
- Guest sleep has normal tiredness recovery and actual-bed discomfort relief. Private beds are not sold, and bookings grant only the necessary access, not general rights to household storage/equipment.

## Work units

1. **Business, bookings and pricing.** Add innkeeper role, explicit guest-bed configuration, household-owned inn state, safe booking/payment/access lifecycle, occupancy history and daily price adjustment. Integrate persistence and departure/property cleanup. Provide meaningful simulation tests and an independently reviewed commit.
2. **Citizen choice and management UI.** Integrate paid-bed alternatives into real and predicted sleep planning. Add compact normal management controls/readouts for guest beds, price and occupancy, with details in tooltips. Ensure dev profession selection exposes innkeepers. Test competing inns, scarce beds, unaffordable offers, outdoors/private alternatives, travel/interruption, worker mutations and save/load. Independently review and commit, then hand back.

## Constraints and validation

Keep implementation simple and reuse existing object claims, worker mutation and accounting patterns. Stop for discussion if a new policy/architecture choice materially changes scope or a small specification change would substantially simplify implementation. Old saves may be invalidated; no migration required. No meals, new recipes, travellers, property listings, town haulers or unsolicited UI redesign.

Test rate thresholds, rolling availability, price floors and rounding, no-carpenter estimates, multi-inn competition and tie handling, conservation of money, concurrent bookings, actual paid occupancy, cancellation/departure, state refresh and persistence validation. Run appropriate workspace tests/checks. Known baseline failures are documented in household-population.md. Resolve valid high/medium review findings then request a fresh review; fix low documentation findings and report other low issues. Commit each completed reviewed unit.

## Progress

- Plan recorded; implementation pending.
