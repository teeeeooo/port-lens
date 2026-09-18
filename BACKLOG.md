# Port Lens Backlog

Last updated: 2026-09-18

## Active work

None. **Project complete for now**, by user decision. PR #5 and PR #6 are merged for v0.3.2 after user Windows validation. Do not start deferred work without a new request.

## Deferred work — not scheduled

### Follow-up stabilization — specified, not implemented

Detailed scope, source locations, failure scenarios, and acceptance tests: `docs/audits/2026-09-18-product-purpose-audit.md`.

1. A03 (P1): fail-closed listener scan, explicit UTF-8/read errors, no-match versus provider failure.
2. A04 (P1): atomic registry/settings persistence and publish-after-commit; preserve corrupt inputs for recovery.
3. A05 (P1): per-App lifecycle/config operation gate and independent frontend busy state.
4. A06/A07 (P1/P2): source-specific freshness/errors, common identity/status/count selectors.
5. A08 (P1/P2): Unknown versus Exited process probes, bounded queries, remaining termination identity hardening; address safety-critical portions alongside A03.
A09 is complete in PR #6: automatic App output capture removed; lifecycle diagnostics retained and user Windows validation passed.

Keep each follow-up bounded; do not combine them into a drag rewrite or feature expansion.

### Deferred compact follow-ups — do not open for this release

- N01 (P1): recoverable native Open/collapse transitions; retain snapshot until success; avoid native calls under state mutex.
- N02 (P1/P2): monitor removal / mixed DPI / maximized-normal restore bounds.
- N03 (P2): invalidate stale keeper work at actual native application time.
- CL-11 (P2): hover document/session handshake recovery; current ACK fix is same-document only.
- CL-12 (conditional): instrument residual latency, compare baseline/runtime-only/current candidate; add native session guard only with evidence.

### Product expansion

- UDP listener discovery.
- HTTP health checks for registered Apps.
- Native D1 drag redesign remains evidence-gated; the prior brief drag-start hitch is an accepted residual, not a claim of measured elimination.

External listeners can already be registered for monitoring. Silent lifecycle adoption of arbitrary external processes is not planned.

## Evidence

- Current baseline and validation: `STATE.md`.
- Release summary: `docs/releases/v0.3.2.md`.
- Detailed findings: `docs/audits/2026-09-18-product-purpose-audit.md` and `docs/audits/2026-09-18-compact-window-lifecycle-audit.md`.
