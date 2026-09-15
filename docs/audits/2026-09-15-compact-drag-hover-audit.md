# Compact Drag / Hover Audit — 2026-09-15

## Scope

This audit records the Windows compact-mode investigation after PR #4 introduced a dedicated hover list window. It is a handoff document, not an implementation plan for production code.

Repository state at audit time:
- repo: `teeeeooo/port-lens`
- local: `/Users/sunjaekim/Developer/port-lens`
- branch: `feature/compact-app-hover`
- tested code baseline: `43d4c15645c763d1dc9fc5caa20b81cf3802b0c0`
- PR #4: OPEN, targets `main`, do not merge yet
- Windows Bundle #26: run `34911753874`

Bundle #26 manual Windows result:
- compact `Open` → expanded main UI: PASS
- compact drag: FAIL; drag may begin late after the cursor has already moved away, then continue with a persistent cursor/window offset
- if hover list is visible when dragging starts, the hover window stays at its old screen position while the compact bar moves

Bundle #26 Portable artifact ZIP SHA-256:
`21ecb89c8808e604a188d99f5988f52fbfbf4feee9a76cbe03685553d96d5f14`

Extracted Portable EXE SHA-256:
`e57062eb04542f694376aa1766a7a52d7c0048451b7724176de02aeb82b83b60`

## What is already validated

The two-window hover architecture solved the original compact-hover flicker. The main compact HWND stays fixed at 276×46; `compact-hover` is a persistent hidden/reused `WebviewWindow` that renders the App list.

Other fixes that must be preserved:
- compact lower-corner clipping is correct
- main startup size/work-area clamping is correct
- StartupGate removed the transient `state not managed` startup race
- Bundle #26 confirmed the compact `Open` deadlock fix; expansion now completes normally
- runtime reattach/lifecycle safety from PR #3 must remain unchanged

The Bundle #25 Open hang was traced to synchronous BubbleController mutex re-entry during native expand operations. `bubble::expand()` now transitions compact state and releases that mutex before changing decorations, size, position, or focus. `Moved`/`Resized` no longer reacquire BubbleController. Do not regress this.

## Current drag path

The compact bar currently uses Tauri's built-in deep drag region:

`mousedown → Tauri drag.js → plugin:window|start_dragging → tauri-runtime-wry → Tao drag_window()`

Installed Tauri 2.11.5 `drag.js` invokes `start_dragging` directly from a left-button `mousedown` on a drag region. The current Port Lens mouse-down handler does only local React hover-state cleanup; it sends no Port Lens IPC before Tauri's drag handoff.

Installed Tao 0.35.3 handles Windows `drag_window()` by reading the cursor with `GetCursorPos()`, releasing capture, then posting `WM_NCLBUTTONDOWN` with `HTCAPTION` to the main HWND. This means native drag starts after the WebView/Tauri dispatch path and samples the cursor at that later point, not from the original DOM mouse-down coordinates.

## Audit finding: late native capture

The remaining drag symptom is consistent with a late native capture boundary:

1. DOM receives mouse-down while cursor is still over the compact bar.
2. Tauri's WebView command crosses into the runtime/event-loop path.
3. The user moves the mouse before Tao executes `drag_window()`.
4. Tao samples the newer cursor position and begins Windows caption dragging from there.
5. The window then follows the cursor with the already-created fixed offset.

This is the leading explanation, not yet a production-grade proof. It is supported by:
- Bundle #26 reproducing the issue after Port Lens mouse-down IPC/hide work was removed
- direct inspection of Tauri/Tao source
- Tauri Windows issue #10767, which reports drag-region focus/capture anomalies, lost mouse-up events, and stuck dragging behavior

Do not continue adding timing patches around `data-tauri-drag-region`; validate a different Windows drag substrate instead.

## Audit finding: hover window does not follow the main window

`compact-hover` is a separate native `WebviewWindow` owned/parented by `main`. On Windows, ownership keeps z-order/lifetime relationships but does not make a child/owned top-level window automatically track the owner's position.

Electron documents the same platform behavior explicitly: on macOS a child keeps its relative position when the parent moves, while on Windows and Linux it does not.

