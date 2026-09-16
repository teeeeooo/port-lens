# Compact Drag PoC-C Deep Audit — 2026-09-15

## Scope and decision

This audit reopens root-cause analysis after both PoC-A and PoC-B failed Windows manual validation. No production drag code is changed by this audit.

The main finding is an upstream implementation divergence that materially changes the next experiment: Winit 0.30.10 fixed the Windows title-bar ~500 ms modal-loop pause by posting a dummy `WM_MOUSEMOVE` with `lParam = 0`, while Port Lens' Tao 0.35.3 still reposts the original `WM_NCLBUTTONDOWN` lParam. Tao 0.36.0 and 0.37.0 retain the same behavior.

Therefore the earlier blanket decision to abandon every caption/move-loop control is narrowed. One final, evidence-backed compatibility control is justified before moving to a fully manual positioning substrate.

Selected PoC-C candidate: reproduce the Winit 0.30.10 zero-lParam wake-up behavior on the already isolated native-grip PoC without changing PR #4 production code.

## Evidence baseline

- production feature branch: `feature/compact-app-hover`; PR #4 remains OPEN and unmerged
- PoC-A WebView2 non-client drag regions: CLOSED / FAILED
- PoC-B native grip + `WM_NCLBUTTONDOWN/HTCAPTION`: CLOSED / FAILED under B1/B1.1/B1.2
- B1.2: `e20e751` / Windows Bundle `34931769814`
- B1.2 manual result: delayed start, pause/jump, cursor offset, catch-up jump, and residual stutter all remain; `Open` remains PASS
- expanded-window size drift: CLOSED / PASS on `918b377` / Bundle `34929848061`

## What PoC-A/B already rule out

The repeated failures make several earlier suspects low probability rather than merely untested:
- hover-list scan/render work is not the primary drag-start blocker
- Port Lens mouse-down IPC is not required to reproduce the defect
- synchronous `SendMessageW` re-entry is not the primary cause; B1.2 `PostMessageW` behaves the same
- WebView2 feature flags are not the primary cause; WRY already enables non-client regions
- Tao's late `GetCursorPos()` sampling is not sufficient to explain the defect; PoC-B supplied the original packed screen point and still failed
- persistent window-state writes and taskbar z-order refresh are not occurring on every active drag move

## Shared failure path discovered

Despite different initiation mechanisms, production and PoC-B converge on the same Tao-managed top-level HWND non-client move path.

Production:
`DOM mousedown → Tauri start_dragging → Tao drag_window → WM_NCLBUTTONDOWN/HTCAPTION → DefWindowProc move-size loop`

PoC-B:
`native grip WM_LBUTTONDOWN → Send/Post WM_NCLBUTTONDOWN/HTCAPTION → Tao top-level WndProc → DefWindowProc move-size loop`

Tao 0.35.3 handles `WM_ENTERSIZEMOVE` / `WM_EXITSIZEMOVE` and, on `WM_NCLBUTTONDOWN` with `HTCAPTION`, posts a synthetic `WM_MOUSEMOVE` before native default processing. PoC-B bypassed Tao's drag initiation but did not bypass this top-level WndProc behavior.

WRY 0.55.1 adds another shared layer on the same parent HWND. Its WebView2 parent subclass calls `MoveFocus(PROGRAMMATIC)` on `WM_ENTERSIZEMOVE` and `ICoreWebView2Controller::NotifyParentWindowPositionChanged()` on every `WM_MOVE` / `WM_MOVING`. Therefore a native caption drag of the real Port Lens HWND necessarily exercises both Tao's move-loop/event dispatch and WRY's WebView2 parent-position notification even when Port Lens application persistence is deferred.

Tauri issue #10767 independently reports Windows drag-region focus/capture anomalies and lost mouse events in this general path. The more specific ~500 ms native title-bar pause is independently documented by Winit 0.30.10 and is the stronger match for the initial dead period seen here.

## Critical upstream divergence: Winit 0.30.10

A direct local source diff between Winit 0.30.9 and 0.30.10 shows the targeted fix behind Winit's release note: “fixed ~500 ms pause when clicking the title bar during continuous redraw.”

Winit 0.30.9 effectively did:
```rust
PostMessageW(window, WM_MOUSEMOVE, 0, lparam)
```

Winit 0.30.10 changed this to:
```rust
let lparam = 0;
PostMessageW(window, WM_MOUSEMOVE, 0, lparam)
```

