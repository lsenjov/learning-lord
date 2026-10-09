# Market tabs and trade colours

## Step 1

Keep the goods sidebar and move selected-good information into Overview, Listings, and Caravan policy tabs. Preserve live readouts and history preview/pinning. Stack chart bodies by completed export, local, and caravan-purchase volumes, retain price and quantity wicks, and outline the partial day. Verify tab visibility, selected-good updates, hidden chart interaction, segment geometry, and native rendered layouts. Review before committing.

## Completed

- Title and tabs remain fixed above independently scrolling panels. Policy drafts and pinned history survive tab switches; hidden controls no longer receive input. Listings include current affordable unfulfilled demand.
- Candle segments use completed exports, local trades, and purchases from caravans. Import deliveries remain separate. A cyan outline identifies the partial day.
- All 108 desktop tests pass, including new tab, focus, pinning, and segment geometry regressions. Workspace build, formatting, strict Clippy, and whitespace checks pass.
- Native screenshots were inspected at wide and 720 by 480 logical window sizes. The market content minimum width was removed to prevent narrow-window clipping. Evidence: `/tmp/ll-market-tabs-top.png`, `/tmp/ll-market-tabs-{0,1,2}.png`, and `/tmp/ll-market-tabs-small-{0,1,2}.png`. Chart samples came from an isolated visual harness; no harness code is included in the app.
- Independent review of the final changes is complete with no findings.