Therefore the observed `hover list stays behind while compact bar moves` behavior is expected for this architecture unless Port Lens explicitly repositions the hover HWND. Repositioning it continuously during drag would reintroduce drag-time native work and is not recommended.

## Reference implementation findings

### Token Lens v2

Local Token Lens v2 is Tauri and does not use Tauri `start_dragging()` for its floating bubble. It uses pointer capture, a 4 px movement threshold, repeated `move_floating_bubble` calls, backend `cursor_position()`, and `set_position()`.

This avoids the Tauri native caption-drag path, but it is not the preferred Port Lens answer because per-pointermove IPC can create its own responsiveness/catch-up failure mode.

### Electron / Chromium style

Electron's custom-window guidance uses draggable/non-draggable regions for frameless windows. WebView2 exposes a similar experimental/native feature through `msWebView2EnableDraggableRegions`, enabling CSS `app-region: drag` and `app-region: nodrag`.

This is the preferred first PoC because it could move drag ownership into WebView2 rather than Tauri's JS `start_dragging()` command path.

Risk: Tauri `additionalBrowserArgs` has multi-WebView constraints. Tauri documents that WebViews with different browser arguments require different data directories, and upstream issues report failures/deadlocks when additional browser args are used with multiple WebviewWindows. Port Lens has both `main` and `compact-hover`, so this must be tested explicitly before integration.

### Native Win32 fallback

If WebView2 draggable regions are not viable, the second PoC should use a Port Lens-owned native drag surface/hit-test layer without subclassing the WRY/WebView2 child HWND. Bundle #24's child-window interception must not be revived.

## Decision and next-session sequence

Do not modify PR #4 production drag code first. Use an isolated branch/worktree for substrate PoCs.

PoC-A — WebView2 draggable regions:
- enable `msWebView2EnableDraggableRegions` safely for the relevant Windows WebView configuration
- use `app-region: drag` for the compact body and `app-region: nodrag` for `Open`
- verify behavior with both `main` and `compact-hover` WebViews present
- explicitly test creation/startup/freeze behavior caused by browser args/data-directory rules

PoC-A acceptance:
- mouse-down followed by immediate movement has no visible dead period
- no persistent cursor/window offset
- no catch-up jump under normal or fast repeated drag
- `Open` remains responsive and expands correctly
- hover-list presence cannot block drag start
- no multi-WebView startup freeze/deadlock

## PoC-A1 manual result

PoC branch/worktree: `poc/compact-webview2-drag-regions` / `/Users/sunjaekim/Developer/port-lens-poc-a`. PoC-A1 commit `46409e2` replaced `data-tauri-drag-region` with WebView2 `app-region: drag/nodrag` and used the `msWebView2EnableDraggableRegions` browser flag. Windows Bundle run `34915796285` built successfully.

Manual Windows result: FAIL overall.
- compact hover causes a brief freeze / micro-stutter
- the separate hover list usually fails to open and appears only intermittently
- drag still pauses before the window jumps/catches up
- the previous persistent cursor/window offset after late snap is no longer reproduced
- if the hover list is visible when drag starts, the separate hover HWND stays at its old position while the compact bar moves; it disappears after mouse release

The release-time hide is explained by current `window_state::schedule_persist()`: while the left button is down it defers compact persistence; after release it hides the hover panel and persists the compact position. This does not satisfy the desired drag-start hide policy.

A source audit found that pinned WRY `0.55.1` already calls `ICoreWebView2Settings9::SetIsNonClientRegionSupportEnabled(true)` on Windows. Therefore the browser feature flag is redundant in this stack.

The browser-args-free control commit `ac954cd` / Windows Bundle `34917910453` built successfully and also failed manual acceptance:
- hover popup opens at most initially; after any drag it never opens again
- drag still pauses/jumps and has visible residual stutter
- the old persistent cursor/window offset remains absent

This closes PoC-A as **FAILED**. Removing `additionalBrowserArgs` did not resolve the drag defect, so the remaining failure belongs to the WebView2 non-client draggable-region approach in this product context, not to the duplicate feature flag. The post-drag hover lockout is consistent with the current frontend state machine: `prepareNativeBubbleDrag()` sets `bubbleHoverSuppressUntilReentry=true`, while reset depends on the DOM `mouseleave` path in `endBubbleHover()`. Native non-client dragging can bypass that expected DOM sequence. Fixing that secondary state bug would not rescue the failed drag substrate, so no further PoC-A patching is planned.

