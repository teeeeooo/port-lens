# Port Lens Backlog

Last updated: 2026-09-16

Only current actionable or intentionally deferred work is kept here. Completed implementation and investigation history belongs in `STATE.md` and `docs/audits/`.

## Active work

None. `main` is clean and synchronized with `origin/main`; there are no open PRs or non-main branches/worktrees.

## Deferred product work

### Release housekeeping

- decide whether the next public preview should keep `0.3.0` or receive a version bump
- prepare release notes and publish the corresponding Windows NSIS / MSI / Portable artifacts and checksums when a release is cut

### Future scope — not scheduled

- UDP listener discovery
- HTTP health checks for registered apps
- safe adoption of externally started servers into managed ownership

### Compact drag residual

Status: **ACCEPTED / NO ACTIVE WORK**.

Windows validation of the final D0.5 path still shows a short drag-start hitch/jump on roughly 3 of 10 starts when monitoring/render work overlaps drag initiation. Cursor offset and sustained catch-up jump are absent; compact polling, drag-end refresh, hover hide, and `Open` remain correct.

Do not reopen the native D1 capture state machine unless the residual becomes materially worse or new evidence justifies the added lifecycle/regression risk.

## Evidence

- current baseline and accepted limitations: `STATE.md`
- compact drag/hover root audit: `docs/audits/2026-09-15-compact-drag-hover-audit.md`
- history correction / Token Lens comparison: `docs/audits/2026-09-15-compact-drag-history-token-lens-audit.md`
- PoC-C deep audit: `docs/audits/2026-09-15-compact-drag-poc-c-deep-audit.md`
- final PoC-D / D0.5 evidence: `docs/audits/2026-09-15-compact-drag-poc-d-audit.md`