The new upstream comment is explicit: `WM_NCLBUTTONDOWN` carries screen-coordinate semantics while `WM_MOUSEMOVE` uses client-area coordinates; forwarding the non-client lParam as-is did not reliably cancel the modal pause, whereas `0` did. This is independently corroborated by Mixxx's Windows native event filter, which posts `WM_MOUSEMOVE(hwnd, 0, 0)` on `WM_NCLBUTTONDOWN/HTCAPTION` specifically to avoid the same 500 ms event-loop wait.

## Tao comparison

Port Lens currently resolves Tao 0.35.3. Its Windows handler still does:
```rust
PostMessageW(window, WM_MOUSEMOVE, WPARAM(0), lparam)
```

The same source check was repeated against downloaded Tao 0.36.0 and the current Tao 0.37.0 package. Both still forward the original `lparam`; neither contains Winit 0.30.10's `lParam = 0` compatibility fix.

A dependency upgrade alone is therefore not a targeted solution to this drag defect. As of this audit, Tauri's current stable core is 2.11.5; its ecosystem currently lists Tao 0.36.0, and a direct standalone Tao 0.37.0 source check also retains the old synthetic-mousemove argument. Neither Tao 0.36.0 nor 0.37.0 contains Winit's zero-lParam compatibility change.

A second source-level anomaly exists in Tao's `handle_os_dragging`. `WM_NCLBUTTONDOWN` expects its `lParam` to contain signed screen x/y coordinates packed into the low/high words. Tao instead constructs a local `POINTS` and casts the **address of that local structure** into `LPARAM` before `PostMessageW`. Because `PostMessageW` is asynchronous, that is not only different from the documented scalar message encoding but also points at stack storage that has no valid pointer semantics for this message. Tao's own Windows code elsewhere uses a `MAKELPARAM(x, y)` helper for coordinate-bearing messages, which makes the divergence explicit.

Winit 0.30.10 still has the same initial pointer-address pattern, so its 500 ms fix is independent of this anomaly. More importantly for Port Lens, PoC-B already supplied a correctly packed original screen point and still reproduced pause/jump/stutter. Therefore malformed initial `WM_NCLBUTTONDOWN` coordinates are a credible contributor to the production path's fixed cursor offset, but they are **not sufficient** to explain the full failure and are not the PoC-C variable.

## Why B1.2 did not test the Winit fix

B1.2 changed only the child-grip handoff from blocking `SendMessageW` to queued `PostMessageW` and provided the original mouse-down screen point.

When the parent receives that `WM_NCLBUTTONDOWN`, Tao 0.35.3 still executes its own old synthetic `WM_MOUSEMOVE` behavior using the non-client lParam. Therefore B1.2 ruled out synchronous child-WndProc nesting, but it did **not** reproduce Winit 0.30.10's modal-loop wake-up fix.

This is the key reason one narrowly scoped caption-loop control is reopened despite PoC-B previously being closed.

## Tauri runtime amplification audit

Tauri issue #15569 is relevant but not a direct match. On a near-identical 2.11.x / WRY 0.55.1 / Tao 0.35.3 Windows stack, continuous undecorated resize progressively degraded because Tauri's resize helper called `SetWindowRgn` on every `WM_SIZE`.

Its controlled bisection is useful: `SetWindowPos` alone produced only constant slight overhead and no progressive degradation; `SetWindowRgn` was the necessary trigger.

Port Lens compact mode sets `resizable(false)` before `decorations(false)`. In Tauri runtime-wry 2.11.4 that removes the undecorated resize helper and does not reattach it while resizable is false. Therefore #15569 demonstrates that Tauri's Windows message pipeline can amplify native-operation costs, but its specific per-`WM_SIZE` region update is not on Port Lens' compact drag critical path.

Port Lens still receives a Tao `WindowEvent::Moved` for each `WM_WINDOWPOSCHANGED`. Tauri runtime maps that event directly, and the app's global window handler calls `get_webview_window("main")`, `is_minimized()`, then `schedule_persist()`. Actual compact persistence is deferred while the left button is down, so this is application callback churn rather than per-move disk I/O.

In parallel, WRY's parent subclass calls WebView2 `NotifyParentWindowPositionChanged()` on `WM_MOVE` / `WM_MOVING`. Consequently there are at least three per-move layers once the real HWND starts moving: Win32/Tao window processing, WRY/WebView2 host notification, and Port Lens/Tauri `Moved` callback work. Any one of them may amplify residual micro-stutter even if the initial modal dead period is fixed.

This distinction is important: the Winit compatibility control targets **drag-start latency / modal pause** only. If that control converts the failure from pause→jump into immediate-but-slightly-stuttery motion, the next experiment must remove Port Lens `Moved` callback work from the active compact drag path before changing the drag substrate again. If necessary after that, a minimal Tauri-vs-bare-Tao+WRY diagnostic can isolate framework overhead. Building that separate minimal app is not justified before the smaller Winit-matched control is tested.

