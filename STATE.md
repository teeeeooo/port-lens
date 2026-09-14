# Port Lens State

Last updated: 2026-09-14

## Current baseline

Port Lens v0.3.0 preview is a Tauri/Rust + React/TypeScript desktop app for discovering TCP listeners and managing selected local services.

Current merged baseline: `bee52e3` (PR #3, verified runtime reattach + lifecycle transition hardening).

Active branch: `feature/compact-app-hover`; PR #4 targets `main` and remains unmerged pending final Windows compact-mode validation.

PR #3 manual Windows validation passed:
- surviving Port Lens-started App reattaches after Port Lens restart
- verified reattached Stop works
- Stop no longer exposes the provisional lifecycle-verification message

PR #4 local validation after the latest compact interaction changes:
- frontend production build: PASS
- Rust tests: 32/32 PASS
- native clippy with warnings denied: PASS
- Windows-target cross-clippy with warnings denied: PASS

## Product model

`Listening Ports` is discovery-only. Registering a Port creates a persistent `App` immediately.

An App may be monitoring-only, or may gain Start / Stop / Restart after both Working Directory and Start Command are configured.

Port Lens keeps lightweight inventory scanning separate from targeted monitoring of registered Ports. OS scans run off the Tauri command thread and use timeout protection.

## Managed process behavior

Windows Start Commands are executed through hidden `cmd.exe` with stdout/stderr redirected to per-App logs. Quoted command content is passed using raw Windows command-line handling so paths such as `-File "C:\path with spaces\script.ps1"` survive intact.

Port Lens records managed root PID, exit code, elapsed runtime, expected/unexpected exit state, and early exit diagnostics. A process that exits unexpectedly within 10 seconds is surfaced as an early exit.

Windows can recover lifecycle authority after Port Lens restarts only after verifying the persisted listener generation and managed root process identity. Reattached Stop re-verifies root identity immediately before termination. Stop / Restart suppress reattach while destructive lifecycle work is in progress, and managed refresh ordering prevents provisional ownership state from leaking into the UI.

## Compact mode

Compact mode is enabled by default. Minimize enters the floating compact monitor; disabling Compact mode makes Minimize hide to tray. Close exits the application.

On Windows, user drag may use the full monitor bounds, including the taskbar-reserved area. A topmost keeper protects intentional taskbar overlap. Non-Windows desktop builds retain the 12 px edge margin policy.

PR #4 adds a registered-App hover list with green/gray status dots, capped at 8 visible rows with internal scrolling. The compact bar and App list are separate translucent surfaces so native hover resizing does not repaint one monolithic glass surface.

The latest drag path keeps the current native window size fixed for the entire pointer gesture. If hover is already expanded, the expanded geometry moves as one unit and collapses only after pointer release. Drag motion is position-only; taskbar z-order correction is no longer repeated for every pointer move. Hover close hides the list before shrinking the native window.

## Validation gate

Do not merge PR #4 until the latest Windows Portable is manually checked for:
- hover open/close flicker
- drag continuity with hover closed and hover open
- no position jump when an expanded hover drag collapses after release
- taskbar overlap/topmost behavior
- saved compact position after restart
