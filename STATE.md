# Port Lens State

Last updated: 2026-09-18

## Current baseline

- product: Port Lens `0.3.1` Preview, Tauri/Rust + React/TypeScript
- release tag: `v0.3.1`
- release commit: `e32e9a95018e4556e733cb75b53c1a07e597dc49`
- GitHub Release: **published / pre-release** on 2026-09-16
- released default-branch baseline: `main` at `320fe90045ef60d021f88ab9646c8e8a952d5559`
- release-time housekeeping (2026-09-16): no open PRs, extra branches, or worktrees
- current audit candidate: `fix/product-audit-2026-09-18`, isolated worktree; not released

Recent merged milestones:
- PR #3 — verified managed-runtime reattach and lifecycle hardening
- PR #4 — compact registered-app hover, expanded-window restore fixes, and final compact drag stabilization

Final Windows runtime-validated integration commit before release: `680d18e9044c174e73c6f806b80f45d1a566eadb`. There are no `src/` or `src-tauri/src/` execution-code changes between that commit and the `v0.3.1` release commit.

## Validation state

`v0.3.1` release gates:
- local frontend production build: **PASS**
- rustfmt: **PASS**
- Clippy with `-D warnings`: **PASS**
- Rust tests: **36/36 PASS**
- GitHub CI run `35059809355`: Windows **PASS**, macOS **PASS**
- Windows Bundle run `35060240844`: **PASS**
- release assets uploaded: NSIS / MSI / Portable / `SHA256SUMS.txt`

Release artifact workflow IDs: Portable `10432077432`, NSIS `10432097263`, MSI `10431673891`.

Release SHA-256:
- NSIS: `742dd65b6658346e917295369dffc02cbac69084080d01737c93b005adf35754`
- MSI: `1e19e6ddff9c0c964febc4ba7bcd66efce610ed7fef154865ea83863ae1c04ac`
- Portable: `ea458032b003e312fa7a2d7207fc9e347a948ef5b6ba9dc7b3781bab68ba4992`

## Runtime status

Validated behavior:
- live TCP listener monitoring and Managed App state refresh
- external listeners can be registered as persistent monitoring-only Apps
- Start / Stop / Restart with managed ownership checks
- verified runtime reattach after Port Lens restart
- separate `compact-hover` window for registered-app hover content
- compact `Open` restores the expanded window without the previous size-growth drift
- compact polling remains active while idle and resumes with catch-up refresh after drag
- hover is hidden at actual drag start rather than continuously tracked across windows
- taskbar overlap/topmost behavior, saved compact position, mixed-DPI/multi-monitor handling, startup gating, and compact clipping are preserved

### Accepted compact-drag residual

The released drag baseline is **CLOSED / ACCEPTED RESIDUAL** at the D0.5 quality level. A user-requested compact lifecycle re-audit is active on PR #5; this does not establish that the native hitch is fixed.

On the tested Windows machine, roughly 3 of 10 drag starts can show one short hitch/jump when monitoring/render work overlaps drag initiation. Once movement is underway:
- no persistent cursor/window offset
- no sustained pause/catch-up jump
- micro-stutter is acceptable
- idle polling and drag-end polling resume correctly
- hover-visible drag → hide is correct
- `Open` remains correct

This residual is accepted for the current preview. The larger Windows-native D1 `SetCapture` state machine is deferred because its lifecycle/regression cost is disproportionate to the remaining symptom.

## Guardrails

- do not reopen `WM_NCLBUTTONDOWN` / `HTCAPTION` / `SC_MOVE` / Send-vs-Post / zero-lParam wake-up / `data-tauri-drag-region` tuning for the closed drag issue without new evidence
- keep compact monitoring live; disabling polling for the entire compact state is not acceptable product behavior
- preserve the separate hover-window architecture and the compact `Open` deadlock/restore fixes
- preserve PR #3 runtime identity, creation-time, ancestry, and lifecycle-suppression checks

## Evidence map

- release summary: `docs/releases/v0.3.1.md`
- `docs/audits/2026-09-15-compact-drag-hover-audit.md`
- `docs/audits/2026-09-15-compact-drag-history-token-lens-audit.md`
- `docs/audits/2026-09-15-compact-drag-poc-c-deep-audit.md`
- `docs/audits/2026-09-15-compact-drag-poc-d-audit.md`

## Next action

`v0.3.1` release housekeeping remains complete; the released build is unchanged.

