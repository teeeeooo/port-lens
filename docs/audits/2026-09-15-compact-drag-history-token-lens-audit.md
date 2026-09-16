# Compact Drag History / Token Lens Audit — 2026-09-15

## Question

Why did Port Lens spend many revisions on Windows native drag instead of using the Token Lens floating-bubble drag pattern?

## Executive finding

Port Lens did not simply overlook the Token Lens pattern. A functionally equivalent pointer-capture/manual-positioning path existed in Port Lens from `78c006c` through `cca33ce` on 2026-09-14.

The important historical gap is that repository evidence does **not** show a Windows manual failure for the final `cca33ce` form before it was removed. CI and Windows Bundle `34802709558` passed, while `STATE.md` / `BACKLOG.md` still described Windows manual validation as pending.

Commit `7144042` then combined two architecture changes: it split hover into a dedicated `compact-hover` WebviewWindow and simultaneously removed pointer-capture / repeated move IPC / backend cursor sampling / manual position updates in favor of one-shot native `start_dragging()`.

The recorded rationale was architectural risk reduction — avoiding repeated pointermove IPC, cursor polling, and manual per-frame positioning — not a documented manual FAIL of the `cca33ce` drag path.

A later 2026-09-15 audit explicitly inspected Token Lens v2 and rejected it without a Port Lens control because per-pointermove IPC *could* create responsiveness/catch-up problems. That concern was plausible but remained predictive rather than experimentally demonstrated for the fixed-window + separate-hover architecture.
## Chronology

| Commit | Drag architecture | Evidence / transition |
| --- | --- | --- |
| `bed908d` | grip `pointerdown` → Tauri `startDragging()` | initial compact implementation |
| `4f7b667` | native Tauri drag retained while same-HWND hover list was added | hover feature introduced |
| `78c006c` | pointer capture + 4 px threshold + repeated `move_compact_bubble` invoke; backend samples current cursor and repositions window | first Token-Lens-like Port Lens path |
| `a3995d2` | same pointer/manual drag | hover rendering refinements |
| `cca33ce` | same pointer/manual drag, Windows movement reduced to position-only `SetWindowPos`; per-move z-order refresh removed | strongest historical manual-positioning form; Windows Bundle `34802709558` PASS, manual gate still documented as pending |
| `7144042` | dedicated hover WebviewWindow + one-shot native `start_dragging()` after 4 px threshold | pointermove IPC/manual movement removed by design |
| `b24c8a6` | direct deep drag region from mouse-down | threshold/capture/custom command removed; prior native path reported major improvement with rare start/catch-up jump |
| `5d8d09e` → `465866c` | increasingly native Windows initiation and caption-drag controls | attempted to remove JS/IPC drag-start latency and drag-time interference |
| `f1e68ec` / `43d4c15` | reverted failed child-HWND interception to deep drag region and removed surrounding contention | Bundle #24/#25 regressions; Bundle #26 still late-capture/fixed-offset FAIL |
| PoC-A/B/C | WebView2 non-client regions, native grip caption drag, Winit wake-up control | all Windows manual FAIL |

## Historical implementation equivalence

The `78c006c` / `cca33ce` frontend already used the core Token Lens interaction model: `setPointerCapture()`, a 4 px threshold, a stored grab offset ratio, and one backend move request per pointer move.

The backend did **not** receive historical screen coordinates. It sampled `window.cursor_position()` when each command executed, selected the current monitor, recomputed physical geometry for that monitor/DPI, preserved the stored grab ratio, and positioned the window from the newly sampled cursor.

That is the same important anti-stale-coordinate property used by current Token Lens. Therefore the later shorthand “JS pointermove → IPC → position can catch up” omitted a material detail: queued invocations do not necessarily replay old pointer coordinates in sequence because each backend execution resamples the current cursor.
## Why the historical Port Lens path was still not identical to current Token Lens

Several differences could have affected smoothness and should not be erased by the equivalence above.

