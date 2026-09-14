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

Status: active on `feature/compact-app-hover` via PR #4, retargeted to `main`. Latest A-D interaction fixes are implemented and pass local frontend/Rust/native+Windows-target clippy validation. Awaiting fresh Windows package and manual validation before merge.

Goal: keep the compact bar stable while exposing a lightweight registered-App list on hover and preserving reliable drag behavior.

Implemented behavior:
- hovering expands the native compact window upward after a short delay
- each registered App shows only its name and a green/gray status dot
- visible height is capped at 8 Apps; additional Apps remain scrollable
- compact bar and hover list render as separate rounded translucent surfaces
- hover close hides the list before the native window shrinks
- drag never resizes the native window while the pointer gesture is active
- dragging an already-expanded hover moves the expanded geometry as one unit and collapses after release
- drag pointer offsets are relative to the whole native compact window, preserving the grab point while expanded
- Windows drag uses position-only `SetWindowPos`; per-move z-order refresh was removed
- intentional taskbar overlap remains protected by the periodic topmost keeper and a final drag-end correction
- the list remains display-only; no App lifecycle controls are added

Manual acceptance gate:
- no compact-bar flash/recreation effect on repeated hover open/close
- no dropped drag while moving quickly or repeatedly
- hover-open drag collapses cleanly after release without position drift
- taskbar overlap and multi-monitor placement remain correct
- saved compact position restores after restart

Suggested branch: `feature/compact-app-hover`

## Deferred housekeeping

After the active items above stabilize, review version bump/release notes and decide whether the next packaged release remains 0.3.x or advances based on accumulated feature scope.