## Rejected PoC-C alternatives

### Windows App SDK `InputNonClientPointerSource`
Rejected as a materially different substrate. Microsoft documents `EnteredMoveSize` as entry into the move-size loop and explicitly maps it to `WM_ENTERSIZEMOVE`. It modernizes non-client region definition but retains the same class of native modal movement.

### WebView2 CompositionController
Rejected for this product phase. Composition hosting can position a WebView visual and requires host-side mouse/pointer forwarding, but moving only the visual does not move the real top-level HWND/hit target. Adopting it would replace a large part of WRY/Tauri's current windowed WebView hosting rather than isolate the drag defect.

### Tao/Tauri dependency upgrade only
Rejected as the primary PoC. Tao 0.36 and 0.37 retain the old non-zero synthetic `WM_MOUSEMOVE` lParam. A future Tauri dependency refresh may change other Windows behavior, but it does not currently contain the targeted Winit compatibility fix discovered here.

### `SC_MOVE`, more Send/Post timing variants, or `WM_MOVING`
Rejected. These remain variants of the same native move-size modal path and add no new evidence beyond PoC-B unless paired specifically with the known Winit zero-lParam wake-up.

## Candidate comparison

| Candidate | Avoids failed initiation path? | Avoids native move-size loop? | Change/risk | Audit verdict |
| --- | --- | --- | --- | --- |
| Winit-compatible `WM_MOUSEMOVE(0,0)` wake-up | Yes: tests a missing upstream compatibility step | No | Very small, one controlled message difference | **PoC-C selected** |
| Remove Port Lens `Moved` callback work | No | No | Small | Only after partial PoC-C PASS to isolate residual stutter |
| Minimal Tauri vs bare Tao+WRY harness | Diagnostic only | Depends on test | Medium | Ambiguity resolver, not next gate |
| Windows App SDK non-client regions | Yes at API surface | No; still exposes move-size loop | Medium/high dependency and integration cost | Rejected for C |
| WebView2 composition hosting | Yes | Potentially | Very high; changes WebView hosting/input model | Rejected for current product phase |
| Native `SetCapture` + same-thread `SetWindowPos` | Yes | Yes | Medium; custom drag lifecycle and DPI/clamp work | **Fallback if C fails** |
| JS pointermove → IPC → position | Yes | Yes | High latency/backlog risk | Rejected |

## Selected PoC-C: Winit zero-lParam wake-up control

PoC-C should reuse the isolated native-grip branch because it already proves immediate native receipt of the user's mouse-down and avoids WebView/JS dispatch before the parent drag handoff.

Starting from the B1.2-style queued handoff:
1. hide `compact-hover` once at drag start
2. release capture as required by the native caption handoff
3. snapshot the original cursor screen point
4. queue `WM_NCLBUTTONDOWN / HTCAPTION` with the packed original screen point
5. immediately queue `WM_MOUSEMOVE` with `WPARAM(0), LPARAM(0)` to the parent HWND
6. return from the child WndProc

The second queued message is the compatibility control. Queue order is deliberate: the child posts `WM_NCLBUTTONDOWN`, then immediately posts the zero-lParam `WM_MOUSEMOVE`. When the parent later processes `WM_NCLBUTTONDOWN`, Tao posts its own old non-zero synthetic mouse move. The PoC-C zero-lParam message was already in the thread queue before Tao generated its message, so it is the best non-invasive approximation of Winit 0.30.10's wake-up without patching Tao itself.

A PoC-C PASS would validate the missing modal wake-up, but it would **not** make the current production `data-tauri-drag-region` path safe to restore unchanged. Production still enters Tao `handle_os_dragging()`, whose posted `WM_NCLBUTTONDOWN` encodes the address of a local `POINTS` object rather than packing the signed screen coordinates into the scalar `LPARAM`. The native-grip PoC intentionally bypasses that initiation defect. Therefore production integration after a PASS must either retain a validated native initiation path or carry a separately validated Tao coordinate-packing fix; it must not simply switch the React drag region back on.

Do not change hover recovery, persistence semantics, z-order logic, DPI logic, grip geometry, application `Moved` callbacks, or dependency versions in this control. Do not add a timing delay. The experiment must answer one question: does reproducing Winit 0.30.10's modal wake-up remove the dead period?

## PoC-C implementation evidence

The selected control is now implemented in the isolated worktree `/Users/sunjaekim/Developer/port-lens-poc-c` on branch `poc/compact-winit-wakeup`. Commit `ee2533a54347b2ed883bcbdd573dc1579d901571` (`poc: add winit-style compact drag wakeup`) is based directly on B1.2 `e20e751`.

