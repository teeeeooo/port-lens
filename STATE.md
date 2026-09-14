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

PR #4 local validation after the compact-window architecture rework and follow-up fixes:
- frontend production build: PASS
- Rust tests: 33/33 PASS
- native clippy with warnings denied: PASS
- cargo fmt / diff check: PASS
- Windows-target cross-clippy remains unavailable locally because the macOS toolchain lacks `llvm-rc`; Windows CI is the authoritative compile gate

Latest Windows manual evidence before the follow-up fixes:
- hover open/close flicker: PASS after splitting the hover panel into its own window
- native drag: major improvement, but a very rare start/catch-up jump remained
- compact bottom corners, startup state race, and initial main-window height still needed correction

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

PR #4 now uses two persistent desktop windows in compact mode. The main window becomes a fixed-size 276×46 compact bar; a separate hidden `compact-hover` WebViewWindow renders the registered-App list with green/gray status dots, capped at 8 visible rows with internal scrolling. Hover no longer resizes the main compact HWND.

The hover window is created once at app startup, owned by the main window, non-focusable, transparent, taskbar-hidden, and reused with show/hide. Main sends already-computed App status data to it, so no duplicate listener/process scan is introduced. A revision handshake waits until the hidden hover WebView has committed the requested list before showing the native panel.

Compact dragging now uses Tauri's built-in drag-region path directly from mouse-down. The previous 4 px threshold, pointer capture, custom `start_compact_drag` command, and per-drag IPC handoff have been removed. The main window has an explicit least-privilege `core:window:allow-start-dragging` capability; `core:default` does not include this command. During native movement the taskbar topmost keeper is suspended; after movement settles, debounced `WindowEvent::Moved` persistence re-clamps/saves the position and restores the z-order correction.

The compact bar again clips its own WebView surface with the validated rounded `clip-path`, including the lower corners. Main-window startup now targets 1020×680 and additionally clamps restored/default bounds to the active monitor work area so saved 760 px-era bounds cannot reopen below the taskbar.

Frontend startup is gated by a builder-managed `StartupGate`. `App` does not mount or call state-dependent commands until backend setup has managed `AppState`, settings, bubble/window controllers, and completed tray setup, eliminating the transient `state not managed` startup race.

## Validation gate

Do not merge PR #4 until the latest Windows Portable is manually checked for:
- hover panel appears/disappears without compact-bar flicker
- hover panel contains current App names/statuses and remains scrollable above 8 Apps
- fast/repeated native drag tracks the pointer continuously without lag, drop, or catch-up jump
- starting drag while the hover panel is visible hides the panel cleanly and moves only the compact bar
- taskbar overlap/topmost behavior and multi-monitor/mixed-DPI movement remain correct
- saved compact position restores after restart
