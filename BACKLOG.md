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
- B1.1 `d7da866` / Windows Bundle `34924195134`: manifest-only Windows compatibility control; build/package/artifact upload PASS; manual runtime result pending

Initial scope:
- do not subclass or consume WRY/WebView2 child-window mouse messages
- keep `Open` and normal WebView hover handling outside the native drag surface
- prefer a minimal native grip surface first, not an overlay across the whole compact bar
- hide `compact-hover` at native drag start; do not continuously track the hover HWND during drag
- prove immediate native capture and normal button/hover behavior in a Windows package before integration

Acceptance gate:
- immediate movement on mouse-down + movement; no pause/jump or cursor offset
- repeated slow/fast drag remains smooth
- hover popup opens before drag and recovers after drag
- visible hover popup hides at drag start
- `Open` remains responsive; no startup/deadlock regression

## 3. PR #4 integration and manual gate

Status: BLOCKED on a passing drag-substrate PoC.

After one PoC passes:
- integrate only that mechanism into `feature/compact-app-hover`
- preserve the current hover-window flicker fix and Bundle #26 Open deadlock fix
- run Windows/macOS CI and Windows packaging
- manually validate drag, hover-hide-on-drag, Open, taskbar overlap, multi-monitor/mixed-DPI movement, and saved-position restore
- merge PR #4 only after explicit manual approval

## Deferred housekeeping

After PR #4 stabilizes, review version bump/release notes and prune stale compact-drag experiments from documentation if they are no longer needed for audit history.

Detailed rationale and external references:
`docs/audits/2026-09-15-compact-drag-hover-audit.md`
