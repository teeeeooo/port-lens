# Port Lens Backlog

Last updated: 2026-09-18

Only current actionable or intentionally deferred work is kept here. Completed implementation and investigation history belongs in `STATE.md` and `docs/audits/`.

## Active work

### Managed output retention — separate candidate after PR #5 merge

- PR #5 merged (`879fdc8`); user Windows drag PASS recorded for `066a688`.
- Implement A09: 5 MiB × 2 files per stream, 20 MiB output history per App, live rotation and UI-closed capture.
- Deliver a separate PR and Windows Actions artifact. User performs Windows manual validation; do not merge this PR or release yet.
- Test guide: `docs/testing/managed-log-retention.md`. N01 onward remains deferred.


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
6. A09 (P2): implementation in separate `fix/managed-log-retention` candidate; automatic gates and Windows user validation pending. See `docs/testing/managed-log-retention.md`.

Keep each follow-up bounded; do not combine them into a drag rewrite or feature expansion.

### Compact lifecycle candidate — drag validation toward release

Specification: `docs/audits/2026-09-18-compact-window-lifecycle-audit.md`.

- Resume from the existing local changes on parent `363e77b`; the earlier commit/push blockage is historical. Prepare the candidate commit and Windows build on PR #5. Frontend regressions rechecked: 26/26 PASS.
- CL-01~CL-10: session/epoch cancellation, loading/catch-up recovery, latest-only move pump, hover ownership/ACK, Open frontend coordination. Candidate implementation; not a Windows GUI PASS.
- Gate: this candidate's Windows/macOS CI and package build, then Windows drag/hover/polling/cancellation/ordinary Open regression checks. Release if these pass; preserve the existing accepted brief drag-start hitch tolerance. Do not copy parent test PASS forward.
- User decision (2026-09-18): N01 onward is deferred and does not block this release. Monitor-removal fault scenarios, native fault injection, document restarts and latency instrumentation are not mandatory gates for this scope.

### Deferred compact follow-ups — do not open for this release

- N01 (P1): recoverable native Open/collapse transitions; retain snapshot until success; avoid native calls under state mutex.
- N02 (P1/P2): monitor removal / mixed DPI / maximized-normal restore bounds.
- N03 (P2): invalidate stale keeper work at actual native application time.
- CL-11 (P2): hover document/session handshake recovery; current ACK fix is same-document only.
- CL-12 (conditional): instrument residual latency, compare baseline/runtime-only/current candidate; add native session guard only with evidence.

## Deferred product work

### Future scope — not scheduled

- UDP listener discovery
- HTTP health checks for registered apps

Existing external listeners can already be registered as persistent monitoring-only Apps. Lifecycle ownership is intentionally granted only to Port Lens-started runtimes or runtimes that pass verified reattach checks; silent adoption of arbitrary external processes is not planned work.

### Compact drag residual

Status: **ACCEPTED for released v0.3.1; compact lifecycle follow-up active on PR #5 by explicit user request**.

Windows validation of the final D0.5 path still shows a short drag-start hitch/jump on roughly 3 of 10 starts when monitoring/render work overlaps drag initiation. Cursor offset and sustained catch-up jump are absent; compact polling, drag-end refresh, hover hide, and `Open` remain correct.

The focused follow-up fixes lifecycle races without switching to D1. Keep D1 evidence-gated; do not label the approximate 3/10 native hitch solved before Windows comparison testing.

## Evidence

- current baseline and accepted limitations: `STATE.md`
- released stability baseline: `docs/releases/v0.3.1.md`
- compact drag/hover root audit: `docs/audits/2026-09-15-compact-drag-hover-audit.md`
- history correction / Token Lens comparison: `docs/audits/2026-09-15-compact-drag-history-token-lens-audit.md`
- PoC-C deep audit: `docs/audits/2026-09-15-compact-drag-poc-c-deep-audit.md`
- final PoC-D / D0.5 evidence: `docs/audits/2026-09-15-compact-drag-poc-d-audit.md`
