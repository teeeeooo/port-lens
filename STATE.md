# Port Lens State

Last updated: 2026-09-16

## Current baseline

- product: Port Lens `0.3.0` preview, Tauri/Rust + React/TypeScript
- branch: `main` only
- `main` / `origin/main`: `faf02cce3416e086c59b2a4a12600c68eab21228` (PR #4 merge)
- open PRs: none
- extra local/remote branches: none
- extra worktrees: none
- working tree: clean

Recent merged milestones:
- PR #3 — verified managed-runtime reattach and lifecycle hardening
- PR #4 — compact registered-app hover, expanded-window restore fixes, and final compact drag stabilization

Final integration commit before PR #4 merge: `680d18e9044c174e73c6f806b80f45d1a566eadb`.

## Validation state

Final PR #4 integration passed:
- frontend production build
- rustfmt
- Clippy with `-D warnings`
- Rust tests: **36/36 PASS**
- GitHub CI: Windows **PASS**, macOS **PASS**
- Windows Bundle run `35054358897`: **PASS**

Windows artifacts from that integration run: Portable `10430516595`, NSIS `10430511586`, MSI `10429854175`.

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

- `docs/audits/2026-09-15-compact-drag-hover-audit.md`
- `docs/audits/2026-09-15-compact-drag-history-token-lens-audit.md`
- `docs/audits/2026-09-15-compact-drag-poc-c-deep-audit.md`
- `docs/audits/2026-09-15-compact-drag-poc-d-audit.md`

## Next action

There is no active blocker. Use `BACKLOG.md` for deferred product/release work; reopen compact drag only if the accepted residual materially worsens or new evidence changes the risk/reward of D1.
