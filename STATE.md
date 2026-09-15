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
`docs/audits/2026-09-15-compact-drag-hover-audit.md`

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

## PoC-B current state

- isolated branch/worktree: `poc/compact-native-drag-surface` / `/Users/sunjaekim/Developer/port-lens-poc-b`
- B1 commit `3d9b12e`: Port Lens-owned 34 px native child HWND grip; no WRY/WebView2 child subclassing
- Windows Bundle `34921608950`: build PASS, but manual compact entry FAIL because `CreateWindowExW` returned NULL
- B1.1 commit `d7da866`: manifest-only control adding Windows 8/8.1/10+ compatibility while preserving Common Controls v6
- Windows Bundle `34924195134`: build/package/artifact upload PASS
- B1.1 manual result: compact entry PASS from both in-app Minimize and native title-bar minimize; post-drag hover reopen PASS
- B1.1 drag result: **FAIL**; native grip still has visible pause/jump and residual drag stutter
- hover-list drag-start hide cannot be evaluated in B1 because hover and native grip are intentionally separate hit regions

## Expanded-window size drift

A separate lifecycle bug was found while repeatedly testing compact → `Open`: the expanded main window grows wider on each cycle. Root cause is an outer/inner size mismatch that predates PoC-B and has existed since compact mode was introduced in `bed908d`.

- compact collapse saved `outer_size()` but expand restored it through `set_size()`, which sets the inner/client size
- persisted `expandedBounds` had the same mismatch: `outer_size()` was stored and later restored through `set_size()`
- production fix on PR #4 standardizes both transient and persisted expanded dimensions on inner/client size
- legacy preview settings are marked with `expandedBoundsAreInner`; old settings without the marker are converted once by subtracting the current non-client frame and then rewritten using inner-size semantics
- Windows manual gate: **PASS** on commit `918b377` / Bundle `34929848061`; compact → `Open` repetition, restart restore, and manual-resize restore were reported stable

## Next action

1. Keep PR #4 open and unmerged.
2. Return to isolated PoC-B and synchronize the already-validated size-drift fix as a separate baseline commit.
3. Run one final B1.2 control that queues the parent caption-drag handoff instead of using blocking `SendMessageW`; keep this drag change isolated from the size fix.
4. If pause/jump remains in B1.2, close PoC-B; do not keep tuning Windows caption/modal-loop drag.
5. Only after a drag substrate passes Windows manual validation should it be integrated into PR #4.

Before any new work, verify git/PR state against the repository; do not assume this file alone proves merge or CI state.
