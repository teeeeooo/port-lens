# Port Lens Backlog

Last updated: 2026-09-15

Only current actionable/deferred work is kept here. Completed implementation history belongs in `STATE.md` and audit documents.

## 1. Compact drag substrate PoC-A

Status: CONTROL VALIDATION. Production path remains untouched.

PoC-A1 `46409e2` / Windows Bundle `34915796285` built successfully but failed manual acceptance: hover micro-stutter, hover list usually absent, and drag still pauses then jumps. The old persistent cursor/window offset was removed, so the substrate changed behavior but is not acceptable.

Current work:
- control commit `ac954cd` removes only redundant `additionalBrowserArgs`
- keep CSS `app-region: drag` for compact body and `app-region: nodrag` for `Open`
- rely on WRY 0.55.1 native `ICoreWebView2Settings9` non-client-region enablement
- validate Windows Bundle run `34917910453`
- if pause/jump remains, stop PoC-A and move to PoC-B

Acceptance gate:
- immediate movement after mouse-down
- no fixed cursor/window offset
- no normal/fast repeated-drag catch-up
- `Open` remains responsive
- no multi-WebView startup/create freeze or deadlock
- hover list is hidden at drag start rather than tracked during movement

## 2. Compact drag substrate PoC-B

Status: BLOCKED pending the single PoC-A control result.

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
