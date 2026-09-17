# Managed output retention — Windows candidate check

## Scope

PR #5 was merged at `879fdc8` after the user's Windows drag PASS for `066a688`.
This separate candidate implements A09 only. N01~N03 / CL-11~CL-12 remain deferred.
No release is published by this work; Windows manual validation belongs to the user.

## Retention contract

- Each managed App has `stdout.log`, `stdout.log.1`, `stderr.log`, `stderr.log.1`.
- Each output file is capped at 5 MiB (5,242,880 bytes), including run/exit markers.
  The four output files total at most 20 MiB. There is no `.log.2` or date-based archive.
- A tiny lock file and an optional latest-warning `*.log.capture-status` file (at most
  2 KiB per stream) are additional metadata, not output history. There is no global
  limit on the number of App directories and removed Apps' folders are not deleted.
- Existing oversized logs are reduced to their latest 5 MiB when that App is next
  started by this version. No whole-file allocation or temporary full-size copy.
- Run context is repeated at rotation; the parent writes exit context through the
  same bounded collector. Raw output is retained as bytes; a rotation boundary can
  split a very long line or a UTF-8 character.
- Port Lens's own `port-lens.log` keeps its existing 1 MiB rotation policy.

## Lifecycle and failure policy

The same executable runs in a private, windowless capture mode before Tauri or the
single-instance plugin starts. There is one collector per stream. Managed commands
still have their own original shell PID and ancestry; a logging wrapper is not made
into the managed root. Capture startup/legacy migration runs off the UI thread.

Collectors own the output file handles, close them before rotation, and hold an OS
file lock to reject simultaneous writers for the same stream. During a restart a
new collector allows up to 2 seconds for its predecessor to finish. Startup fails
before launching the managed command if a collector cannot become ready.

The pipe reader uses 8 KiB chunks and a bounded 64-chunk queue (512 KiB per stream).
It does not wait on the disk writer. If storage stalls, queue overflow is discarded;
if writes/rotation fail, output is drained and discarded while retrying at most once
per second. The latest warning is visible in the existing App log folder, and
omission markers are written when storage is writable again. Capture does not
promise lossless logging under overload or disk failure. If warning-file writes
also fail, a status file cannot be guaranteed. Unexpected collector termination or
OS pipe failure is not made transparent to the producing process.

Closing Port Lens leaves collectors and managed processes running. Collectors exit
when every producer closes its inherited pipe. Reopening Port Lens keeps the existing
verified-reattach rules. Stop the managed Apps before replacing/upgrading the Windows
EXE: collectors execute that same file and Windows can keep it locked while they run.

**Already-running Apps started by an older version still use their old file handles.**
Reattach alone cannot replace stdout/stderr: Stop/Start once in this version to opt
that run into bounded capture. This does not auto-restart any user App.

## Manual Windows check (user)

1. Quit the previous Port Lens. Download this candidate's **Portable** artifact from
   its Windows Bundle run and extract it into a separate folder. Version display is
   still `0.3.1`; use the PR commit/run to identify the candidate.
2. Use a chatty development service, or download [managed-log-stress.mjs](./managed-log-stress.mjs)
   into a test folder with Node.js available. Register an App with command
   `node managed-log-stress.mjs 32109`, that folder as cwd, and port `32109` (choose
   another unused port in both places if needed). Start it from Port Lens.
3. Open the App's log folder. Wait about 30–60 seconds. Check that each output file
   is at most **5,242,880 bytes**, only one `.log.1` exists per stream, and recent
   timestamps/sequence numbers continue appearing after repeated rotation.
4. While it outputs, check ordinary compact drag/hover/Open. Visit
   `http://127.0.0.1:32109` to confirm the probe still responds.
5. Quit Port Lens for 30 seconds. Verify the service responds and log sizes stay
   bounded. Reopen Port Lens and verify normal reattach/Stop/Start/Restart behavior.
6. Stop the App. Verify output stops and (while the owner is open) the exit marker
   appears in stderr. Start again and check the new run marker. Do not manually
   delete files while the service is running just to enforce the cap.

Optional existing-log migration: stop the test App, place an oversized disposable
`stdout.log` / `stdout.log.1` in this test App's folder, then Start and verify the cap.
Do not use valuable historical logs as the test fixture.

Report: candidate SHA, retention PASS/FAIL, UI-closed capture PASS/FAIL,
reattach/Stop/Restart PASS/FAIL, drag smoke PASS/FAIL, and any capture-status warning.

## Automated coverage

- Bounded rotation over many generations and newline-free data; newest bytes retained.
- Legacy file trimming, repeated run markers, and concurrent writer rejection.
- Refused rotation never grows the active file; capture drains after storage failure
  and a subsequent start recovers. Queue overflow/disconnection remains bounded.
- Parent-process exit followed by delayed producer output, EOF and lock release.
- Actual shipped executable's early collector entry, repeated 5 MiB rotation, final
  output and EOF termination without GUI initialization.
- Existing managed stdout/stderr/exit markers, quoted Windows command and runtime
  identity/reattach tests remain in the normal Rust suite.

Three ignored Rust test functions are subprocess entrypoints invoked by passing
parent tests; they are not skipped functional acceptance cases.

Local macOS candidate validation: rustfmt PASS; Clippy all-targets/all-features
with warnings denied PASS; Rust 48 unit tests + 1 shipped-entry integration test
PASS. Three internal subprocess entrypoints are invoked by these tests. The local
build used debug symbols/incremental compilation disabled to limit disk usage.
GitHub CI uses its existing default profiles; Windows manual results are pending.
