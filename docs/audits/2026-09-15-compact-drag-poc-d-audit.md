# Compact Drag PoC-D Audit — 2026-09-15

## Scope

This audit compares two non-caption drag substrates after PoC-A/B/C failed Windows manual validation. It is based on current Port Lens `463f5da` and current local Token Lens `main`.

Candidates:
- **D0:** Token Lens-style DOM pointer capture → repeated lightweight invoke → backend current-cursor resampling → position-only movement.
- **D1:** Port Lens-owned native grip → Win32 `SetCapture` → same-thread `WM_MOUSEMOVE` → position-only `SetWindowPos`.

Both intentionally avoid `data-tauri-drag-region`, Tauri/Tao `start_dragging()`, `WM_NCLBUTTONDOWN`, `HTCAPTION`, `SC_MOVE`, and the Windows move-size modal loop.

## Current Port Lens prerequisites

The current code already provides most D0 safety properties:
- compact `main` is a fixed 276×46 window; hover is a separate `compact-hover` WebviewWindow
- Windows `set_compact_position()` is position-only `SetWindowPos(... SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE)`
- taskbar z-order maintenance skips while physical left-button input is down
- compact position persistence waits while physical left-button input is down
- position persistence is already coalesced after `WindowEvent::Moved`
- `Open` is an explicit button and can be excluded from pointer drag initiation
## D0 audit

### IPC backlog

D0 does not send historical cursor coordinates. Each move invoke carries only the stable grab ratio. The Rust command reads `window.cursor_position()` when that invocation executes, so delayed invocations converge on a recent cursor position instead of replaying old `screenX/screenY` samples.

Outstanding invocations can still add scheduling overhead, but this is a measurable performance risk rather than a correctness blocker. For the PoC, the move command should return no UI payload and should not update React state on every move.

### Pointer capture

DOM `setPointerCapture()` keeps pointermove/up delivery after the cursor leaves the compact WebView. This is already proven in Token Lens and was used in historical Port Lens `78c006c` → `cca33ce`.

The PoC must explicitly release capture on pointerup/cancel and avoid any native-caption handoff.

### Hover interaction

The separate hover HWND must hide at the first real drag movement, not on pointerdown and not on every move. The first D0 move command will optionally hide `compact-hover` inside the same backend transaction before repositioning. Normal hover-closed drag therefore has no extra hide work.

Frontend hover generation/suppression state must be invalidated at drag start so an in-flight hover-open transaction cannot remain visible after movement begins.
### Mixed DPI / monitor crossing

The move command resolves the monitor from the backend-sampled cursor, computes the compact physical size using that monitor's scale factor, preserves the original grab ratio against that target size, then clamps the target using the existing Windows full-monitor policy.

This matches the historical Port Lens/Token Lens geometry model. Actual DPI resize remains owned by the existing Tauri/Windows window pipeline; D0 does not add per-move size changes. Mixed-DPI behavior remains a required Windows manual gate.

### `WindowEvent::Moved` callback cost

Each programmatic move still generates Port Lens/Tauri `Moved` callback traffic. The current callback only schedules/coalesces persistence; while LButton is down the worker does not perform compact persistence, hover hide, or z-order correction.

This leaves a small amount of per-move callback churn, but not disk I/O or repeated z-order `SetWindowPos`. It is not a blocker for D0. If D0 fixes start latency but leaves residual micro-stutter, suppressing the application `Moved` scheduling path during D0 drag is the next isolation control.

### Drag finish / persistence

The final pointerup move samples the current backend cursor once more. Existing coalesced `Moved` persistence can save the final compact position after LButton release, so the PoC does not add a synchronous settings write to every pointer event.

## D1 audit

D1 removes JS/Tauri command scheduling from the move path and gives the strongest timing control. However it requires a new native capture state machine, capture-loss cleanup, coordinate conversion, direct parent HWND movement, and explicit lifecycle ownership around `WM_CAPTURECHANGED`.

That is materially more code and Windows-specific risk than D0. D1 remains a valid fallback if D0 fails the Windows manual gate, but there is no evidence-based reason to pay that complexity first.
## Decision

**No architectural blocker found for D0. Select D0 as PoC-D1.**

Implementation boundary:
- remove the compact bar's `data-tauri-drag-region`
- add DOM pointer capture with the existing 4 px threshold
- preserve `Open` as non-drag interactive UI
- send only grab ratios plus a one-shot `hideHover` flag; never send cursor coordinates
- backend samples current cursor, resolves monitor/DPI, computes/clamps target, and calls existing position-only movement
- no per-move React state update, persistence write, z-order correction, size change, or hover reposition
- pointerup performs one final current-cursor move; existing moved-event coalescing persists after release

Windows manual primary gate:
1. slow grip/bar drag begins with the first post-threshold movement
2. no pause → jump
3. no persistent cursor/window offset
4. repeated slow/fast drag has no catch-up jump
5. record residual micro-stutter separately
6. drag after visible hover hides the hover panel rather than leaving it behind
7. `Open` remains immediate and does not initiate drag

Secondary gate after primary PASS: taskbar overlap/topmost, restart position restore, and mixed-DPI/multi-monitor crossing.

Do not integrate PoC-D into PR #4 until Windows manual validation passes.

## D0 implementation status

D0 is now implemented on the isolated PoC-D branch. The compact bar no longer declares a Tauri drag region. React owns pointer capture and the 4 px click-vs-drag threshold, while each move invoke carries only the original grab ratios; cursor coordinates are never sent from the frontend.

Rust `move_compact_bubble` validates compact state, optionally hides `compact-hover` on the first real movement, samples the current cursor, resolves the current monitor, derives the compact physical size from that monitor's scale factor, preserves the grab ratio, clamps the target through the existing policy, and calls the existing position-only move helper.

Static inspection confirms the PoC-D drag implementation contains no `data-tauri-drag-region`, `start_dragging`, `WM_NCLBUTTONDOWN`, `HTCAPTION`, or `SC_MOVE` path. The old main-window start-dragging capability file remains present but unused so the PoC does not mix an unrelated permission cleanup into the drag control.

Local validation after implementation:
- `npm ci`: PASS, 0 vulnerabilities
- `npm run build`: PASS
- `cargo fmt --check`: PASS
- `cargo clippy --all-targets --all-features -- -D warnings`: PASS
- `cargo test`: PASS, 36/36
- `git diff --check`: PASS

Windows package/runtime validation remains pending. A package PASS will prove compilation only; the drag substrate remains unproven until the manual gate above passes.
