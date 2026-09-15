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

Status: **CLOSED / FAILED**.

Isolated branch/worktree: `poc/compact-native-drag-surface` / `/Users/sunjaekim/Developer/port-lens-poc-b`.

Evidence:
- B1 `3d9b12e` / Windows Bundle `34921608950`: package PASS, compact entry initially failed because the layered child HWND required a Windows compatibility manifest
- B1.1 `d7da866` / Windows Bundle `34924195134`: manifest fixed compact entry; native grip drag still paused/jumped and stuttered
- `c4a821c`: synchronized the separately validated expanded-window size fix into the PoC baseline
- B1.2 `e20e751` / Windows Bundle `34931769814`: replaced blocking `SendMessageW` with queued `PostMessageW`
- B1.2 manual result: movement still not immediate; pause/jump remained; cursor/window offset appeared; catch-up jump and residual stutter remained; `Open` after drag stayed PASS

Conclusion:
- synchronous child-WndProc re-entry was not the primary cause
- close the native-grip → `WM_NCLBUTTONDOWN`/`HTCAPTION` Windows caption/modal-loop approach
- do not continue with `SC_MOVE`, Send/Post timing variants, or drag-end lifecycle work on this substrate

## 3. Expanded main-window size drift

Status: **CLOSED / WINDOWS MANUAL PASS**.

Root cause and fix:
- compact collapse and persisted window-state capture used `outer_size()`
- both restore paths used `set_size()`, which restores the inner/client size and therefore added the Windows frame again on every cycle
- PR #4 commit `918b377` stores inner/client size for both transient compact restore state and persisted `expandedBounds`
- legacy outer-size `expandedBounds` are converted once and rewritten with `expandedBoundsAreInner=true`
- Bundle `34929848061` manual validation passed: repeated compact → `Open`, restart restore, and manual-resize restore stayed stable

## 4. PoC-C Winit compatibility control

Status: **AUDIT COMPLETE / CONTROL SELECTED**.

Deep audit: `docs/audits/2026-09-15-compact-drag-poc-c-deep-audit.md`.

Selected control:
- Winit 0.30.10 fixed a documented Windows ~500 ms title-bar pause by queuing a dummy `WM_MOUSEMOVE` with `lParam=0` before/while the native move-size loop takes over
- Port Lens' Tao 0.35.3 still forwards the original non-client lParam; direct source checks show Tao 0.36.0 and 0.37.0 retain the same behavior
- B1.2 did not test this fix because its parent `WM_NCLBUTTONDOWN` still passed through Tao's old synthetic-mousemove handler
- PoC-C should reuse the isolated native-grip/B1.2 path and add only an immediately queued `WM_MOUSEMOVE` with `WPARAM(0), LPARAM(0)`; no delays, dependency upgrade, hover-lifecycle change, or persistence change in the same control
- a PoC-C PASS does **not** authorize restoring production `data-tauri-drag-region` unchanged: Tao `handle_os_dragging()` still has a separate malformed `WM_NCLBUTTONDOWN` coordinate encoding path, so production integration must retain a validated native initiation path or separately validate a Tao coordinate-packing correction

Immediate Windows gate:
- movement begins on the first slow cursor movement
- pause → jump and fixed cursor/window offset disappear
- repeated slow/fast drag has no catch-up jump; record residual micro-stutter separately
- `Open` after drag remains PASS

Decision tree:
- full PASS → design production integration and real drag-end/hover recovery separately
- partial PASS (dead period fixed, residual stutter remains) → isolate Port Lens/Tauri `Moved` callback work next
- unchanged FAIL → permanently close caption/modal-loop work

Fallback after an unchanged FAIL only:
- native grip `SetCapture` + event-driven same-thread `SetWindowPos`, with cleanup on `WM_LBUTTONUP`/`WM_CAPTURECHANGED`
- still prohibited: timer-driven positioning and high-frequency JS → IPC → backend movement loops
- the previous blanket `SetWindowPos` ban is therefore narrowed to those asynchronous/high-frequency architectures; direct native movement is not approved unless PoC-C fails first

## 5. PR #4 integration and manual gate

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
