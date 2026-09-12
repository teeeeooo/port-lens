# Port Lens State

Last updated: 2026-09-12

## Current baseline

Port Lens v0.3.0 preview is a Tauri/Rust + React/TypeScript desktop app for discovering TCP listeners and managing selected local services.

Current merged baseline: `249d111` (PR #2, compact-position polish).

Runtime-reattach parent branch: `feature/runtime-reattach` at `e46c538`; PR #3 remains open pending manual Windows restart → Stop/Restart verification.

Stacked validation branch: `feature/compact-app-hover` at `767227f`; PR #4 targets PR #3 so one combined Windows Portable can validate both features before either merge.

Validated parent-branch evidence:
- macOS: PASS (`34597065813`)
- Windows: PASS (`34597065813`)
- Windows Bundle: PASS (`34597087024`)

Manual Windows validation completed for:
- responsive listener discovery and tray interaction
- App registration and Offline persistence
- App Edit Save persistence
- quoted PowerShell `.ps1` launch commands
- direct CMD/Node launch commands
- stdout/stderr capture and process-exit diagnostics
- Port Lens restart followed by recognition of a previously Port Lens-started server

## Product model

`Listening Ports` is discovery-only. Registering a Port creates a persistent `App` immediately.

An App may be monitoring-only, or may gain Start / Stop / Restart after both Working Directory and Start Command are configured.

Port Lens keeps lightweight inventory scanning separate from targeted monitoring of registered Ports. OS scans run off the Tauri command thread and use timeout protection.

## Managed process behavior

Windows Start Commands are executed through hidden `cmd.exe` with stdout/stderr redirected to per-App logs. Quoted command content is passed using raw Windows command-line handling so paths such as `-File "C:\path with spaces\script.ps1"` survive intact.

Port Lens records managed root PID, exit code, elapsed runtime, expected/unexpected exit state, and early exit diagnostics. A process that exits unexpectedly within 10 seconds is surfaced as an early exit.

When Port Lens starts an App and observes its listener, it persists managed identity metadata. After Port Lens itself restarts, a matching surviving listener is recognized as previously managed instead of being reported as a different process.

On the merged baseline, Stop / Restart ownership is still session-local. On the active `feature/runtime-reattach` branch, Windows can recover lifecycle authority only after verifying the persisted listener generation and managed root process identity; reattached Stop re-verifies root identity immediately before termination.

## Compact mode

Compact mode is enabled by default. Minimize enters the floating compact monitor; disabling Compact mode makes Minimize hide to tray. Close exits the application.

Compact positioning clamps to the monitor work area. Windows now uses a 0 px edge margin so the bubble can sit directly against the work-area/taskbar boundary without covering the taskbar. Non-Windows desktop builds retain the existing 12 px edge margin.

On stacked PR #4, hovering the compact bubble expands the native window upward and shows the registered App list with green/gray status dots. Up to 8 rows are visible before scrolling; closing hover restores the pre-hover compact position.

## Windows package evidence

Latest active-branch package evidence: runtime-reattach bundle run `34595549143` at `afb6d3e` (PASS).

- Portable: `Port.Lens_0.3.0_x64-portable.exe`
  - SHA-256 `18d5425f3b0b1d69a2eeb2787734305a7559ed37aa08e0b679beaa296e2c2a61`
- MSI: `Port Lens_0.3.0_x64_en-US.msi`
  - SHA-256 `5d8dfdaf6a1889b258cc0c734eb8f7383631a3e868d998b45f30aacc64dbe333`
- NSIS: `Port Lens_0.3.0_x64-setup.exe`
  - SHA-256 `b52e5c23833ce5e11f7efae66c7215ad4f0d9946f32c08b07b8870a440afc5a2`

Latest merged-baseline package evidence remains compact-position bundle run `34578926043` (PASS).
