# Collectives and entity navigation

## Agreed behaviour

- Add a movable, resizable, closeable Collectives window using the existing window manager.
- Show Households, Guilds and Businesses tabs; only Households has live data for now.
- Keep a list on the left and selected household details on the right: members, shared home, coins, wealth, stocks grouped by location, accessible workplaces with their actual owners, and maintenance cooldowns/claimants.
- Keep the household link and a brief shared-finances summary on citizens; move the detailed shared stock and maintenance breakdown into Collectives.
- Citizen, collective, good and location references navigate through stable entity IDs. Open or focus the existing category window and select the target, without creating duplicate windows or closing the source.
- Good links open the Market overview. Collective links choose the correct category tab. Names have a consistent visible link treatment and pointer feedback.
- Split references into distinct clickable targets when a row names multiple entities. Keep selectors and form controls acting as controls.
- Preserve unfinished tax and tariff edits across navigation. Reset stale selections and drafts when changing universes.
- Guild and business simulation behaviour is out of scope.

## Step 1: navigation and collectives

- Add a shared typed navigation mechanism and link presentation.
- Add the Collectives window, household list/detail view, placeholder tabs, and citizen membership links.
- Preserve location drafts by scope while navigating and maintain existing market policy drafts.
- Verify selection/focus, window reuse, household data, draft preservation and universe resets.
- Independently review, resolve findings and commit.

## Step 2: references across existing views

- Replace plain entity references with typed links in citizen details/plans/decisions/history, location details/stocks/taxes and market listings/requests/recipe descriptions wherever an entity is named.
- Preserve information density and current control behaviour; avoid rebuilding unchanged content or increasing idle CPU use.
- Update documentation; verify navigation between categories and visual layout in the native app.
- Run relevant tests, formatting and strict Clippy; independently review, resolve findings and commit.

## Progress

- Step 1 complete: Collectives, typed navigation, focus handling and draft preservation implemented and independently reviewed. Desktop tests, formatting and strict Clippy pass.
- Step 2 complete: typed links cover citizen details, plans, decisions and history; location stocks and taxes; and market participants, locations and recipe ingredients. Existing controls and display data are preserved.
- Independent review found no remaining high or medium issues. The full 129-test desktop suite and an additional action-link regression pass; formatting and strict Clippy pass.
- Native screenshots checked Collectives, citizens, plans, market listings/overview and location stocks, including resized windows. Adjusted inline spacing and used an ASCII recipe separator for the current font.
