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

## 4. PoC-C drag-substrate audit

Status: **NEXT / AUDIT FIRST**.

Before implementation, survey alternatives that satisfy all of these constraints:
- do not use Tauri/Tao `start_dragging()` or Windows caption/move modal-loop dragging
- do not use WebView2 non-client draggable regions (`app-region`)
- do not subclass/intercept the WRY/WebView2 child HWND
- do not implement per-frame `SetWindowPos` drag or a high-frequency JS→IPC position loop
- keep `Open`, hover, taskbar overlap, saved position, mixed-DPI/multi-monitor, and compact sizing behavior intact

The audit should compare concrete Windows/Tauri reference implementations and select at most one narrowly scoped PoC-C candidate before code changes.

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
