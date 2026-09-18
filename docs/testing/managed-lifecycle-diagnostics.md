# Port Lens lifecycle diagnostics — Windows candidate check

## Final scope

PR #5 was merged at `879fdc8` after the user's Windows drag PASS for `066a688`.
PR #6 now **removes automatic managed stdout/stderr capture**. Its previous rotating
collector candidate `12b2f2d` is superseded. Do not use that candidate's artifacts for
this validation. N01 onward remains deferred. This work does not publish a release.

## Behavior

- Commands started by Port Lens receive null devices for stdout/stderr. No App log
  files, background collectors, capture queues, or App-specific Logs button/API.
- An App may still write its own files or explicitly redirect output in its command.
  Port Lens does not intercept or delete those files.
- Port Lens's own `port-lens.log` retains the existing 1 MiB rotation threshold and
  one previous file. Open it through Settings → Diagnostics → Open logs.
- Diagnostic events distinguish these facts:

| Event | Meaning |
| --- | --- |
| `managed_action phase=requested` | Start/Stop/Restart requested for an App ID |
| `managed_action phase=returned result=ok/error` | The existing action returned; failure includes its reason |
| `managed_process_start result=process_created` | The launch process was created, with PID and configured port |
| `managed_process_start result=already_running` | Start reused an existing live managed runtime |
| `managed_identity listenerObserved=… identityRecorded=…` | The existing listener/identity observation path succeeded or exhausted its attempts |
| `managed_process_stop result=termination_command_succeeded/already_not_running` | Existing termination command succeeded or the existing liveness check found no running root |
| `managed_process_exit` | While Port Lens was running, its child watcher observed exit code (if available), elapsed time, expected/unexpected and early-exit state |

These are not HTTP health checks. Process creation is not proof of listener readiness;
a listener is not proof of application health. A successful forced termination command
is not a graceful exit. Reattached processes have no original Child wait handle, so
exit codes are not fabricated for them. The existing scan/liveness limitations remain.
No new polling loop or subprocess is added for diagnostics.

Closing Port Lens leaves managed Apps running, with no Port Lens log collector left
behind. Port Lens does not record events while it is closed. Reopening uses existing
verified reattach behavior; output remains discarded for this run.

**Transition from earlier versions:** already-running Apps retain their old file/pipe
handles. Stop them in the old version first, Quit that version, then Start with this
candidate. In particular, collectors launched by `12b2f2d` end when their producer
handles close; the new candidate does not forcibly kill those old collectors or Apps.
Old `managed-apps` log folders are preserved and not migrated/deleted automatically.

## Windows checks (user)

1. Stop test Apps in the previous version and Quit it. Download the **new** Portable
   artifact linked from PR #6 and use a separate folder. The version still reads
   `0.3.1`; identify the candidate by commit and workflow run.
2. Register a normal test server and Start it. Verify the listener is detected and
   the server responds. No App-specific Logs button should remain. In Settings,
   Port Lens diagnostic logs should show the start request/result, process PID and
   eventual listener/identity observation.
3. Start a test command that exits immediately with a nonzero code (for example
   `node -e "process.exit(7)"`). Verify the exit code/early-exit UI and diagnostic
   record; it must not suggest that captured App logs exist.
4. Test an occupied port and a missing executable/invalid cwd. Verify the reported
   failure and the lifecycle result/exit record appropriate to that failure.
   A shell can start successfully and then report a missing command by exiting.
5. Stop and Restart the normal server. Verify requests, action results and observed
   exit records in `port-lens.log`; check listener state and normal process ownership.
6. With the server running, Quit Port Lens. Verify the server still responds and
   no new Port Lens log-writer process remains. If the server prints output, it must
   continue normally without producing Port Lens stdout/stderr files. Its own logs
   may still change. Reopen and verify normal reattach/Stop behavior.
7. Confirm compact drag/hover/Open still work as a short regression check.

Report: SHA, Start/Stop/Restart, early exit/failure diagnostics, Quit/reopen behavior,
absence of new collectors/App capture files, and compact smoke PASS/FAIL.

## Automated coverage

- Real managed shell entry: 8 MiB stdout + 8 MiB stderr, completion without a reader,
  nonzero exit code preserved, no capture files created, historical file untouched.
- Launcher-process exit followed by delayed App stdout/stderr writes and completion.
- Lifecycle request/result records, failure sanitization and own-log rotation.
- Existing Windows quoting, managed runtime identity/reattach, port policy and all
  frontend regressions. The single ignored test is a subprocess entry invoked by
  the parent-exit test, not an untested functional acceptance case.

Local candidate check: macOS Rust 42/42 PASS (one subprocess-entry test invoked by
a parent test), Clippy with warnings denied PASS, rustfmt PASS, frontend 26/26 and
production build PASS. Candidate-specific Actions and Windows manual results must
be read separately from PR #6; previous collector-candidate results do not apply.
