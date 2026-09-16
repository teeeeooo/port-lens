# Port Lens Backlog

Last updated: 2026-09-16

Only current actionable or intentionally deferred work is kept here. Completed implementation and investigation history belongs in `STATE.md` and `docs/audits/`.

## Active work

None. `main` is clean and synchronized with `origin/main`; there are no open PRs or non-main branches/worktrees.

## Deferred product work

### Future scope — not scheduled

- UDP listener discovery
- HTTP health checks for registered apps

Existing external listeners can already be registered as persistent monitoring-only Apps. Lifecycle ownership is intentionally granted only to Port Lens-started runtimes or runtimes that pass verified reattach checks; silent adoption of arbitrary external processes is not planned work.

### Compact drag residual

Status: **ACCEPTED / NO ACTIVE WORK**.

Windows validation of the final D0.5 path still shows a short drag-start hitch/jump on roughly 3 of 10 starts when monitoring/render work overlaps drag initiation. Cursor offset and sustained catch-up jump are absent; compact polling, drag-end refresh, hover hide, and `Open` remain correct.

Do not reopen the native D1 capture state machine unless the residual becomes materially worse or new evidence justifies the added lifecycle/regression risk.

## Evidence

- current baseline and accepted limitations: `STATE.md`
- released stability baseline: `docs/releases/v0.3.1.md`
- compact drag/hover root audit: `docs/audits/2026-09-15-compact-drag-hover-audit.md`
- history correction / Token Lens comparison: `docs/audits/2026-09-15-compact-drag-history-token-lens-audit.md`
- PoC-C deep audit: `docs/audits/2026-09-15-compact-drag-poc-c-deep-audit.md`
- final PoC-D / D0.5 evidence: `docs/audits/2026-09-15-compact-drag-poc-d-audit.md`
