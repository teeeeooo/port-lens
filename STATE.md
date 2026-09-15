# Port Lens State

Last updated: 2026-09-15

## Current baseline

- product: Port Lens v0.3.0 preview, Tauri/Rust + React/TypeScript
- merged `main`: `bee52e3` (PR #3 runtime reattach/lifecycle hardening)
- active branch: `feature/compact-app-hover`
- PR #4: OPEN, targets `main`, **do not merge yet**
- Windows-tested code baseline: `43d4c15645c763d1dc9fc5caa20b81cf3802b0c0`
- Windows Bundle #26: run `34911753874`

Detailed compact drag/hover evidence and references:
- `docs/audits/2026-09-15-compact-drag-hover-audit.md`
- `docs/audits/2026-09-15-compact-drag-poc-c-deep-audit.md`

## Windows manual state

- compact hover flicker: PASS after split into dedicated `compact-hover` window
- compact `Open` → expanded main UI: PASS on Bundle #26
- compact drag: FAIL; native drag can start late after cursor movement and then retain a fixed cursor/window offset
- hover-visible drag: FAIL UX; separate hover window remains at its previous screen position while the compact bar moves
- compact bottom rounding: PASS
- startup `state not managed` race: PASS
- main window initial/work-area sizing: PASS

The remaining drag issue persists after Port Lens removed mouse-down hover IPC, per-move hover hiding, drag-time persistence, and drag-time z-order work. Current evidence points to the Tauri/Tao Windows drag substrate rather than the App hover scan/render path.

## Active constraints

- Do not patch the current `data-tauri-drag-region` path further before substrate PoCs.
- Do not reintroduce Bundle #24 WRY/WebView2 child HWND subclassing.
- Preserve the Bundle #26 compact expansion deadlock fix.
- Preserve the separate hover-window architecture unless a safer replacement is proven; it solved the original flicker.
- On drag start, the intended hover policy is `hide`, not continuous cross-window tracking.
- Preserve taskbar overlap/topmost behavior, saved-position restore, mixed-DPI/multi-monitor behavior, startup gating, and compact clipping.
- Preserve PR #3 runtime reattach identity/suppression logic unchanged.

## PoC-A manual result

- PoC-A1 branch/worktree: `poc/compact-webview2-drag-regions` / `/Users/sunjaekim/Developer/port-lens-poc-a`
- PoC-A1 commit: `46409e2`
- Windows Bundle run: `34915796285` (build PASS)
- manual result: FAIL overall
  - hover can cause a brief compact freeze / micro-stutter
  - hover list usually does not open; it appears only intermittently
  - drag still pauses and then jumps/catches up
  - previous persistent cursor/window offset after snap is no longer reproduced
  - if hover list is visible when drag begins, the list remains at its old position while `main` moves; it disappears after drag release

WRY `0.55.1` already enables WebView2 non-client-region support through `ICoreWebView2Settings9`, so `additionalBrowserArgs` is not required for `app-region`. The browser-args-free control `ac954cd` / Windows Bundle `34917910453` also failed manual acceptance: the first hover popup could open, but after any drag the hover popup never returned; drag still paused/jumped and retained visible micro-stutter.

PoC-A is therefore **CLOSED / FAILED**. The control rules out redundant browser args as the primary cause. The drag-after-hover failure is also consistent with `prepareNativeBubbleDrag()` setting `bubbleHoverSuppressUntilReentry=true` while WebView2 non-client drag does not reliably deliver the DOM `mouseleave` path that clears it. Do not spend more time patching this failed substrate.

## PoC-B manual result

- isolated branch/worktree: `poc/compact-native-drag-surface` / `/Users/sunjaekim/Developer/port-lens-poc-b`
- B1 `3d9b12e` / Bundle `34921608950`: build PASS, compact entry FAIL because layered child HWND creation failed without a compatibility manifest
- B1.1 `d7da866` / Bundle `34924195134`: manifest control fixed compact entry, but native grip drag still paused/jumped and stuttered
- validated size-drift fix synchronized into PoC-B as `c4a821c`
- B1.2 `e20e751` / Bundle `34931769814`: replaced blocking `SendMessageW` with queued `PostMessageW` while keeping the native grip/caption-drag architecture otherwise fixed
- B1.2 manual result: **FAIL**
  - movement is not immediate after mouse-down
  - pause → jump remains
  - cursor/window offset appears
  - catch-up jump remains during/release-side movement
  - residual drag stutter remains
  - `Open` after drag remains PASS

PoC-B is therefore **CLOSED / FAILED**. The B1.2 control rules out synchronous child-WndProc re-entry as the primary cause. Generic `WM_NCLBUTTONDOWN`, `HTCAPTION`, `SC_MOVE`, or Send/Post timing tuning remains closed. A later PoC-C deep audit found one evidence-backed exception: Winit 0.30.10 fixed the Windows ~500 ms title-bar pause by posting a synthetic `WM_MOUSEMOVE` with `lParam=0`, while Tao 0.35.3/0.36.0/0.37.0 still forward the original non-client lParam. That exact compatibility control is the only caption-loop experiment reopened.

## Expanded-window size drift

A separate lifecycle bug was found while repeatedly testing compact → `Open`: the expanded main window grows wider on each cycle. Root cause is an outer/inner size mismatch that predates PoC-B and has existed since compact mode was introduced in `bed908d`.

- compact collapse saved `outer_size()` but expand restored it through `set_size()`, which sets the inner/client size
- persisted `expandedBounds` had the same mismatch: `outer_size()` was stored and later restored through `set_size()`
- production fix on PR #4 standardizes both transient and persisted expanded dimensions on inner/client size
- legacy preview settings are marked with `expandedBoundsAreInner`; old settings without the marker are converted once by subtracting the current non-client frame and then rewritten using inner-size semantics
- Windows manual gate: **PASS** on commit `918b377` / Bundle `34929848061`; compact → `Open` repetition, restart restore, and manual-resize restore were reported stable

## Next action

1. Keep PR #4 open and unmerged; production drag behavior remains unresolved.
2. PoC-C deep audit is complete. The selected control is the Winit 0.30.10-compatible zero-lParam modal wake-up documented in `docs/audits/2026-09-15-compact-drag-poc-c-deep-audit.md`.
3. Implement PoC-C only on a new isolated branch/worktree derived from the proven native-grip/B1.2 baseline: keep the queued `WM_NCLBUTTONDOWN/HTCAPTION` handoff and add an immediately queued `WM_MOUSEMOVE` with `WPARAM(0), LPARAM(0)`. Change no other drag/lifecycle variable.
4. Manual decision gate: full PASS → design integration/lifecycle; partial PASS (initial pause fixed, residual stutter only) → isolate Port Lens `Moved` callback work; unchanged FAIL → permanently close caption/modal-loop work and evaluate native captured-pointer positioning as the fallback.
5. Preserve the validated size-drift fix, hover-window architecture, Open deadlock fix, taskbar behavior, mixed-DPI/multi-monitor handling, startup gating, and PR #3 runtime lifecycle protections. Integrate nothing into PR #4 until a candidate passes Windows manual validation.

Before any new work, verify git/PR state against the repository; do not assume this file alone proves merge or CI state.