The 2026-09-18 purpose audit has a separate candidate: full-inventory unmanaged Kill protection, nonblocking runtime probes with snapshot-safe pruning, five regression tests, and EN/KO close-behavior documentation corrections. Local macOS validation: frontend build PASS, Clippy `-D warnings` PASS, Rust tests **41/41 PASS**. Windows CI and native runtime evidence must be checked separately before merge/release. That parent commit did not change drag/window execution code. The subsequent compact lifecycle audit below does change those paths.

Next: review the candidate and run the Windows regression gate in `docs/audits/2026-09-18-product-purpose-audit.md`. That audit records remaining scan correctness, persistence, operation concurrency, freshness, identity, process-query, and log-retention work. `BACKLOG.md` is the execution index. Native D1 remains evidence-gated; the user explicitly requested the compact lifecycle re-audit described below.

## Log-retention follow-up — 2026-09-18

- PR #5 merged at `879fdc8` after user Windows drag PASS for `066a688`. Its CI passed on Windows/macOS; Bundle `35285774074` passed. No new release published.
- Current work: A09 on `fix/managed-log-retention`, separate PR. Implemented bounded live stdout/stderr capture: each stream 5 MiB current + 5 MiB previous, 20 MiB output history per App.
- Preserve managed process ancestry and UI-closed capture via detached collectors using the same EXE's early, windowless entry. Run startup/migration off the UI thread. Bounded queues discard excess output with warning/omission records if storage cannot keep up.
- Existing old-version processes require Stop/Start to adopt capture. Small lock/status metadata is outside the 20 MiB output-file total. Windows executable replacement requires collectors to have exited.
- Windows manual validation is reserved for the user. Prepare CI and Windows Bundle artifacts, then leave this PR unmerged and the release unpublished.
- Test instructions and operational limits: `docs/testing/managed-log-retention.md`. N01 onward remains deferred.

## Current release decision — 2026-09-18

- User scope: evaluate the existing purpose-audit + CL-01~CL-10 candidate, verify Windows drag behavior, then release if validation passes.
- N01/N02/N03 and CL-11/CL-12 are deferred by user decision. They are not prerequisites for this release; do not open new implementation or instrumentation work for them.
- Gate: candidate-specific Windows/macOS CI and Windows package build, then manual drag regression checks covering hover closed/open, polling boundaries, long/repeated gestures, cancellation, catch-up refresh, and ordinary Open/tray Open.
- Preserve the accepted v0.3.1 brief drag-start hitch tolerance: release requires no material regression, no persistent offset/catch-up jump, and working hover/polling/Open. Do not claim hitch elimination without measurements.
- Use the existing candidate worktree. Prepare a reviewable commit and Windows artifact; publish only after Windows manual validation passes. No native redesign or unrelated backlog work is included.
- Latest local recheck: frontend 26/26 PASS, app/test TypeScript PASS, production build PASS, rustfmt PASS, diff whitespace check PASS. Rust tests/Clippy and Windows GUI are not validated by that local recheck.

## Compact lifecycle follow-up — 2026-09-18

- Handoff state before current release preparation: UNCOMMITTED follow-up on `fix/product-audit-2026-09-18` (parent `363e77b`); the previous commit/push attempt was blocked. Main and v0.3.1 are unchanged. Candidate commit, CI and bundle evidence will be recorded in the PR and workflow runs.
- Implemented: drag session/refresh epoch guards, distinct release/cancel behavior, bounded per-gesture move queue, blur/Open cleanup, foreground loading settlement, independent inventory/monitored catch-up, pending-hover hide, nondecreasing render ACK, and persistence/hover ownership separation.
- Native cursor sampling, position-only movement, separate hover window, 4px threshold and idle polling cadence are preserved. Already-issued invokes are not cancellable; the queue bound is per gesture.
- Local frontend regressions **26/26 PASS**, app/test TypeScript PASS, production build PASS, rustfmt PASS. Local `cargo test` was blocked by disk exhaustion (ENOSPC, exit101); Clippy did not run in that chain. Only this worktree's generated target cache (1.3GiB) was cleaned.
- Windows GUI/manual validation has NOT been performed for this candidate. Previous 36/41-test and Windows PASS records refer to their stated earlier commits, not this follow-up.
- Deferred, outside this release: N01 native Open partial-failure recovery; N02 display/maximized restore; N03 stale keeper ordering; CL-11 hover document epochs; CL-12 conditional native session/latency instrumentation.
- Handoff and Windows gate: `docs/audits/2026-09-18-compact-window-lifecycle-audit.md`.