1. `cca33ce` still used a **same-HWND hover expansion**. The compact HWND changed height when the App list opened, and drag logic had to account for expanded geometry, hover origin, and post-drag collapse. Current Port Lens now avoids that entire geometry coupling with a separate fixed hover window.
2. `cca33ce` had a periodic Windows taskbar z-order keeper that could still run during drag. Current Token Lens marks `last_drag_move` and suppresses that keeper for 500 ms after movement. Port Lens gained analogous drag-time suppression only after it had already abandoned manual pointer movement.
3. Earlier `78c006c` did more work per move: min/max size, size, position, and z-order correction. `cca33ce` improved Windows movement to position-only `SetWindowPos`, but the history often discusses the whole manual-drag family as if all revisions had the same cost.
4. Port Lens has a wider 276×46 compact bar and a dedicated `compact-hover` WebViewWindow. Token Lens has a simpler collapsed-bubble lifecycle. These differences justify a fresh control, but they do not prove the manual substrate is unsuitable.

## Why it was removed

The strongest repository evidence is `7144042`. Its documentation explicitly presents removal of repeated pointermove IPC, cursor polling, and manual per-frame `SetWindowPos` as an architectural improvement while introducing native `start_dragging()` and the separate hover window.

However, the preceding `cca33ce` state still says Windows manual validation is pending. No commit, audit note, PR comment, or recorded Windows result found in this audit identifies `cca33ce` as a manual drag FAIL.

Therefore the defensible conclusion is:
- the Token-Lens-like path was consciously removed,
- but the recorded reason was **expected latency/backlog risk plus preference for OS-owned native movement**, not a preserved empirical failure of the final optimized path.

This distinction became important later because the native path itself generated the persistent pause → jump / cursor-offset defect that dominated subsequent work.
## Later audit decision that reinforced the exclusion

The 2026-09-15 compact-drag audit explicitly inspected local Token Lens v2 and correctly documented its design: pointer capture, 4 px threshold, repeated `move_floating_bubble`, backend `cursor_position()`, and `set_position()`.

It then rejected Token Lens as the preferred Port Lens answer because per-pointermove IPC *can* create responsiveness/catch-up failure. The later PoC-C audit strengthened this into a general prohibition on high-frequency JS → IPC positioning loops and described Token Lens as a conceptual example of that risk.

That conclusion was too broad relative to the available evidence. It treated a plausible queueing risk as if it were already demonstrated in the current fixed-window Port Lens architecture, and it underweighted the current-cursor resampling property that limits stale-coordinate replay.

Token Lens' core pointer/manual drag implementation dates to 2026-09-05, before Port Lens' 2026-09-14 switch to native drag. Token Lens later added drag-aware taskbar z-order suppression in `4bf1e42` on 2026-09-14 17:17, after Port Lens had already switched at 14:56. By the 2026-09-15 reference audit, that improved Token Lens implementation was available locally.

## Revised interpretation

There is no basis to declare the Token Lens substrate proven good for Port Lens yet. There is also no adequate basis to keep it categorically excluded.

The missing experiment is specific: fixed 276×46 Port Lens compact HWND + current separate `compact-hover` architecture + Token Lens pointer capture/current-cursor resampling + drag-time z-order/persistence suppression, with no native caption/move-size loop.

This combination has never been Windows-manually validated in Port Lens. Historical `cca33ce` tested an older same-HWND hover architecture, while all later A/B/C work tested native/non-client substrates.

A future PoC-D audit should therefore compare two materially different non-caption candidates instead of automatically jumping to raw Win32 capture:
- D0: Token Lens exact-style pointer capture → repeated lightweight invoke → backend current-cursor sample → position-only movement
- D1: Port Lens-owned native `SetCapture` → same-thread `WM_MOUSEMOVE` → position-only `SetWindowPos`

Audit first; do not integrate either into PR #4 without a Windows manual control.
