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

The drag issue is **CLOSED / ACCEPTED RESIDUAL** at the D0.5 quality level.

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

The 2026-09-18 purpose audit has a separate candidate: full-inventory unmanaged Kill protection, nonblocking runtime probes with snapshot-safe pruning, five regression tests, and EN/KO close-behavior documentation corrections. Local macOS validation: frontend build PASS, Clippy `-D warnings` PASS, Rust tests **41/41 PASS**. Windows CI and native runtime evidence must be checked separately before merge/release. No drag/window execution code was changed.

Next: review the candidate and run the Windows regression gate in `docs/audits/2026-09-18-product-purpose-audit.md`. That audit records remaining scan correctness, persistence, operation concurrency, freshness, identity, process-query, and log-retention work. `BACKLOG.md` is the execution index. Reopen compact drag only if the accepted residual materially worsens or new evidence changes the risk/reward of D1.
