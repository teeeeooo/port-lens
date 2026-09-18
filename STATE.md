# Port Lens State

Last updated: 2026-09-18

## Current baseline

- Product: Port Lens `0.3.2` Preview, Tauri/Rust + React/TypeScript.
- Release tag: `v0.3.2`; release notes: `docs/releases/v0.3.2.md`.
- Project status: **COMPLETE / no active work**, by user decision. Future work requires a new request.
- PR #5 merged at `879fdc8`: purpose-audit fixes and compact lifecycle stabilization.
- PR #6 merged at `d541dc4`: remove App stdout/stderr capture and retain lifecycle diagnostics.
- Release preparation changes version metadata and documentation only; execution code matches user-validated candidate `0332413`.

## Validation evidence

- User confirmed Windows drag validation for `066a688`, then Windows lifecycle-diagnostics validation for `0332413`: no issues reported; merge/release authorized.
- Candidate CI `35292253583`: Windows and macOS PASS; frontend 26/26, Rust Windows 43 and macOS 42 tests passed. One internal subprocess test entry is intentionally ignored and invoked by a passing parent test.
- Candidate Windows Bundle `35292252630`: NSIS, MSI and Portable PASS.
- Release-specific CI, bundle runs and checksums are recorded on the GitHub Release. Candidate GUI validation is inherited; version-only release packages are not a separate manual GUI test.

## Product behavior and guardrails

- Port Lens manages TCP listeners and registered Apps; it is not an App log storage service.
- App stdout/stderr are not captured. App-owned logging and explicit redirection remain controlled by each App.
- Start/Stop/Restart requests/results, process creation, listener/identity observations and observed exits are recorded in Port Lens's own rotating diagnostics (1 MiB threshold plus one previous file).
- Quit leaves managed Apps alive without Port Lens log collectors. Existing old-version Apps must be stopped/restarted once to replace inherited output handles; historical log files are left untouched.
- Action success is not an HTTP health check or proof of graceful shutdown.
- Preserve verified runtime ownership, PID generation, creation-time and ancestry checks.
- Preserve separate hover window, 4 px drag threshold, native cursor sampling, position-only movement, idle compact polling and drag-end catch-up.
- Prior brief drag-start hitch remains an accepted residual; do not claim measured elimination. Native D1 redesign is deferred.

## Deferred scope

All remaining A03–A08, N01–N03, CL-11/CL-12 and product expansion are unscheduled. Do not reopen them without a new user request. See `BACKLOG.md`.

## Evidence map

- `docs/releases/v0.3.2.md`
- `docs/releases/v0.3.1.md` (historical release evidence)
- `docs/testing/managed-lifecycle-diagnostics.md`
- `docs/audits/2026-09-18-product-purpose-audit.md`
- `docs/audits/2026-09-18-compact-window-lifecycle-audit.md`
- `docs/audits/2026-09-15-compact-drag-poc-d-audit.md`