The implementation changes only `src-tauri/src/native_drag.rs`: after posting the correctly packed original screen-point `WM_NCLBUTTONDOWN/HTCAPTION`, the native grip immediately posts `WM_MOUSEMOVE` with `WPARAM(0), LPARAM(0)`. No delay, dependency, hover recovery, persistence, z-order, DPI, or drag-end lifecycle variable is changed.

Local validation is PASS: frontend build, Rust format, clippy with warnings denied, 36/36 Rust tests, and `git diff --check`. Windows Bundle run `34941498309` completed successfully for the exact commit above; Windows package build, portable preparation, and NSIS/MSI/portable artifact uploads all passed. Artifact IDs are portable `10386245220`, MSI `10385409169`, and NSIS `10385402661`.

Windows manual acceptance **FAILED**. The first cursor movement still did not move the window immediately; pause → jump remained; cursor/window offset became worse than B1.2; catch-up jump also became worse; micro-stutter remained; `Open` after drag stayed normal. Because PoC-C changed only the Winit-style zero-lParam wake-up, this control directly rejects that workaround for the Port Lens Tao-managed caption path on the tested machine.

## PoC-C immediate manual gate

Primary gate:
- slow mouse-down + first movement: window moves immediately
- no pause → snap/jump
- no fixed cursor/window offset after movement starts
- repeated slow and fast drags do not develop catch-up jump
- residual stutter is recorded separately from initial-start latency
- `Open` after drag remains PASS

Interpretation result:
- **FAIL confirmed**: pause/jump remained, while cursor offset and catch-up jump became worse; only `Open` remained normal
- the Winit-style zero-lParam wake-up is therefore rejected for this Port Lens path
- caption/move-loop tuning is permanently closed for this issue: no further `WM_NCLBUTTONDOWN`, `HTCAPTION`, `SC_MOVE`, Send/Post timing, zero-lParam wake-up, or production `data-tauri-drag-region` variants
- advance only to the manual captured-pointer fallback below, after an audit of capture lifecycle, coordinate semantics, and per-move side effects

## Fallback only if PoC-C fails: captured-pointer manual positioning

There is a fundamental Win32 trade-off here. If the real top-level HWND must visibly follow the cursor, then one of two mechanisms must continuously change its position: the system's move-size loop, or application-driven repeated positioning. Therefore these three requirements cannot all be held simultaneously: avoid the Windows move-size modal loop; avoid repeated programmatic position updates; and still have the real HWND follow the mouse continuously. After PoC-A/B, keeping all three would leave no physical drag mechanism.

The previous blanket ban on per-frame `SetWindowPos` should therefore be narrowed, not silently discarded. The original concern was catch-up lag from a high-frequency JS → IPC → backend positioning loop, as seen conceptually in Token Lens, not evidence that a same-thread Win32 position call is inherently unusable. Tauri #15569 is also useful negative evidence: its `SetWindowPos`-only control had constant slight overhead but did not exhibit the progressive degradation caused by `SetWindowRgn`.

A future fallback would use the Port Lens-owned grip only:
- `WM_LBUTTONDOWN`: `SetCapture`, snapshot `GetCursorPos` and parent `GetWindowRect`, preserve the exact grab offset
- `WM_MOUSEMOVE`: sample screen coordinates and call `SetWindowPos(parent, ..., SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE)` only when the target position changes
- `WM_LBUTTONUP` / `WM_CAPTURECHANGED`: clear drag state, release capture as appropriate, persist/clamp once, then restore hover eligibility
- no JS pointer loop, no Tauri command IPC, no `WM_NCLBUTTONDOWN`, no `HTCAPTION`, and no `WM_ENTERSIZEMOVE`

Microsoft documents `SetCapture` specifically for continuing to receive mouse movement while the pointer leaves the original client area. `WM_CAPTURECHANGED` provides the cleanup path if another window takes capture. `SetWindowPos` can move a top-level window while retaining size, Z order, and activation state through explicit flags.

This fallback is materially different from Token Lens' frontend pointermove → command → backend path and from PoC-B's modal caption drag. Tauri #15569 also provides evidence that `SetWindowPos` by itself did not cause that issue's progressive degradation.

However, it remains fallback, not the selected PoC-C, because the newly discovered Winit/Tao divergence is a smaller and more directly symptom-matched control.

## Revised constraints after this audit

Still prohibited:
- WRY/WebView2 child HWND subclassing
- WebView2 `app-region` rework
- arbitrary Send/Post timing delays
- `SC_MOVE` variants with no new upstream evidence
- high-frequency JS → IPC positioning loops
- timer-driven position polling
- integrating any unvalidated drag mechanism into PR #4

