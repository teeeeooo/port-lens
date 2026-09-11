# Port Lens State

Last updated: 2026-09-11

## Current baseline

Port Lens v0.3.0 preview is a Tauri/Rust + React/TypeScript desktop app for discovering TCP listeners and managing selected local services.

Latest validated implementation commit before these state docs: `300974e` (`fix: preserve managed app identity and Windows quoting`).

Validated on GitHub CI:
- macOS: PASS
- Windows: PASS
- Windows Bundle: PASS (`34573936844`)

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

For safety, Stop / Restart ownership is still session-local. A previously managed listener recognized after Port Lens restarts is monitored and can be opened, but is not yet reattached for Stop / Restart.

## Compact mode

Compact mode is enabled by default. Minimize enters the floating compact monitor; disabling Compact mode makes Minimize hide to tray. Close exits the application.

Current Port Lens compact positioning clamps to the monitor work area with a forced 12 px edge margin. On Windows this leaves a visible gap above the taskbar. Token Lens uses a different Windows policy; Port Lens will be polished separately rather than changing this baseline during the stability merge.

## Windows package evidence

Latest Windows Bundle run: `34573936844`.

- Portable: `Port.Lens_0.3.0_x64-portable.exe`
  - SHA-256 `6b1a42e245f7a05da5657f32135140c3302a63a73bea90700ae34c1e5d3e8428`
- MSI: `Port Lens_0.3.0_x64_en-US.msi`
  - SHA-256 `a26b19cbde325815bfdb166798fffd1f42d4d575c4132a9fae540a841d8dda08`
- NSIS: `Port Lens_0.3.0_x64-setup.exe`
  - SHA-256 `4f101ae911160e468e449bdf4377ab7b77f5ccbc7dddc19e2fc6439fd6eda324`

The preceding bundle run `34571634102` failed only while downloading an NSIS utility with Windows socket error 10054; the unchanged code path succeeded on the next run.
