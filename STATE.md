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

WRY `0.55.1` already enables WebView2 non-client-region support through `ICoreWebView2Settings9`, so `additionalBrowserArgs` is not required for `app-region`. A single control variant removes only the redundant browser args before abandoning PoC-A.

## Next action

1. Validate PoC-A control commit `ac954cd` (same `app-region`, browser args removed) with Windows Bundle run `34917910453`.
2. If drag still pauses/jumps, close PoC-A as failed; do not keep tuning WebView2 drag-region timing.
3. Then start PoC-B: an independent Win32 drag surface that does not subclass the WRY/WebView2 child HWND.
4. Integrate only a Windows-manually-validated mechanism into PR #4.

Before any new work, verify git/PR state against the repository; do not assume this file alone proves merge or CI state.
