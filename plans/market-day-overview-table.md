# Market day overview table

## Scope

- Replace the selected-day prose metrics with a table: Total, Local, Import, Export.
- Show completed traded quantity, rounded coins paid, volume-weighted paid price, tariffs/subsidies, seller receipts before income tax, and unsold stock.
- Keep the day/time range, accepted local quote, reference start/end, unmet demand, and caravan arrivals separate.
- Preserve hover previews and pinned days; avoid rebuilding unchanged table contents.
- Reuse existing history data; no save format changes.

## Verification

- Focused desktop market-history tests passed (17): rounded prices, import deliveries versus purchases, subsidies, counted goods, selection behavior, and reference start/end.
- Desktop suite: 141 passed, 1 manual test ignored. Strengthened rendered-table selection test also passed after review.
- Locked desktop build, strict desktop/all-targets Clippy, formatting and diff checks passed.
- Independent review found no high/medium issues; its low test-coverage finding was fixed and confirmed resolved.
- Actual window appearance was not visually verified; rendered ECS content is covered by selection tests.

## Status

- Complete. No simulation or save format changes.
