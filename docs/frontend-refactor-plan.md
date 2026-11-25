# Frontend refactor plan

Applied with the **frontend-refactor-planner** skill.

## Current state

- `SettingsPage.tsx` is the largest surface: appearance, connection, ping,
  sorting, updates, core and logs in one file.
- `ServersPage.tsx` mixes filtering, bulk selection, sorting, latency rendering
  and rows. The latency presentation was extracted into `ServerLatency.tsx`.
- `RoutingPage.tsx` contains two large dialogs and profile-card markup.
- Shared M3 primitives are already separated: `Button`, `Card`, `Segmented`,
  `Switch`, `Icon`, `Dialog`-style native dialogs.

## Phase 1 — stabilize

- Keep the current UI contracts and tests green.
- Add the a11y fixes: skip link, radiogroup keyboard navigation, progressbar
  semantics, heading hierarchy, timeline/latency labels.
- Do not split files while behavior is still changing.

## Phase 2 — extract responsibilities

1. `ServerLatency` — done.
2. `ServerRow` — extract row markup and callbacks from `ServersPage`.
3. `ServerToolbar` — search, filter, sort and bulk actions.
4. `SettingsSection` components — appearance, connection, ping, updates,
   core/logs.
5. `RoutingProfileDialog` — import and edit dialogs.

Each extraction must preserve DOM roles, labels and keyboard behavior.

## Phase 3 — simplify state

- Keep server filtering and bulk selection local to `ServersPage`.
- Keep persistent choices (`servers.sort`, exclusions, ping settings) in the
  store/DB, never duplicated as component defaults.
- Introduce one typed ping-history model before adding charts/jitter/loss.

## Phase 4 — polish

- Add Playwright journeys: import, exclude, bulk exclude, connect, update check.
- Run axe on each page and fix violations.
- Check 200% zoom and `prefers-reduced-motion`.

## Done criteria

- No page component above ~350 lines.
- No duplicated table/row markup.
- Keyboard-only navigation covers nav, toolbar, rows and dialogs.
- Axe reports no serious violations on the main pages.
- `npm run typecheck`, build and Playwright journeys pass.