Narrowed constraint:
- direct native event-driven `SetWindowPos` is no longer categorically prohibited; it is reserved as fallback only if the Winit compatibility control fails

Preserve throughout:
- Bundle #26 Open deadlock fix
- validated inner-size restore/migration fix
- separate hover window and hide-on-drag policy
- topmost/taskbar overlap behavior
- mixed-DPI/multi-monitor behavior
- saved compact position and startup gating
- PR #3 runtime reattach/lifecycle protections

## Root-cause confidence after deep audit

High confidence:
- the problem is not primarily hover rendering, persistent-state I/O, or synchronous `SendMessageW`
- production and PoC-B both enter the Tao-managed non-client move-size path
- Tao 0.35.3 lacks Winit 0.30.10's explicit zero-lParam workaround for the documented ~500 ms title-bar pause
- Tao 0.36.0 and 0.37.0 still lack that exact workaround

High confidence after the runtime control:
- the Winit zero-lParam synthetic mouse-move argument is **not** the missing fix for Port Lens; the exact control failed and made offset/catch-up behavior worse
- further Tao-managed caption/move-loop timing work has low expected value and is closed for this issue
- the next useful substrate must avoid `WM_NCLBUTTONDOWN` / `HTCAPTION` and the native move-size modal loop entirely

Still unknown:
- whether WebView2-backed top-level HWND movement remains smooth when driven directly by same-thread native `SetWindowPos`
- how much Port Lens' `Moved` callback and WRY's `NotifyParentWindowPositionChanged()` contribute during direct programmatic movement
- whether the product's always-on-top/taskbar-overlap policy needs drag-time suppression or special z-order flags
- the exact mixed-DPI behavior when cursor and parent HWND cross monitors during a captured-pointer drag

A separate minimal Tauri-vs-bare-Tao+WRY harness is no longer the immediate next gate because PoC-C was not mixed or ambiguous: it failed clearly. The next step is instead an audit of the materially different captured-pointer/manual-positioning substrate.

## External references

- Tauri #10767 — Windows draggable-region focus/capture anomalies and lost mouse events: https://github.com/tauri-apps/tauri/issues/10767
- Winit 0.30.10 release/changelog — fixed ~500 ms pause when clicking the title bar during continuous redraw: https://github.com/rust-windowing/winit/releases/tag/v0.30.10
- Winit Windows event-loop source — zero-lParam dummy `WM_MOUSEMOVE` rationale: https://docs.rs/winit/latest/src/winit/platform_impl/windows/event_loop.rs.html
- Mixxx Windows native event filter — independent zero-lParam workaround for the same 500 ms wait: https://sources.debian.org/src/mixxx/2.5.0%2Bdfsg-3/src/nativeeventhandlerwin.cpp
- Tauri current ecosystem releases — stable core 2.11.5 / Tao 0.36.0 at audit time: https://tauri.app/release/
- Tauri #15569 — undecorated Windows resize degradation; `SetWindowPos`-only control did not progressively degrade: https://github.com/tauri-apps/tauri/issues/15569
- Microsoft `WM_NCLBUTTONDOWN` — screen-coordinate lParam: https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-nclbuttondown
- Microsoft `WM_MOUSEMOVE` — client-coordinate lParam: https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-mousemove
- Microsoft mouse capture / `WM_CAPTURECHANGED`: https://learn.microsoft.com/en-us/windows/win32/inputdev/about-mouse-input
- Microsoft `SetWindowPos`: https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowpos
- Windows App SDK `InputNonClientPointerSource.EnteredMoveSize`: https://learn.microsoft.com/en-us/windows/windows-app-sdk/api/winrt/microsoft.ui.input.inputnonclientpointersource.enteredmovesize
- WebView2 `CoreWebView2CompositionController` documentation

## Next action

PoC-C is closed as **FAILED**. Preserve `poc/compact-winit-wakeup` and its Bundle `34941498309` as evidence; do not integrate its code into PR #4.

Before another implementation, audit the captured-pointer/manual-positioning fallback in depth. The audit must settle: `SetCapture`/`ReleaseCapture`/`WM_CAPTURECHANGED` ordering; screen-vs-client coordinate math; exact grab-offset preservation; mixed-DPI and cross-monitor behavior; `SetWindowPos` flags and topmost/taskbar interaction; WRY `NotifyParentWindowPositionChanged()` cost; Port Lens `Moved` callback suppression; one-shot persistence/clamp at drag end; and failure cleanup if capture is stolen. Only after that audit should a new isolated PoC-D be created.
