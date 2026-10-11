# Physical households and allegiance

## Agreed behaviour

- Two farmers form one household; the miller and baker form another. Other citizens remain independent.
- Pool members' coins and goods. Keep properties individually owned, with household access.
- Use the first farmer's home and the miller's home as shared homes. Place food and drink at home and starting ingredients at the relevant workplaces.
- Goods retain an owner while stored at a location or carried by a citizen. Members consume home supplies and workplace ingredients directly.
- Production output stays at its workplace; open-air gathering output stays with its worker.
- Citizens execute actions on behalf of their household. There is no separate household action planner.
- One-hop allegiance adds household wealth wellbeing to personal wellbeing with initial weight 1.0. Household wealth excludes members' wellbeing and counts assets once.
- Designated workplace staffing sums each worker's projected ingredient needs: restock below two working hours towards twelve working hours per worker.
- Replenishing and listing excess may occur only first in a goal pair. Each has a six-hour owner-wide cooldown after successful completion, with claims only for committed work.
- Replenishment includes delivering home. Food and drink reserves cover all supplied members; workplace ingredients cover all assigned workers.
- Employment, guilds, facility capacity and demand histories remain deferred.

## Step 1: physical storage and transport

- Add explicit collection and deposit actions using existing storage ownership and tax reservation invariants.
- Make prediction and execution preserve physical stock and location access.
- Include owned stored ingredients as production supply sources, with travel and collection.
- Validate transfer conservation, unavailable stock, reservations and prediction isolation.
- Obtain independent review, resolve findings and commit.

## Step 2: shared household economy

- Add household identities, membership, shared accounts, allegiance and designated workplaces.
- Apply shared ownership to production, purchases, market listings, tax reservations and transport.
- Consume supplies at home/work, and retain produced goods at their workplace.
- Sum worker stock requirements and household reserves without double counting.
- Implement first-goal maintenance restrictions, six-hour successful-completion cooldowns and committed-task claims.
- Validate shared spending, resource contention, scoring, tax ownership and household transport.
- Obtain independent review, resolve findings and commit.

## Step 3: observable integration

- Create the test households and pool/place starting resources.
- Display household membership, stock, wealth, maintenance state and distinct personal/decision wellbeing.
- Update save format, validation and round-trip coverage. Older version files remain untouched and unsupported.
- Run integrated simulation, workspace checks and independent review; resolve findings and commit.

## Progress

- Step 1 implemented and independently reviewed. Collection/deposit actions preserve physical ownership and leave tax-reserved stock at its source; existing direct transfers retain reservation-moving support. Production input preparation collects stored ingredients even when total owned stock already exceeds the replenishment trigger.
- Workspace tests, final simulation tests, formatting, strict workspace Clippy and diff checks passed. Regression coverage includes stock conservation, immutable predictions, reserved stock protection and fully stored production inputs without purchasing money.
- Workplace output relocation and direct site consumption remain in Step 2, alongside the corresponding household ownership, tax and production-history changes.
