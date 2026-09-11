# Port Lens Backlog

Last updated: 2026-09-11

Items are ordered by intended implementation sequence after the current stability merge.

## 1. Compact position polish

Status: implemented on `feature/compact-position-polish`; awaiting native Windows package/manual verification before merge.

Goal: allow the Windows compact bubble to sit directly above the taskbar without an artificial gap.

Implemented behavior:
- Port Lens still clamps compact position to monitor `work_area()`.
- Windows uses a 0 px compact edge margin so the bubble can touch the work-area boundary without covering the taskbar.
- Non-Windows desktop behavior retains the existing 12 px edge margin.

Preferred Windows policy:
- keep the monitor work area as the safe boundary
- reduce the compact edge margin to 0 px
- do not allow the bubble to cover the taskbar

Acceptance criteria:
- bubble can touch the top edge of the taskbar/work-area boundary
- no taskbar overlap
- left/right/top clamping remains valid
- multi-monitor movement remains correct
- mixed-DPI movement remains correct
- saved compact position restores and clamps correctly after restart

Suggested branch: `feature/compact-position-polish`

## 2. Verified managed runtime reattach

Goal: restore safe Stop / Restart control for a server that was started by Port Lens, survived Port Lens exit, and is rediscovered after Port Lens restarts.

Current behavior:
- persisted managed identity prevents a surviving server from being misclassified as a different process
- monitoring and Open work after Port Lens restarts
- Stop / Restart remain disabled because the original `Child` handle and session-local root ownership are gone

Required design constraints:
- identify the current listener PID for the configured Port
- verify the persisted managed process identity
- inspect the Windows parent-process chain and locate the expected Port Lens launch root where possible
- compare process/command evidence before granting ownership
- defend against PID reuse and unrelated processes taking the same Port
- never enable destructive lifecycle actions on ambiguous identity

Acceptance criteria:
- verified surviving Port Lens-started Apps become reattached runtimes after restart
- Stop terminates only the verified managed process tree
- Restart performs verified Stop followed by the configured Start Command
- mismatched or ambiguous processes remain Online/Changed without Stop / Restart authority
- external listeners are never silently adopted
- tests cover PID reuse/mismatch and successful reattach cases

Suggested branch: `feature/runtime-reattach`

## Deferred housekeeping

After the two items above stabilize, review version bump/release notes and decide whether the next packaged release remains 0.3.x or advances based on accumulated feature scope.
