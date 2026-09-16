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

## Windows manual result — D0 (`f474b0d`, Bundle `34946164626`)

User validation on Windows:
- first movement: slightly delayed, but acceptable and similar to Token Lens
- pause → jump: **FAIL / still present**
- persistent cursor/window offset: **PASS / absent**
- catch-up jump: **FAIL / still present**
- micro-stutter: present but acceptable and similar to Token Lens
- hover-visible drag → hide: **PASS**
- `Open`: **PASS**

Interpretation: D0 substantially improves the old offset behavior and preserves hover/Open, but it does not pass the primary drag gate because jump/catch-up remains. For subsequent validation, `pause → jump` means the initial post-threshold stall followed by a large first move; `catch-up jump` means the same lag/catch-up pattern occurring during an already-active drag. They may share one queueing/movement cause and can be treated as one jump-class symptom unless they diverge under A/B testing.
## D0.1 isolation decision

A direct comparison against current Token Lens found that both applications use the same desktop stack: Tauri 2.11.5, tauri-runtime-wry 2.11.4, Tao 0.35.3, and WRY 0.55.1. Token Lens also receives `WindowEvent::Moved` and schedules persistence during drag, so Port Lens `schedule_persist()` is not a strong first differentiator.

The material movement primitive differs:
- Port Lens D0 and historical `cca33ce`: Windows raw position-only `SetWindowPos`
- current Token Lens: Tauri `WebviewWindow::set_position()`

Select **D0.1** as the next single-variable control. Keep pointer capture, 4 px threshold, grab-ratio-only invoke, backend current-cursor resampling, separate hover HWND, and all lifecycle logic unchanged; replace only the Windows drag-time movement call with `window.set_position(target)`. This does not use caption hit-testing or the Windows move/size modal loop.

## D0.1 build evidence

Implementation SHA: `850d728bda02e975106db0c51d6913274b7a06af` (`poc: route compact drag through tauri position`).

Local gates: frontend build PASS, rustfmt PASS, clippy `-D warnings` PASS, Rust tests 36/36 PASS, diff-check PASS.

Windows Bundle `35037312466`: SUCCESS. Artifacts: portable `10423698454`, MSI `10423942712`, NSIS `10423539252`. Windows runtime/manual validation remains pending; this bundle must be judged primarily on whether the jump/catch-up class disappears relative to D0 `f474b0d` / Bundle `34946164626`.
## D0.1 Windows manual result

Windows manual validation of `850d728` reported:
- first movement: slightly delayed but acceptable / Token-Lens-like
- jump/catch-up: still present and worse than D0
- cursor offset: absent
- micro-stutter: present but acceptable / Token-Lens-like
- hover-visible drag hide: PASS
- Open: PASS

Conclusion: Tauri `window.set_position()` does not explain Token Lens' smoother result. On Port Lens it made the jump/catch-up symptom worse than raw position-only `SetWindowPos`. D0.1 is CLOSED / FAILED.## D0.2 compact-workload isolation control

A same-stack comparison found another material difference: Port Lens continues 3 s monitored/runtime refresh and 10 s inventory refresh in compact mode, while Token Lens refresh cadence is much lower and uses a simpler vanilla-DOM renderer.

D0.2 therefore returns to the better D0 movement primitive (raw Windows position-only `SetWindowPos`) and changes only compact background workload:
- suspend the 3 s monitored/runtime poll while compact
- suspend the 10 s inventory poll while compact
- ignore late results from already in-flight refreshes when compact
- keep pointer capture, 4 px threshold, grab-ratio-only invoke, backend current-cursor resampling, separate-hover behavior, and final position math unchanged

This is a diagnostic control, not a proposed production policy. If it removes jump/catch-up, subsequent work must restore live compact monitoring with drag-safe/deferred updates instead of permanently freezing compact data. If it fails, renderer/background polling is not sufficient to explain the defect and D1 native captured-pointer positioning becomes the next substrate.## D0.2 build evidence

