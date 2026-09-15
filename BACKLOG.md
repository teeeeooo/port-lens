# Port Lens Backlog

Last updated: 2026-09-15

Only current actionable/deferred work is kept here. Completed implementation history belongs in `STATE.md` and audit documents.

## 1. Compact drag substrate PoC-A

Status: **CLOSED / FAILED**.

Evidence:
- A1 `46409e2` / Windows Bundle `34915796285`: hover micro-stutter, hover list instability, drag pause/jump; persistent cursor/window offset removed
- control `ac954cd` / Windows Bundle `34917910453`: browser args removed, but drag pause/jump and micro-stutter remain; first hover can open, then hover never returns after any drag
- no further WebView2 `app-region` timing/flag tuning

## 2. Compact drag substrate PoC-B

Status: **MANUAL VALIDATION**.

Isolated branch/worktree: `poc/compact-native-drag-surface` / `/Users/sunjaekim/Developer/port-lens-poc-b`.

Evidence so far:
- B1 `3d9b12e` / Windows Bundle `34921608950`: package PASS, manual compact entry FAIL at native child-window creation (`CreateWindowExW` returned NULL)
- B1.1 `d7da866` / Windows Bundle `34924195134`: manifest-only Windows compatibility control; compact entry PASS from both minimize paths; post-drag hover reopen PASS; native grip drag still FAILS with pause/jump and residual stutter
- `c4a821c`: synchronizes the separately validated expanded-window size fix into the PoC baseline
- B1.2 `e20e751` / Windows Bundle `34931769814`: queues the parent caption-drag message with `PostMessageW`; build/package PASS, Windows manual drag validation pending
- B1.2 intentionally omits a fake immediate drag-ended event; if drag smoothness passes, drag-end lifecycle is a separate follow-up rather than part of this control

Initial scope:
- do not subclass or consume WRY/WebView2 child-window mouse messages
- keep `Open` and normal WebView hover handling outside the native drag surface
- prefer a minimal native grip surface first, not an overlay across the whole compact bar
- hide `compact-hover` at native drag start; do not continuously track the hover HWND during drag
- prove immediate native capture and normal button/hover behavior in a Windows package before integration

B1.2 immediate gate:
- immediate movement on mouse-down + movement; no pause/jump or cursor offset
- repeated slow/fast drag remains smooth with no residual catch-up lag
- `Open` still enters the expanded UI normally after drag

Full adoption gate after a smooth substrate is proven:
- implement a real drag-end signal, then validate hover popup hide/recovery
- preserve startup/deadlock, taskbar, saved-position, and multi-monitor/mixed-DPI behavior

## 3. Expanded main-window size drift

Status: **CLOSED / WINDOWS MANUAL PASS**.

Root cause and fix:
- compact collapse and persisted window-state capture used `outer_size()`
- both restore paths used `set_size()`, which restores the inner/client size and therefore added the Windows frame again on every cycle
- PR #4 commit `918b377` stores inner/client size for both transient compact restore state and persisted `expandedBounds`
- legacy outer-size `expandedBounds` are converted once and rewritten with `expandedBoundsAreInner=true`
- Bundle `34929848061` manual validation passed: repeated compact → `Open`, restart restore, and manual-resize restore stayed stable

## 4. PR #4 integration and manual gate

Status: BLOCKED on a passing drag-substrate PoC.

After one drag PoC passes:
- integrate only the validated drag mechanism into `feature/compact-app-hover`
- preserve the current hover-window flicker fix and Bundle #26 Open deadlock fix
- run Windows/macOS CI and Windows packaging
- manually validate drag, hover-hide-on-drag, Open, taskbar overlap, multi-monitor/mixed-DPI movement, and saved-position restore
- merge PR #4 only after explicit manual approval

## Deferred housekeeping

After PR #4 stabilizes, review version bump/release notes and prune stale compact-drag experiments from documentation if they are no longer needed for audit history.

Detailed rationale and external references:
`docs/audits/2026-09-15-compact-drag-hover-audit.md`
