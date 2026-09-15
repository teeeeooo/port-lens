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

## Next action

Start with an isolated Windows PoC, not production integration:

1. PoC-A: WebView2 `msWebView2EnableDraggableRegions` + CSS `app-region: drag/nodrag`.
2. Validate immediate capture, no fixed offset/catch-up, responsive `Open`, and multi-WebView startup stability.
3. If PoC-A fails, PoC-B: an independent Win32 drag surface that does not subclass the WRY/WebView2 child HWND.
4. Integrate only the Windows-manually-validated mechanism into PR #4.

Before any new work, verify git/PR state against the repository; do not assume this file alone proves merge or CI state.