Implementation commit: `b7f936f231e50cb7995b0f992c8a1e762971a94a` (`poc: isolate compact drag workload`).

Local gates: frontend build PASS, fmt PASS, clippy `-D warnings` PASS, Rust tests 36/36 PASS, diff check PASS.

Windows Bundle `35040462376`: SUCCESS. Artifacts: portable `10425132792`, MSI `10424689681`, NSIS `10425605338`.

Windows runtime/manual validation remains pending. PR #4 remains separate, OPEN, CLEAN, and unmerged.
## D0.2 Windows manual result

Windows manual validation of `b7f936f` / Bundle `35040462376` reported that the jump/catch-up symptom disappeared completely. This is the first PoC-D control to eliminate the primary drag blocker. The result strongly implicates compact-mode background refresh / renderer update contention rather than the pointer-capture/manual-position substrate itself.

D0.2 is therefore a diagnostic PASS, not the production policy: permanently disabling compact polling would defeat Port Lens' continuous-monitoring purpose.

## D0.3 production-shape control

D0.3 restores normal 3 s monitored/runtime polling and 10 s inventory polling whenever compact drag is idle. Polling is suspended only after the 4 px threshold marks a real drag as active. Poll ticks during drag do not start backend refresh work, and refreshes that started before drag may finish but their results/errors are not applied to React state while the drag gate is active.
On `pointerup`, `pointercancel`, or `lostpointercapture`, the final compact move is allowed to finish first. The drag gate is then released, any pre-drag in-flight refresh promises are allowed to settle, and Port Lens immediately performs one catch-up refresh before continuing the normal polling cadence.

This keeps compact monitoring live outside the actual drag interval while preserving the workload isolation proven by D0.2. No React state is used to represent drag-active state; refs are used so entering/leaving drag does not itself trigger a render.

D0.3 acceptance gate:
- normal compact counts continue to refresh while idle
- during continuous 10–20 s drag, no jump/catch-up returns even as normal polling deadlines pass
- after release, values catch up without requiring Open/restart
- cursor offset remains absent; hover-hide and Open remain PASS

## D0.3 build evidence

Implementation commit: `01cfe4d33889385dc761d08ed466ceeb0d1a7185` (`poc: suspend compact polling only during drag`).

Local gates: frontend build PASS, fmt PASS, clippy `-D warnings` PASS, Rust tests 36/36 PASS, diff check PASS.

Windows Bundle `35044750834`: SUCCESS. Artifacts: portable `10427100345`, MSI `10426059827`, NSIS `10426790993`.

Windows runtime/manual validation remains pending. Primary gate: compact idle polling must continue to refresh; during an actual drag (>4 px threshold), polling/result application must suspend and jump/catch-up must remain absent; after release/cancel/lost capture, polling must resume and an immediate catch-up refresh must restore current state.

## D0.3 Windows manual result

Windows validation passed the production-shaped drag gate: idle compact polling updates normally; first movement and micro-stutter are acceptable; jump/catch-up and cursor offset are absent; polling resumes after drag; hover-hide and Open pass. One residual cold-start issue remains: after a full process restart, the first compact drag shows one hitch/jump, while all later drags in that process are clean.

## D0.4 cold-start race control

Leading hypothesis: D0.3 raises `compactDragActive` only after the 4 px drag threshold. Initial startup inventory/managed refreshes may still be in flight, and a first-process refresh result can land between `pointerdown` and the first post-threshold movement, causing a React update exactly as the first drag begins. Later drags do not reproduce because those startup refreshes have already settled.

D0.4 changes one variable only: raise the polling/result gate immediately on non-button compact-bar `pointerdown`; keep the 4 px movement threshold unchanged; release the gate on pointerup/cancel/lost capture, with the same catch-up refresh. This preserves normal idle compact monitoring and only broadens the protected interval from active drag to the potential-drag gesture. Runtime validation must determine whether the first-process hitch disappears before this hypothesis is treated as confirmed.

