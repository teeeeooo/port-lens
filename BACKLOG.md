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

Status: **CLOSED / FAILED**.

Deep audit: `docs/audits/2026-09-15-compact-drag-poc-c-deep-audit.md`.

Selected control:
- Winit 0.30.10 fixed a documented Windows ~500 ms title-bar pause by queuing a dummy `WM_MOUSEMOVE` with `lParam=0` before/while the native move-size loop takes over
- Port Lens' Tao 0.35.3 still forwards the original non-client lParam; direct source checks show Tao 0.36.0 and 0.37.0 retain the same behavior
- B1.2 did not test this fix because its parent `WM_NCLBUTTONDOWN` still passed through Tao's old synthetic-mousemove handler
- PoC-C should reuse the isolated native-grip/B1.2 path and add only an immediately queued `WM_MOUSEMOVE` with `WPARAM(0), LPARAM(0)`; no delays, dependency upgrade, hover-lifecycle change, or persistence change in the same control
- a PoC-C PASS does **not** authorize restoring production `data-tauri-drag-region` unchanged: Tao `handle_os_dragging()` still has a separate malformed `WM_NCLBUTTONDOWN` coordinate encoding path, so production integration must retain a validated native initiation path or separately validate a Tao coordinate-packing correction

Implementation and runtime evidence:
- isolated branch/worktree: `poc/compact-winit-wakeup` / `/Users/sunjaekim/Developer/port-lens-poc-c`
- commit `ee2533a` changes only `src-tauri/src/native_drag.rs` and adds the zero-lParam synthetic `WM_MOUSEMOVE` immediately after the existing correctly packed queued caption handoff
- local gates: frontend build PASS, fmt PASS, clippy PASS, Rust tests 36/36 PASS, diff check PASS
- Windows Bundle `34941498309` / full SHA `ee2533a54347b2ed883bcbdd573dc1579d901571`: SUCCESS; portable/MSI/NSIS upload PASS
- manual Windows result: first movement still not immediate; pause → jump remains; cursor offset became worse; catch-up jump became worse; micro-stutter remains; `Open` remains PASS

Conclusion:
- the Winit-style zero-lParam wake-up does not fix the Port Lens Tao-managed caption path and materially worsens offset/jump behavior on the tested Windows machine
- permanently close `WM_NCLBUTTONDOWN` / `HTCAPTION` / `SC_MOVE` / Send-vs-Post / zero-lParam wake-up / `data-tauri-drag-region` tuning for this issue
- preserve PoC-C branch/worktree as evidence; do not integrate its code into PR #4

## 5. PoC-D non-caption drag audit

Status: **D0 FAILED / D0.1 FAILED / D0.2 DIAGNOSTIC PASS / D0.3 RUNTIME PASS WITH COLD-START RESIDUAL / D0.4 IMPLEMENTED, WINDOWS MANUAL VALIDATION PENDING**.

History correction: `78c006c` → `cca33ce` already implemented the core Token Lens pattern in Port Lens: pointer capture, 4 px threshold, grab-ratio-only IPC, backend current-cursor resampling, and manual positioning. `cca33ce` Windows Bundle `34802709558` built successfully, while repository docs still showed Windows manual validation pending; no preserved manual FAIL for that final form was found. `7144042` removed it during the separate-hover refactor based on expected IPC/manual-movement risk rather than recorded Windows failure. See `docs/audits/2026-09-15-compact-drag-history-token-lens-audit.md`.

Audit two candidates before implementation:
- **D0 — Token Lens exact-style control:** current fixed 276×46 main HWND + separate `compact-hover`; DOM pointer capture and 4 px threshold; repeated lightweight move invoke carries only grab ratio; backend samples current cursor at execution time; Windows movement is position-only; drag-time persistence/z-order work is suppressed.
- **D1 — native captured-pointer control:** Port Lens-owned native grip; `WM_LBUTTONDOWN` → `SetCapture` and snapshot cursor/parent rect; same-thread `WM_MOUSEMOVE` → position-only `SetWindowPos`; `WM_LBUTTONUP` / `WM_CAPTURECHANGED` terminates and persists/clamps once.

Audit result: no architectural blocker was found for D0, so D0 was selected before D1. D0 Windows validation on `f474b0d` / Bundle `34946164626` removed the persistent cursor offset and kept hover-hide/Open correct, but jump/catch-up remained. D0.1 changed only raw Windows `SetWindowPos` to Tauri `window.set_position()` and made jump/catch-up worse. D0.2 restored raw position-only `SetWindowPos` and suspended compact polling/UI refresh; Windows validation reported jump/catch-up **completely gone**, proving background refresh/render contention is the decisive variable. D0.3 restored continuous idle compact polling and gated it only during drag; Windows validation passed all normal-drag criteria, but a full process restart still produces one first-drag hitch/jump. D0.4 moves the gate to non-button pointerdown to isolate a startup-refresh result race before the 4 px threshold. D1 remains deferred unless D0.4 fails or introduces a new blocker.

## 6. PR #4 integration and manual gate

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
