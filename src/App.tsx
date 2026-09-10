import { FormEvent, useCallback, useEffect, useMemo, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  getListeners,
  getManagedApps,
  getManagedRuntimes,
  killListenerProcess,
  removeManagedApp,
  restartManagedApp,
  saveManagedApp,
  startManagedApp,
  stopManagedApp,
} from "./api";
import type { ListenerInfo, ManagedApp, ManagedRuntime, ManagedStatus } from "./types";
import "./App.css";

type ManagedRow = ManagedApp & {
  status: ManagedStatus;
  runtime?: ManagedRuntime;
  listener?: ListenerInfo;
};

const createDraft = (): ManagedApp => ({
  id: crypto.randomUUID(),
  name: "",
  port: 3000,
  command: "npm run dev",
  cwd: "",
});

function messageOf(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}

function App() {
  const [listeners, setListeners] = useState<ListenerInfo[]>([]);
  const [apps, setApps] = useState<ManagedApp[]>([]);
  const [runtimes, setRuntimes] = useState<ManagedRuntime[]>([]);
  const [query, setQuery] = useState("");
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [draft, setDraft] = useState<ManagedApp | null>(null);
  const [killTarget, setKillTarget] = useState<ListenerInfo | null>(null);

  const refresh = useCallback(async (silent = false) => {
    if (!silent) setLoading(true);
    try {
      const [nextListeners, nextApps, nextRuntimes] = await Promise.all([
        getListeners(),
        getManagedApps(),
        getManagedRuntimes(),
      ]);
      setListeners(nextListeners);
      setApps(nextApps);
      setRuntimes(nextRuntimes);
      setError(null);
    } catch (refreshError) {
      setError(messageOf(refreshError));
    } finally {
      if (!silent) setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
    const timer = window.setInterval(() => void refresh(true), 4000);
    return () => window.clearInterval(timer);
  }, [refresh]);

  const managedRows = useMemo<ManagedRow[]>(() => {
    const runtimeByApp = new Map(runtimes.map((runtime) => [runtime.appId, runtime]));
    const listenerByPort = new Map<number, ListenerInfo>();
    for (const listener of listeners) {
      if (!listenerByPort.has(listener.port)) listenerByPort.set(listener.port, listener);
    }

    return apps.map((app) => {
      const runtime = runtimeByApp.get(app.id);
      const listener = listenerByPort.get(app.port);
      const status: ManagedStatus = runtime ? "running" : listener ? "occupied" : "stopped";
      return { ...app, runtime, listener, status };
    });
  }, [apps, listeners, runtimes]);

  const filteredListeners = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return listeners;
    return listeners.filter((listener) =>
      [listener.port, listener.pid, listener.processName, listener.localAddress, listener.protocol]
        .join(" ")
        .toLowerCase()
        .includes(needle),
    );
  }, [listeners, query]);

  const runningCount = managedRows.filter((row) => row.status === "running").length;

  const perform = async (key: string, action: () => Promise<unknown>) => {
    setBusy(key);
    setError(null);
    try {
      await action();
      await refresh(true);
    } catch (actionError) {
      setError(messageOf(actionError));
    } finally {
      setBusy(null);
    }
  };

  const saveDraft = async (event: FormEvent) => {
    event.preventDefault();
    if (!draft) return;
    await perform(`save:${draft.id}`, async () => {
      await saveManagedApp(draft);
      setDraft(null);
    });
  };

  const deleteApp = async (app: ManagedApp) => {
    if (!window.confirm(`Remove ${app.name} from Port Lens?`)) return;
    await perform(`remove:${app.id}`, () => removeManagedApp(app.id));
  };

  const confirmKill = async () => {
    if (!killTarget) return;
    const target = killTarget;
    setKillTarget(null);
    await perform(`kill:${target.pid}`, () => killListenerProcess(target.pid, target.port));
  };

  return (
    <main className="app-shell">
      <header className="topbar">
        <div>
          <div className="eyebrow">LOCAL DEVELOPMENT</div>
          <h1>Port Lens</h1>
          <p>See what is listening, then start or stop the services you actually manage.</p>
        </div>
        <div className="topbar-actions">
          <div className="summary-pill">
            <span className="status-dot running" />
            {runningCount} managed running
          </div>
          <div className="summary-pill">{listeners.length} listeners</div>
          <button className="secondary-button" onClick={() => void refresh()} disabled={loading}>
            {loading ? "Refreshing…" : "Refresh"}
          </button>
        </div>
      </header>

      {error && (
        <div className="error-banner" role="alert">
          <span>{error}</span>
          <button onClick={() => setError(null)}>Dismiss</button>
        </div>
      )}

      <section className="panel managed-panel">
        <div className="section-header">
          <div>
            <h2>Managed apps</h2>
            <p>Start commands you trust. Port Lens only stops processes it started in this session.</p>
          </div>
          <button className="primary-button" onClick={() => setDraft(createDraft())}>
            Add app
          </button>
        </div>

        {managedRows.length === 0 ? (
          <button className="empty-state" onClick={() => setDraft(createDraft())}>
            <strong>No managed apps yet</strong>
            <span>Add a dev server to get one-click Start / Stop / Restart.</span>
          </button>
        ) : (
          <div className="managed-grid">
            {managedRows.map((app) => {
              const appBusy = busy?.includes(app.id) ?? false;
              const stateLabel = app.status === "running"
                ? app.listener ? "Running" : "Starting"
                : app.status === "occupied" ? "Port occupied" : "Stopped";
              return (
                <article className={`managed-card ${app.status}`} key={app.id}>
                  <div className="managed-card-head">
                    <div>
                      <div className="app-title-row">
                        <span className={`status-dot ${app.status}`} />
                        <h3>{app.name}</h3>
                      </div>
                      <span className={`status-label ${app.status}`}>{stateLabel}</span>
                    </div>
                    <button className="icon-button" aria-label={`Edit ${app.name}`} onClick={() => setDraft({ ...app })}>
                      Edit
                    </button>
                  </div>
                  <div className="app-meta">
                    <div><span>Port</span><strong>{app.port}</strong></div>
                    <div><span>Process</span><strong>{app.listener?.processName ?? "—"}</strong></div>
                    <div><span>PID</span><strong>{app.listener?.pid ?? app.runtime?.rootPid ?? "—"}</strong></div>
                  </div>
                  <div className="command-line" title={app.command}>{app.command}</div>
                  <div className="cwd-line" title={app.cwd}>{app.cwd}</div>

                  {app.status === "occupied" && app.listener && (
                    <div className="conflict-note">
                      Port {app.port} is already held by {app.listener.processName} (PID {app.listener.pid}).
                    </div>
                  )}

                  <div className="card-actions">
                    {app.status === "running" ? (
                      <>
                        <button disabled={appBusy} onClick={() => void perform(`restart:${app.id}`, () => restartManagedApp(app.id))}>
                          Restart
                        </button>
                        <button className="danger-soft" disabled={appBusy} onClick={() => void perform(`stop:${app.id}`, () => stopManagedApp(app.id))}>
                          Stop
                        </button>
                      </>
                    ) : (
                      <button className="primary-button" disabled={appBusy || app.status === "occupied"} onClick={() => void perform(`start:${app.id}`, () => startManagedApp(app.id))}>
                        Start
                      </button>
                    )}
                    <button disabled={!app.listener} onClick={() => void openUrl(`http://localhost:${app.port}`)}>
                      Open
                    </button>
                    <button className="ghost-button" disabled={app.status === "running"} onClick={() => void deleteApp(app)}>
                      Remove
                    </button>
                  </div>
                </article>
              );
            })}
          </div>
        )}
      </section>

      <section className="panel listeners-panel">
        <div className="section-header listeners-heading">
          <div>
            <h2>Listening ports</h2>
            <p>Active TCP listeners discovered directly from the operating system.</p>
          </div>
          <label className="search-box">
            <span>Search</span>
            <input
              value={query}
              onChange={(event) => setQuery(event.currentTarget.value)}
              placeholder="Port, PID, process, address…"
            />
          </label>
        </div>

        <div className="table-wrap">
          <table>
            <thead>
              <tr>
                <th>Port</th>
                <th>Process</th>
                <th>PID</th>
                <th>Bind</th>
                <th>Protocol</th>
                <th className="action-column">Action</th>
              </tr>
            </thead>
            <tbody>
              {filteredListeners.map((listener) => {
                const managedListener = managedRows.some(
                  (app) => app.status === "running" && app.port === listener.port,
                );
                return (
                  <tr key={`${listener.protocol}-${listener.localAddress}-${listener.port}-${listener.pid}`}>
                    <td><span className="port-chip">:{listener.port}</span></td>
                    <td className="process-cell">{listener.processName}</td>
                    <td className="mono-cell">{listener.pid}</td>
                    <td className="mono-cell muted-cell">{listener.localAddress}</td>
                    <td><span className="protocol-chip">{listener.protocol}</span></td>
                    <td className="action-column">
                      <button
                        className="danger-link"
                        disabled={managedListener || busy === `kill:${listener.pid}`}
                        title={managedListener ? "Use the managed app Stop action" : "Terminate this process"}
                        onClick={() => setKillTarget(listener)}
                      >
                        Kill
                      </button>
                    </td>
                  </tr>
                );
              })}
              {!loading && filteredListeners.length === 0 && (
                <tr>
                  <td colSpan={6} className="table-empty">No matching listeners.</td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      </section>

      {draft && (
        <div className="modal-backdrop" onMouseDown={() => setDraft(null)}>
          <form className="modal-card app-editor" onSubmit={saveDraft} onMouseDown={(event) => event.stopPropagation()}>
            <div className="modal-header">
              <div>
                <span className="eyebrow">MANAGED APP</span>
                <h2>{apps.some((app) => app.id === draft.id) ? "Edit app" : "Add app"}</h2>
              </div>
              <button type="button" className="icon-button" onClick={() => setDraft(null)}>Close</button>
            </div>
            <label>
              <span>Name</span>
              <input
                autoFocus
                required
                value={draft.name}
                onChange={(event) => setDraft({ ...draft, name: event.currentTarget.value })}
                placeholder="Knowledge API"
              />
            </label>
            <div className="form-row">
              <label>
                <span>Port</span>
                <input
                  type="number"
                  min={1}
                  max={65535}
                  required
                  value={draft.port}
                  onChange={(event) => setDraft({ ...draft, port: Number(event.currentTarget.value) })}
                />
              </label>
              <label className="grow-label">
                <span>Start command</span>
                <input
                  required
                  value={draft.command}
                  onChange={(event) => setDraft({ ...draft, command: event.currentTarget.value })}
                  placeholder="npm run dev"
                />
              </label>
            </div>
            <label>
              <span>Working directory</span>
              <input
                required
                value={draft.cwd}
                onChange={(event) => setDraft({ ...draft, cwd: event.currentTarget.value })}
                placeholder="C:\\0.Coding\\my-project"
              />
            </label>
            <p className="form-help">
              Port Lens will launch this command in the working directory and keep its root PID for safe Stop / Restart.
            </p>
            <div className="modal-actions">
              <button type="button" onClick={() => setDraft(null)}>Cancel</button>
              <button className="primary-button" type="submit" disabled={busy === `save:${draft.id}`}>
                {busy === `save:${draft.id}` ? "Saving…" : "Save app"}
              </button>
            </div>
          </form>
        </div>
      )}

      {killTarget && (
        <div className="modal-backdrop" onMouseDown={() => setKillTarget(null)}>
          <div className="modal-card confirm-card" onMouseDown={(event) => event.stopPropagation()}>
            <div className="warning-mark">!</div>
            <h2>Terminate process?</h2>
            <p>This will terminate the selected process tree. Unsaved work in that process can be lost.</p>
            <dl className="confirm-details">
              <div><dt>Process</dt><dd>{killTarget.processName}</dd></div>
              <div><dt>PID</dt><dd>{killTarget.pid}</dd></div>
              <div><dt>Port</dt><dd>{killTarget.port}</dd></div>
            </dl>
            <div className="modal-actions">
              <button onClick={() => setKillTarget(null)}>Cancel</button>
              <button className="danger-button" onClick={() => void confirmKill()}>Terminate</button>
            </div>
          </div>
        </div>
      )}
    </main>
  );
}

export default App;
