# Carpenter bid markup

Implement and independently review one coherent change, then commit:

- Price carpenter labour and inn household earnings targets from recurring household costs per earning hour, excluding reserve replenishment. Preserve inn occupancy assumptions, price adjustments and maintenance floors, plus reserve protection and financing checks. Saved inn prices remain unchanged.
- Submit bids at estimated materials plus labour, multiplied by a uniform integer percentage from 111 through 120, rounding only the final total upward. Advance construction RNG once per submitted bid. Preserve sealed prices during revalidation and save/load.
- Keep the existing client estimate buffer and save format. Verify endpoint rounding, financing, RNG stability, saved bids and awarded prices; run simulation tests and desktop compilation.

Status: implementation and independent review complete. Simulation library and integration tests passed (757 passed, 7 ignored); desktop cargo check and the targeted bid RNG regression passed. Formatting and whitespace checks passed.

Review noted that an unaffordable markup does not advance the random sequence. Retries therefore repeat that percentage until another operation advances it; existing submitted bids never reroll. This submitted-bid-only behavior is retained.