PoC-B — independent native drag surface:
- do not subclass or consume mouse messages from the WRY/WebView2 child HWND
- keep the `Open` interactive area outside the native drag surface
- prove immediate native capture and normal button behavior in a minimal Windows package before integration

B1 commit `3d9b12e` implemented a Port Lens-owned 34 px transparent native child HWND over the left compact grip. Windows Bundle `34921608950` built successfully, but manual compact entry failed before drag testing: both compact-entry routes reached native grip creation, and `CreateWindowExW` returned NULL. The in-app Minimize path surfaced `failed to create native compact drag surface: 작업을 완료했습니다. (0x000000)`; the title-bar minimize path appeared unresponsive because that event path discarded the same collapse error.

The follow-up audit found that the B1 grip uses `WS_CHILD | WS_EX_LAYERED`. Microsoft requires layered child windows to run under a Windows 8-aware application manifest. Tauri's default application manifest preserves Common Controls v6 but does not declare Windows compatibility. B1.1 therefore changes only the application manifest: commit `d7da866` preserves Common Controls v6 and adds Windows 8, 8.1, and Windows 10/11 `supportedOS` entries through `tauri_build::WindowsAttributes::app_manifest()`. Windows Bundle `34924195134` built and uploaded NSIS/MSI/portable artifacts successfully.

B1.1 manual Windows result partially passes but drag acceptance still fails:
- in-app Minimize → compact: PASS
- native title-bar minimize → compact: PASS
- post-drag hover list reopening: PASS
- native grip drag: FAIL; pause/jump and residual stutter remain
- hover-list drag-start hide is not directly testable in this PoC because the native grip and WebView hover region are intentionally disjoint

Code audit after this result shows no obvious Port Lens per-move work on the critical path: persistence and taskbar z-order maintenance already defer while the left button is down. The remaining B1 path calls `SendMessageW(parent, WM_NCLBUTTONDOWN, HTCAPTION, ...)` synchronously from the child grip's `WM_LBUTTONDOWN` handler. `SendMessageW` does not return until the target window procedure finishes processing the message, while caption movement enters the Windows move/size modal loop. This creates a nested synchronous modal-loop handoff inside the child WndProc and became the leading B1-specific suspect.

B1.2 tests that hypothesis directly. The already-validated expanded-size fix is synchronized into the PoC as baseline commit `c4a821c`; B1.2 itself is isolated in `e20e751`. It changes the child grip handoff to `PostMessageW(parent, WM_NCLBUTTONDOWN, HTCAPTION, original_mouse_down_screen_point)`, so the child WndProc returns immediately instead of synchronously nesting the parent's move/size modal loop. Because queued delivery no longer has a meaningful return point for drag completion, B1.2 removes the immediate `native-compact-drag-ended` emit rather than falsely treating message enqueue as drag end. Windows Bundle `34931769814` built and uploaded NSIS/MSI/portable artifacts successfully.

B1.2 manual Windows result: **FAIL**.
- movement is still not immediate after mouse-down
- the same pause → jump behavior remains
- cursor/window offset appears during drag
- catch-up jump remains during/release-side movement
- residual micro-stutter remains under repeated slow/fast drag
- compact `Open` after drag remains responsive and expands normally

This closes PoC-B as **FAILED**. Replacing `SendMessageW` with queued `PostMessageW` did not improve the drag defect, so synchronous child-WndProc re-entry is not the primary cause. The remaining failure belongs to the native-grip → Windows caption/move modal-loop substrate in this product context. Do not continue testing `SC_MOVE`, further `WM_NCLBUTTONDOWN` timing variants, or drag-end lifecycle work on this failed substrate.

Combined PoC-A/B conclusion: both WebView2 non-client draggable regions and Port Lens-owned native-grip caption dragging still exhibit the pause/jump class of failure. Any PoC-C must avoid the Windows caption/move modal loop entirely while also preserving the existing bans on WRY/WebView2 child HWND subclassing and per-frame `SetWindowPos`.

