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

Status: active on `feature/compact-app-hover` via PR #4, retargeted to `main`. The earlier same-HWND hover/drag implementation has been replaced by the fixed compact window + dedicated hover window + native drag architecture. The latest drag revision removes the Windows JS/IPC drag-start boundary entirely. Local frontend build, 33 Rust tests, native clippy, and Windows-target clippy pass. Awaiting fresh Windows CI/package and manual validation before merge.

Goal: keep the compact bar stable while exposing a lightweight registered-App list on hover and preserving reliable drag behavior.

Implemented behavior:
- the main compact HWND remains fixed at 276×46 and is never resized by hover
- a persistent hidden `compact-hover` WebViewWindow renders the App list and is shown/hidden as needed
- each registered App shows only its name and a green/gray status dot
- visible height is capped at 8 Apps; additional Apps remain scrollable
- hover data is reused from the main monitoring state; the panel does not run its own listener/process scan
- a render revision handshake prevents the native hover window from being shown before its requested DOM is committed
- Windows compact drag no longer starts from React or a Tauri command; native subclasses intercept non-button `WM_LBUTTONDOWN` directly on the main/WebView HWND tree
- the exact mouse-down message position comes from `GetMessagePos()`, so drag startup does not resample the cursor after any IPC delay
- the native handler hides the hover HWND asynchronously and immediately enters the Windows move loop with synchronous `WM_NCLBUTTONDOWN(HTCAPTION)`; the right-side Open-button region is passed through unchanged
- the 4 px threshold, pointer capture, pointermove IPC, delayed `GetCursorPos()`, queued `PostMessageW`, manual per-frame `SetWindowPos`, and per-`Moved` hover `hide()` paths remain removed
- taskbar z-order maintenance is suspended by a native drag-active flag while the move loop owns the HWND; final position is persisted once when that loop returns, then z-order protection resumes
- non-Windows builds keep the existing one-shot Tauri native drag fallback
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
