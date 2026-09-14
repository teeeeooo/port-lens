# Port Lens Backlog

Last updated: 2026-09-14

Items are ordered by intended implementation sequence after the current stability merge.

## 1. Compact position polish

Status: merged to `main` via PR #2 (`249d111`). Windows CI and bundle packaging passed; implementation is now the baseline.

Goal: allow the Windows compact bubble to sit directly above the taskbar without an artificial gap.

Implemented behavior:
- Port Lens still clamps compact position to monitor `work_area()`.
- Windows uses a 0 px compact edge margin so the bubble can touch the work-area boundary without covering the taskbar.
- Non-Windows desktop behavior retains the existing 12 px edge margin.

Preferred Windows policy:
- keep the monitor work area as the safe boundary
- reduce the compact edge margin to 0 px
- do not allow the bubble to cover the taskbar

Acceptance criteria:
- bubble can touch the top edge of the taskbar/work-area boundary
- no taskbar overlap
- left/right/top clamping remains valid
- multi-monitor movement remains correct
- mixed-DPI movement remains correct
- saved compact position restores and clamps correctly after restart

This records the PR #2 baseline. PR #4 intentionally supersedes the Windows drag boundary by allowing user-selected taskbar overlap with topmost protection.

Suggested branch: `feature/compact-position-polish`

## 2. Verified managed runtime reattach

Status: merged to `main` via PR #3 (`bee52e3`). Native macOS/Windows CI passed, and manual Windows restart → verified reattach → Stop validation passed. The transient lifecycle-verification message observed during Stop was also removed before merge.

Implemented behavior:
- persisted listener/root generation and command identity are required before reattach
- PID reuse and unrelated listeners are rejected
- reattached Stop re-verifies root identity immediately before termination
- Stop / Restart temporarily suppress reattach for the affected App
- monitored-listener/reattach refresh completes before managed runtime state is read
- lifecycle actions do not expose provisional reattach state in the App card

## 3. Compact hover App list

Status: active on `feature/compact-app-hover` via PR #4, retargeted to `main`. The fixed compact window + dedicated hover window architecture remains. Windows Bundle #24's experimental WebView-child Win32 drag interception failed manual validation: drag could be completely unavailable and Open could hang the app. That path is retired. The current revision returns to Tauri's known-working deep drag region and instead removes drag-time interference from hover hiding, taskbar z-order maintenance, and deferred position persistence. Local frontend build, 33 Rust tests, native clippy, and Windows-target clippy pass. Awaiting fresh Windows CI/package and manual validation before merge.

Goal: keep the compact bar stable while exposing a lightweight registered-App list on hover and preserving reliable drag behavior.

Implemented behavior:
- the main compact HWND remains fixed at 276×46 and is never resized by hover
- a persistent hidden `compact-hover` WebViewWindow renders the App list and is shown/hidden as needed
- each registered App shows only its name and a green/gray status dot
- visible height is capped at 8 Apps; additional Apps remain scrollable
- hover data is reused from the main monitoring state; the panel does not run its own listener/process scan
- a render revision handshake prevents the native hover window from being shown before its requested DOM is committed
- Windows compact drag uses Tauri's built-in `data-tauri-drag-region="deep"` path again, with start-dragging capability scoped only to the main window
- the experimental WebView-child `WM_LBUTTONDOWN` subclass path from Bundle #24 is removed completely
- the 4 px threshold, pointer capture, pointermove IPC, manual per-frame `SetWindowPos`, and per-`Moved` hover `hide()` paths remain removed
- if the hover panel is already visible at mouse-down, the frontend sends one asynchronous hide request; otherwise drag start adds no Port Lens IPC work
- the taskbar z-order keeper checks physical left-button state and skips all z-order `SetWindowPos` work for the full button-down interval
- deferred compact-position persistence also waits for left-button release, so a pause while still holding the drag cannot trigger clamp/save/z-order work mid-drag
- `WindowEvent::Moved` only marks native movement active; the final stable position is persisted after release and normal z-order protection then resumes
- the compact WebView surface uses rounded `clip-path` clipping on both shell and bar, restoring all four rounded corners
- main-window startup defaults to 1020×680 and clamps both saved/default bounds into the active monitor work area
- frontend state-dependent commands are blocked behind a startup-ready gate until all backend managed state is installed
- the list remains display-only; no App lifecycle controls are added

Manual acceptance gate:
- repeated hover open/close does not flash or recreate the compact bar
- hover list content/status is current and scrolling works above 8 Apps
- normal and fast/repeated drag begins immediately and follows the pointer without lag, drop, catch-up jump, or a fixed cursor/window offset
- the Open button remains clickable and never initiates drag
- dragging while the hover panel is visible hides the panel cleanly before native movement
- taskbar overlap, multi-monitor/mixed-DPI movement, and saved-position restore remain correct

Suggested branch: `feature/compact-app-hover`

## Deferred housekeeping

After the active items above stabilize, review version bump/release notes and decide whether the next packaged release remains 0.3.x or advances based on accumulated feature scope.