## D0.4 build evidence

Implementation commit: `20cf14f64bc37aeede8397750b097b3159c368c3` (`poc: guard compact polling from pointerdown`).

Local gates: frontend build PASS, rustfmt PASS, clippy `-D warnings` PASS, Rust tests 36/36 PASS, diff check PASS.

Windows Bundle `35047586123`: SUCCESS. Artifacts: portable `10427479481`, MSI `10428036175`, NSIS `10427812688`.

Runtime validation target is intentionally narrow: full app exit → relaunch → compact → very first drag. If the cold-start hitch disappears while idle polling and later drag behavior remain unchanged, the startup-refresh result race is confirmed strongly enough for production integration. If the hitch remains, revert D0.4 and instrument first-move timing instead of adding further speculative fixes.

## D0.4 Windows manual result

D0.4 did not remove the residual hitch. The user observed the first-movement hitch/jump randomly on the first, second, third, fourth, or later drag. This rejects the startup-only refresh race hypothesis. The symptom instead tracks an occasional overlap between polling-driven React work and drag initiation.

D0.4 pointerdown gating therefore has no production value and is removed from the next control.

## D0.5 unchanged-snapshot render suppression

D0.5 returns to the accepted D0.3 drag lifecycle: idle compact polling remains active and the drag gate starts only after the 4 px threshold. The only new variable is render suppression for unchanged polling snapshots.

Current polling always receives fresh arrays and previously called the React state setters even when listener/app/runtime/exit content was identical. D0.5 fingerprints each returned snapshot and does not call the corresponding setter when the content is unchanged. Direct user mutations remain synchronized back into the fingerprints through effects.

Goal: preserve continuous monitoring while eliminating avoidable 3 s / 10 s App rerenders that can collide with drag initiation. If intermittent hitch/jump remains after this control, accept D0.3/D0.5 behavior as a residual renderer-contention limitation rather than adding the substantially more complex native D1 capture state machine solely for this low-frequency symptom.

## D0.5 build evidence

Implementation commit: `33aba818f44ca4fc69eb7865f9bd6262b6b9620b` (`poc: skip unchanged compact polling renders`).

Local gates: frontend build PASS, rustfmt PASS, clippy `-D warnings` PASS, Rust tests 36/36 PASS, diff check PASS.

Windows Bundle `35050303880`: SUCCESS. Artifacts: portable `10428667180`, MSI `10428796672`, NSIS `10428457883`.

Runtime validation should focus on several minutes of normal compact polling with drag starts at arbitrary times. If hitch/jump disappears or becomes materially rarer while idle polling and actual state changes still update correctly, D0.5 is the production candidate. If the intermittent hitch remains at roughly the same rate, close the drag issue at the D0.3/D0.5 quality level rather than escalating to D1 solely for this residual renderer-contention symptom.


## D0.5 Windows manual result — accepted residual

Final Windows validation reported an intermittent first-movement hitch/jump on about 3 of 10 drag starts. The occurrence was random relative to drag number, consistent with occasional overlap between polling/render work and drag initiation rather than a one-time initialization defect.

Accepted behavior: first movement is otherwise usable; cursor offset is absent; sustained jump/catch-up is absent once the drag is active; micro-stutter is acceptable; idle polling continues; polling resumes after drag; hover-visible drag hides correctly; Open remains correct. Because unchanged-snapshot suppression did not eliminate the residual, further React-side timing patches are closed.

Decision: **CLOSED / ACCEPTED RESIDUAL**. D0.5 is the production candidate. D1 native `SetCapture` / `WM_MOUSEMOVE` positioning remains technically available but is not justified for this residual symptom because it adds a separate Windows capture lifecycle, capture-loss handling, and broader regression surface. Revisit only if the accepted hitch materially worsens in production.
