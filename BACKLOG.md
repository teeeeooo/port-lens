# Port Lens Backlog

Last updated: 2026-09-15

Only current actionable/deferred work is kept here. Completed implementation history belongs in `STATE.md` and audit documents.

## 1. Compact drag substrate PoC-A

Status: NEXT. Do not modify PR #4 production drag path first.

Goal: determine whether WebView2 native draggable regions remove the Windows late-capture/fixed-offset failure seen with Tauri `data-tauri-drag-region`.

Work:
- create an isolated branch/worktree from the current PR #4 baseline
- test `msWebView2EnableDraggableRegions`
- use CSS `app-region: drag` for compact body and `app-region: nodrag` for `Open`
- keep both `main` and `compact-hover` WebViews present during the test
- account for Tauri `additionalBrowserArgs` / data-directory constraints

Acceptance gate:
- immediate movement after mouse-down
- no fixed cursor/window offset
- no normal/fast repeated-drag catch-up
- `Open` remains responsive
- no multi-WebView startup/create freeze or deadlock
- hover list is hidden at drag start rather than tracked during movement

## 2. Compact drag substrate PoC-B

Status: BLOCKED on PoC-A failure.

If WebView2 draggable regions are not viable, test an independent Win32 drag surface/hit-test layer owned by Port Lens.

Constraints:
- do not subclass/consume WRY or WebView2 child-window mouse messages
- keep `Open` outside the drag surface
- prove Windows behavior in a minimal package before PR #4 integration

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
