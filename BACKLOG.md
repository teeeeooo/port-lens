# Port Lens Backlog

Last updated: 2026-09-18

Only current actionable or intentionally deferred work is kept here. Completed implementation and investigation history belongs in `STATE.md` and `docs/audits/`.

## Active work

### Purpose-audit candidate — review and Windows validation

Branch: `fix/product-audit-2026-09-18`. The existing `v0.3.1` release is not replaced.

- A01: full-inventory guard prevents unmanaged Kill through another port of the same managed child; local policy tests pass.
- A02: runtime probes run off the event thread and outside registry locks; delayed probes cannot prune a replacement PID; local tests pass.
- A10: EN/KO README now describes existing Minimize versus X/Quit behavior correctly; no lifecycle behavior change.
- Gate: Windows CI plus manual dual-port, reattach, concurrent runtime refresh, compact/tray regression checks. Do not treat local macOS PASS as Windows GUI PASS.

### Follow-up stabilization — specified, not implemented

Detailed scope, source locations, failure scenarios, and acceptance tests: `docs/audits/2026-09-18-product-purpose-audit.md`.

1. A03 (P1): fail-closed listener scan, explicit UTF-8/read errors, no-match versus provider failure.
2. A04 (P1): atomic registry/settings persistence and publish-after-commit; preserve corrupt inputs for recovery.
3. A05 (P1): per-App lifecycle/config operation gate and independent frontend busy state.
4. A06/A07 (P1/P2): source-specific freshness/errors, common identity/status/count selectors.
5. A08 (P1/P2): Unknown versus Exited process probes, bounded queries, remaining termination identity hardening; address safety-critical portions alongside A03.
6. A09 (P2): bounded stdout/stderr retention during long-running processes.

Keep each follow-up bounded; do not combine them into a drag rewrite or feature expansion.

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