Hover policy for any future substrate remains: visible hover list should hide at drag start; do not make the separate hover HWND continuously follow the compact bar during drag.

## Separate finding: expanded-window size drift

While manually repeating compact → `Open`, the expanded main window was observed to grow wider after each cycle. This is independent of PoC-A/B drag work and predates the native-grip experiment.

Source audit identified an outer/inner size semantic mismatch introduced with compact mode in `bed908d`:
- `bubble::collapse()` captured `window.outer_size()` into the transient expanded-window state
- `bubble::expand()` restored that value through `window.set_size(...)`
- Tauri runtime maps `WindowMessage::SetSize` to Tao `window.set_inner_size(...)`
- therefore the previously captured title bar/border dimensions were reapplied as client size, and Windows added the non-client frame again

The persisted expanded-window path had the same defect: `window_state::capture_expanded_bounds()` stored `outer_size()` while `restore_initial()` restored the value with `set_size()`.

PR #4 fixes both paths by defining width/height as inner/client dimensions. `ExpandedWindow.size` is renamed to `inner_size`; transient compact restore captures `inner_size()`, and persisted `expandedBounds` now captures `inner_size()` as logical dimensions. Existing preview settings are backward-compatible through `expandedBoundsAreInner`: missing/false means legacy outer-size semantics, so startup subtracts the currently measured non-client frame once and rewrites the actual post-fit inner bounds with the marker set true.

Regression gate result before resuming B1.2: **PASS** on PR #4 commit `918b377` / Windows Bundle `34929848061`.
- repeated compact → `Open` cycles remained stable with no cumulative width/height growth
- expanded close/relaunch restore remained stable
- ordinary manual resize persisted correctly across restart

The size-drift issue is therefore closed and B1.2 may proceed independently on the isolated PoC-B branch.

## Do-not-regress constraints

- Keep PR #4 unmerged until a Windows manual gate passes.
- Preserve the separate hover-window architecture unless a PoC demonstrates a safer replacement; it solved the original hover flicker.
- Preserve the Bundle #26 Open deadlock fix.
- Preserve startup gating, rounded compact clipping, main-window work-area clamping, taskbar overlap policy, and saved-position behavior.
- Preserve PR #3 runtime reattach identity checks and lifecycle suppression exactly.
- Do not reintroduce per-`Moved` hover hide, per-frame `SetWindowPos`, or the Bundle #24 WRY child subclass.
- Do not report a PoC as adopted until the Windows package is manually exercised.

## References

- Tauri Windows drag-region issue #10767: https://github.com/tauri-apps/tauri/issues/10767
- Tauri drag-region behavior discussion #11945: https://github.com/tauri-apps/tauri/issues/11945
- Microsoft WebView2 browser flags (`msWebView2EnableDraggableRegions`): https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/webview-features-flags
- Tauri configuration `additionalBrowserArgs`: https://v2.tauri.app/reference/config/
- Tauri multi-WebView/additional-browser-args issue #11144: https://github.com/tauri-apps/tauri/issues/11144
- Tauri multiple-window `additional_browser_args` deadlock issue #15014: https://github.com/tauri-apps/tauri/issues/15014
- Electron BrowserWindow parent/child platform behavior: https://www.electronjs.org/docs/latest/api/browser-window
- Stack Overflow — owned window movement must be explicitly realigned: https://stackoverflow.com/questions/9722520/how-to-move-an-owned-window-together-with-the-owner-in-wpf
- Stack Overflow — frameless Win32 drag via `WM_NCHITTEST` / `HTCAPTION`: https://stackoverflow.com/questions/35522488/moving-frameless-window-by-dragging-it-from-a-portion-of-client-area

## Handoff rule

The next session must first verify the actual git/PR state and read `STATE.md`, `BACKLOG.md`, and this audit. PoC-A and PoC-B are both closed as failed. Do not resume WebView2 `app-region` work or Windows caption/modal-loop drag tuning. The next technical step is an audit-only PoC-C design pass constrained to a materially different drag substrate; do not modify PR #4 production drag code until that audit selects a candidate and the candidate later passes Windows manual validation.
