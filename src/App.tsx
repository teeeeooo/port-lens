import { FormEvent, PointerEvent as ReactPointerEvent, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  expandFromBubble,
  getBubbleState,
  getListeners,
  getMonitoredListeners,
  getSettings,
  getManagedApps,
  getManagedExits,
  getManagedRuntimes,
  killListenerProcess,
  minimizeMainWindow,
  moveCompactBubble,
  openLogs,
  openManagedAppLogs,
  removeManagedApp,
  restartManagedApp,
  saveManagedApp,
  startManagedApp,
  stopManagedApp,
  updateSettings,
} from "./api";
import { isDevMockMode } from "./devMock";
import SettingsModal from "./SettingsModal";
import { localizeError, resolveLanguage, t } from "./i18n";
import type { AppSettings, BubbleState, ListenerInfo, ManagedApp, ManagedExitInfo, ManagedRuntime, ManagedStatus } from "./types";
import "./App.css";

type ManagedRow = ManagedApp & {
  status: ManagedStatus;
  runtime?: ManagedRuntime;
  listener?: ListenerInfo;
  lastExit?: ManagedExitInfo;
  launchConfigured: boolean;
  identityChanged: boolean;
  recoveredManaged: boolean;
};

const createDraft = (): ManagedApp => ({
  id: crypto.randomUUID(),
  name: "",
  port: 3000,
  command: "",
  cwd: "",
});

const DEFAULT_SETTINGS: AppSettings = {
  language: "system",
  bubbleScale: 1,
  compactModeEnabled: true,
};

function messageOf(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}

function elapsedSeconds(elapsedMs: number) {
  const seconds = elapsedMs / 1000;
  return seconds < 10 ? seconds.toFixed(1) : Math.round(seconds).toString();
}

function normalizedProcessName(value?: string) {
  return value?.trim().toLowerCase().replace(/\.exe$/, "") ?? "";
}

function normalizedCommandLine(value?: string) {
  return value?.trim().replace(/\s+/g, " ").toLowerCase() ?? "";
}

function pathParts(path: string) {
  return path.split(/[\\/]/).filter(Boolean);
}

function baseName(path: string) {
  const parts = pathParts(path);
  return parts.length > 0 ? parts[parts.length - 1] : path;
}

function projectFromCommand(command: string) {
  const match = command.match(/["']?([^"'\s]+)[\\/]node_modules[\\/]/i);
  return match ? baseName(match[1]) : undefined;
}

function scriptLabel(path: string) {
  const parts = pathParts(path);
  const file = parts[parts.length - 1] ?? path;
  const parent = parts[parts.length - 2];
  const genericParents = new Set(["bin", "dist", "src", "scripts", "lib"]);
  return parent && !genericParents.has(parent.toLowerCase()) ? `${parent} · ${file}` : file;
}

function inferredRuntimeLabel(listener: ListenerInfo) {
  const command = listener.commandLine?.trim();
  if (!command) return undefined;
  const lower = command.toLowerCase();
  const project = projectFromCommand(command);

  if (/(^|[\\/\s])next(?:\.cmd)?(?:\s|$)|next[\\/]dist[\\/]bin[\\/]next/.test(lower)) {
    return project ? `${project} · Next.js` : "Next.js";
  }
  if (/(^|[\\/\s])vite(?:\.cmd)?(?:\s|$)|[\\/]vite[\\/]bin[\\/]vite/.test(lower)) {
    return project ? `${project} · Vite` : "Vite dev server";
  }
  if (/(^|[\\/\s])nuxt(?:\.cmd)?(?:\s|$)/.test(lower)) return "Nuxt";
  if (/(^|[\\/\s])astro(?:\.cmd)?(?:\s|$)/.test(lower)) return "Astro";

  const uvicorn = command.match(/\b(uvicorn|hypercorn)\b\s+([^\s]+)/i);
  if (uvicorn) return `${uvicorn[1]} · ${uvicorn[2]}`;

  const jar = command.match(/\s-jar\s+["']?([^"'\s]+)["']?/i);
  if (jar) return baseName(jar[1]);

  const script = command.match(/["']?([^"'\s]+\.(?:mjs|cjs|js|ts|py))["']?(?:\s|$)/i);
  if (script) return scriptLabel(script[1]);
  return undefined;
}

function App() {
  const mockParams = isDevMockMode ? new URLSearchParams(window.location.search) : null;
  const mockScreen = mockParams?.get("screen");
  const [listeners, setListeners] = useState<ListenerInfo[]>([]);
  const [monitoredListeners, setMonitoredListeners] = useState<ListenerInfo[]>([]);
  const [apps, setApps] = useState<ManagedApp[]>([]);
  const [runtimes, setRuntimes] = useState<ManagedRuntime[]>([]);
  const [exits, setExits] = useState<ManagedExitInfo[]>([]);
  const [query, setQuery] = useState("");
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [draft, setDraft] = useState<ManagedApp | null>(() => mockScreen === "app-editor" ? {
    id: "showcase-app",
    name: "Docs Preview",
    port: 4200,
    command: "npm run dev",
    cwd: "C:\\0.Coding\\docs-preview",
  } : null);
  const [killTarget, setKillTarget] = useState<ListenerInfo | null>(null);
  const [settings, setSettings] = useState<AppSettings>(DEFAULT_SETTINGS);
  const [settingsOpen, setSettingsOpen] = useState(mockScreen === "settings");
  const [bubbleMode, setBubbleMode] = useState(mockParams?.get("bubble") === "1");
  const inventoryRefreshInFlight = useRef<Promise<void> | null>(null);
  const managedRefreshInFlight = useRef<Promise<void> | null>(null);
  const managedStateEpoch = useRef(0);
  const monitoredRefreshInFlight = useRef<Promise<void> | null>(null);
  const bubbleDrag = useRef<{
    pointerId: number;
    startX: number;
    startY: number;
    offsetRatioX: number;
    offsetRatioY: number;
    moved: boolean;
  } | null>(null);
  const uiLanguage = resolveLanguage(settings.language);

  const refreshInventory = useCallback((silent = false) => {
    if (inventoryRefreshInFlight.current) return inventoryRefreshInFlight.current;
    if (!silent) setLoading(true);

    const task = getListeners()
      .then((nextListeners) => {
        setListeners(nextListeners);
        setError(null);
      })
      .catch((refreshError) => setError(messageOf(refreshError)))
      .finally(() => {
        if (!silent) setLoading(false);
        inventoryRefreshInFlight.current = null;
      });

    inventoryRefreshInFlight.current = task;
    return task;
  }, []);

  const refreshManagedState = useCallback((force = false) => {
    if (!force && managedRefreshInFlight.current) return managedRefreshInFlight.current;
    if (force) managedStateEpoch.current += 1;
    const epoch = managedStateEpoch.current;
    const task = Promise.all([getManagedApps(), getManagedRuntimes(), getManagedExits()])
      .then(([nextApps, nextRuntimes, nextExits]) => {
        if (epoch !== managedStateEpoch.current) return;
        setApps(nextApps);
        setRuntimes(nextRuntimes);
        setExits(nextExits);
      })
      .catch((refreshError) => setError(messageOf(refreshError)));

    if (!force) {
      managedRefreshInFlight.current = task;
      void task.finally(() => {
        if (managedRefreshInFlight.current === task) managedRefreshInFlight.current = null;
      });
    }
    return task;
  }, []);

  const refreshMonitored = useCallback(() => {
    if (monitoredRefreshInFlight.current) return monitoredRefreshInFlight.current;
    const task = getMonitoredListeners()
      .then((nextListeners) => setMonitoredListeners(nextListeners))
      .catch((refreshError) => setError(messageOf(refreshError)))
      .finally(() => {
        monitoredRefreshInFlight.current = null;
      });
    monitoredRefreshInFlight.current = task;
    return task;
  }, []);

  const refreshAll = useCallback(async (silent = false, forceManaged = false) => {
    await Promise.all([
      refreshInventory(silent),
      refreshMonitored(),
      refreshManagedState(forceManaged),
    ]);
  }, [refreshInventory, refreshManagedState, refreshMonitored]);

  useEffect(() => {
    let stopped = false;
    let timer: number | undefined;
    const scheduleNext = () => {
      if (stopped) return;
      timer = window.setTimeout(async () => {
        await refreshInventory(true);
        scheduleNext();
      }, 10_000);
    };
    void refreshInventory().finally(scheduleNext);
    return () => {
      stopped = true;
      if (timer !== undefined) window.clearTimeout(timer);
    };
  }, [refreshInventory]);

  useEffect(() => {
    let stopped = false;
    let timer: number | undefined;
    const refreshFastState = () => Promise.all([refreshMonitored(), refreshManagedState()]);
    const scheduleNext = () => {
      if (stopped) return;
      timer = window.setTimeout(async () => {
        await refreshFastState();
        scheduleNext();
      }, 3_000);
    };
    void refreshFastState().finally(scheduleNext);
    return () => {
      stopped = true;
      if (timer !== undefined) window.clearTimeout(timer);
    };
  }, [refreshManagedState, refreshMonitored]);

  useEffect(() => {
    void getSettings()
      .then(setSettings)
      .catch((settingsError) => setError(messageOf(settingsError)));
  }, []);

  useEffect(() => {
    document.documentElement.lang = uiLanguage;
    document.documentElement.style.setProperty("--bubble-scale", String(settings.bubbleScale));
  }, [settings.bubbleScale, uiLanguage]);

  useEffect(() => {
    if (mockScreen !== "terminate" || killTarget || listeners.length === 0) return;
    setKillTarget(listeners.find((listener) => listener.port === 5173) ?? listeners[0]);
  }, [killTarget, listeners, mockScreen]);

  useEffect(() => {
    document.documentElement.classList.toggle("bubble-mode", bubbleMode);
    return () => document.documentElement.classList.remove("bubble-mode");
  }, [bubbleMode]);

  useEffect(() => {
    if (isDevMockMode) return;
    let active = true;
    const cleanups: Array<() => void> = [];

    void getBubbleState()
      .then((state) => active && setBubbleMode(state.collapsed))
      .catch((stateError) => active && setError(messageOf(stateError)));

    void listen<BubbleState>("port-lens://bubble-state", (event) => {
      if (active) setBubbleMode(event.payload.collapsed);
    }).then((unlisten) => cleanups.push(unlisten));

    void listen("port-lens://refresh", () => void refreshAll(true, true)).then((unlisten) => cleanups.push(unlisten));

    return () => {
      active = false;
      cleanups.forEach((cleanup) => cleanup());
    };
  }, [refreshAll]);

  const managedRows = useMemo<ManagedRow[]>(() => {
    const runtimeByApp = new Map(runtimes.map((runtime) => [runtime.appId, runtime]));
    const exitByApp = new Map(exits.map((exit) => [exit.appId, exit]));
    const listenerByPort = new Map<number, ListenerInfo>();
    for (const listener of monitoredListeners) {
      if (!listenerByPort.has(listener.port)) listenerByPort.set(listener.port, listener);
    }

    return apps.map((app) => {
      const runtime = runtimeByApp.get(app.id);
      const listener = listenerByPort.get(app.port);
      const lastExit = exitByApp.get(app.id);
      const launchConfigured = Boolean(app.command?.trim() && app.cwd?.trim());
      const managedProcessMatches = Boolean(
        listener && app.lastManagedProcessName
        && normalizedProcessName(listener.processName) === normalizedProcessName(app.lastManagedProcessName),
      );
      const managedPidMatches = Boolean(
        listener && app.lastManagedPid && listener.pid === app.lastManagedPid,
      );
      const managedCommandMatches = Boolean(
        listener?.commandLine && app.lastManagedCommandLine
        && normalizedCommandLine(listener.commandLine) === normalizedCommandLine(app.lastManagedCommandLine),
      );
      const recoveredManaged = Boolean(
        !runtime && listener && managedProcessMatches && (managedPidMatches || managedCommandMatches),
      );
      const processChanged = Boolean(
        listener && app.lastProcessName
        && normalizedProcessName(listener.processName) !== normalizedProcessName(app.lastProcessName),
      );
      const commandChanged = Boolean(
        listener?.commandLine && app.lastCommandLine
        && normalizedCommandLine(listener.commandLine) !== normalizedCommandLine(app.lastCommandLine),
      );
      const identityChanged = !runtime && !recoveredManaged && (processChanged || commandChanged);
      const status: ManagedStatus = runtime
        ? listener ? "running" : "starting"
        : identityChanged
          ? "changed"
          : listener ? "online" : "offline";
      return {
        ...app,
        runtime,
        listener,
        lastExit,
        launchConfigured,
        identityChanged,
        recoveredManaged,
        status,
      };
    });
  }, [apps, exits, monitoredListeners, runtimes]);

  const presentationFor = useCallback((listener: ListenerInfo) => {
    const managed = managedRows.find(
      (app) => !app.identityChanged && app.port === listener.port && app.listener?.pid === listener.pid,
    );
    if (managed) return { primary: managed.name, secondary: listener.processName, kind: "managed" as const };

    const inferred = inferredRuntimeLabel(listener);
    if (inferred && inferred.toLowerCase() !== listener.processName.toLowerCase()) {
      return { primary: inferred, secondary: listener.processName, kind: "inferred" as const };
    }
    return { primary: listener.processName, secondary: undefined, kind: "process" as const };
  }, [managedRows]);

  const filteredListeners = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return listeners;
    return listeners.filter((listener) => {
      const presentation = presentationFor(listener);
      return [
        listener.port,
        listener.pid,
        listener.processName,
        listener.commandLine,
        presentation.primary,
        listener.localAddress,
        listener.protocol,
      ].join(" ").toLowerCase().includes(needle);
    });
  }, [listeners, presentationFor, query]);

  const runningCount = managedRows.filter((row) => row.status === "running" || row.status === "starting").length;
  const onlineAppCount = managedRows.filter((row) => row.listener).length;

  const savePreferences = async (patch: Partial<AppSettings>) => {
    try {
      const next = await updateSettings(patch);
      setSettings(next);
    } catch (settingsError) {
      setError(messageOf(settingsError));
    }
  };

  const openDiagnosticLogs = async () => {
    try {
      await openLogs();
    } catch (logError) {
      setError(messageOf(logError));
    }
  };

  const openAppLogs = async (appId: string) => {
    try {
      await openManagedAppLogs(appId);
    } catch (logError) {
      setError(messageOf(logError));
    }
  };

  const minimizeWindow = async () => {
    try {
      const state = await minimizeMainWindow();
      setBubbleMode(state.collapsed);
    } catch (bubbleError) {
      setError(messageOf(bubbleError));
    }
  };

  const expandBubble = async () => {
    try {
      const state = await expandFromBubble();
      setBubbleMode(state.collapsed);
    } catch (bubbleError) {
      setError(messageOf(bubbleError));
    }
  };

  const beginBubbleDrag = (event: ReactPointerEvent<HTMLElement>) => {
    if (event.button !== 0 || (event.target as Element).closest("button")) return;
    const rect = event.currentTarget.getBoundingClientRect();
    bubbleDrag.current = {
      pointerId: event.pointerId,
      startX: event.screenX,
      startY: event.screenY,
      offsetRatioX: Math.max(0, Math.min(1, (event.clientX - rect.left) / (rect.width || 1))),
      offsetRatioY: Math.max(0, Math.min(1, (event.clientY - rect.top) / (rect.height || 1))),
      moved: false,
    };
    event.currentTarget.setPointerCapture(event.pointerId);
    event.preventDefault();
  };

  const moveBubbleDrag = (event: ReactPointerEvent<HTMLElement>) => {
    const drag = bubbleDrag.current;
    if (!drag || drag.pointerId !== event.pointerId) return;
    if (!drag.moved && Math.hypot(event.screenX - drag.startX, event.screenY - drag.startY) < 4) return;
    drag.moved = true;
    event.currentTarget.classList.add("dragging");
    void moveCompactBubble(drag.offsetRatioX, drag.offsetRatioY)
      .then((state) => setBubbleMode(state.collapsed))
      .catch((bubbleError) => setError(messageOf(bubbleError)));
    event.preventDefault();
  };

  const finishBubbleDrag = (event: ReactPointerEvent<HTMLElement>) => {
    const drag = bubbleDrag.current;
    if (!drag || drag.pointerId !== event.pointerId) return;
    bubbleDrag.current = null;
    event.currentTarget.classList.remove("dragging");
    try { event.currentTarget.releasePointerCapture(event.pointerId); } catch { /* already released */ }
    if (drag.moved) {
      void moveCompactBubble(drag.offsetRatioX, drag.offsetRatioY, true)
        .then((state) => setBubbleMode(state.collapsed))
        .catch((bubbleError) => setError(messageOf(bubbleError)));
    } else {
      void expandBubble();
    }
    event.preventDefault();
  };

  const cancelBubbleDrag = (event: ReactPointerEvent<HTMLElement>) => {
    const drag = bubbleDrag.current;
    if (!drag || drag.pointerId !== event.pointerId) return;
    bubbleDrag.current = null;
    event.currentTarget.classList.remove("dragging");
    try { event.currentTarget.releasePointerCapture(event.pointerId); } catch { /* already released */ }
    if (drag.moved) {
      void moveCompactBubble(drag.offsetRatioX, drag.offsetRatioY, true)
        .then((state) => setBubbleMode(state.collapsed))
        .catch((bubbleError) => setError(messageOf(bubbleError)));
    }
  };

  const perform = async (
    key: string,
    action: () => Promise<unknown>,
    managedMutation = false,
  ) => {
    setBusy(key);
    setError(null);
    if (managedMutation) managedStateEpoch.current += 1;
    try {
      await action();
      await refreshAll(true, managedMutation);
    } catch (actionError) {
      setError(messageOf(actionError));
      if (managedMutation) await refreshManagedState(true);
    } finally {
      setBusy(null);
    }
  };

  const registerListener = async (listener: ListenerInfo) => {
    const presentation = presentationFor(listener);
    const app: ManagedApp = {
      id: crypto.randomUUID(),
      name: presentation.primary || `${listener.processName} :${listener.port}`,
      port: listener.port,
      lastProcessName: listener.processName,
      lastCommandLine: listener.commandLine,
    };
    await perform(`register:${listener.port}`, async () => {
      const saved = await saveManagedApp(app);
      setApps((current) => [...current.filter((item) => item.id !== saved.id), saved]);
    }, true);
  };

  const browseWorkingDirectory = async () => {
    if (!draft) return;
    try {
      const selected = await openDialog({
        directory: true,
        multiple: false,
        defaultPath: draft.cwd || undefined,
        title: t(uiLanguage, "selectWorkingDirectory"),
      });
      if (typeof selected === "string") {
        setDraft((current) => current ? { ...current, cwd: selected } : current);
      }
    } catch (dialogError) {
      setError(messageOf(dialogError));
    }
  };

  const saveDraft = async (event: FormEvent) => {
    event.preventDefault();
    if (!draft) return;
    const command = draft.command?.trim() ?? "";
    const cwd = draft.cwd?.trim() ?? "";
    if (Boolean(command) !== Boolean(cwd)) {
      setError("Start command and working directory must be configured together.");
      return;
    }
    await perform(`save:${draft.id}`, async () => {
      const saved = await saveManagedApp({ ...draft, command: command || undefined, cwd: cwd || undefined });
      setApps((current) => current.map((item) => item.id === saved.id ? saved : item));
      setDraft(null);
    }, true);
  };

  const deleteApp = async (app: ManagedApp) => {
    if (!window.confirm(t(uiLanguage, "removeConfirm", { name: app.name }))) return;
    await perform(`remove:${app.id}`, async () => {
      await removeManagedApp(app.id);
      setApps((current) => current.filter((item) => item.id !== app.id));
      setExits((current) => current.filter((item) => item.appId !== app.id));
    }, true);
  };

  const confirmKill = async () => {
    if (!killTarget) return;
    const target = killTarget;
    setKillTarget(null);
    await perform(`kill:${target.pid}`, () => killListenerProcess(target.pid, target.port));
  };

  if (bubbleMode) {
    return (
      <main
        className="bubble-shell"
        aria-label="Port Lens compact monitor"
        title="Drag to move · Click to open"
        onPointerDown={beginBubbleDrag}
        onPointerMove={moveBubbleDrag}
        onPointerUp={finishBubbleDrag}
        onPointerCancel={cancelBubbleDrag}
      >
        <div className="bubble-grip" aria-hidden="true">
          <img src="/port-lens.svg" alt="" />
        </div>
        <div className="bubble-summary">
          <span className={`status-dot ${onlineAppCount > 0 ? "running" : "stopped"}`} />
          <strong>{onlineAppCount}/{apps.length}</strong>
          <span>apps</span>
          <span className="bubble-divider" />
          <strong>{listeners.length}</strong>
          <span>ports</span>
        </div>
        <button className="bubble-expand" onClick={() => void expandBubble()} aria-label="Expand Port Lens" title="Expand">
          Open
        </button>
      </main>
    );
  }

  return (
    <main className="app-shell">
      <header className="topbar">
        <div className="brand-lockup">
          <img className="brand-mark" src="/port-lens.svg" alt="" aria-hidden="true" />
          <div>
            <div className="eyebrow">LOCAL DEVELOPMENT</div>
            <h1>Port Lens</h1>
            <p>{t(uiLanguage, "headerDescription")}</p>
          </div>
        </div>
        <div className="topbar-actions">
          <div className="summary-pill">
            <span className="status-dot running" />
            {runningCount} running
          </div>
          <div className="summary-pill">{listeners.length} listeners</div>
          <div className="summary-pill">{onlineAppCount}/{apps.length} apps online</div>
          <button className="secondary-button" onClick={() => void minimizeWindow()}>
            Minimize
          </button>
          <button className="secondary-button" onClick={() => setSettingsOpen(true)}>
            Settings
          </button>
          <button className="secondary-button" onClick={() => void refreshAll()} disabled={loading}>
            {loading ? "Refreshing…" : "Refresh"}
          </button>
        </div>
      </header>

      {error && (
        <div className="error-banner" role="alert">
          <span>{localizeError(uiLanguage, error)}</span>
          <button onClick={() => setError(null)}>Dismiss</button>
        </div>
      )}

      <section className="panel managed-panel">
        <div className="section-header">
          <div>
            <h2>Apps</h2>
            <p>{t(uiLanguage, "managedDescription")}</p>
          </div>
          <button className="primary-button" onClick={() => setDraft(createDraft())}>
            Add app
          </button>
        </div>

        {managedRows.length === 0 ? (
          <div className="monitor-empty">
            <strong>{t(uiLanguage, "emptyTitle")}</strong>
            <span>{t(uiLanguage, "emptyDescription")}</span>
          </div>
        ) : (
          <div className="managed-grid">
            {managedRows.map((app) => {
              const appBusy = busy?.includes(app.id) ?? false;
              const stateLabel = app.status === "running"
                ? "Running"
                : app.status === "starting"
                  ? "Starting"
                  : app.status === "online"
                    ? "Online"
                    : app.status === "changed"
                      ? "Different process"
                      : "Offline";
              const canStart = app.launchConfigured && app.status === "offline" && !app.runtime;
              const canRestart = app.launchConfigured && Boolean(app.runtime);
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
                    <button
                      className="icon-button"
                      aria-label={`Edit ${app.name}`}
                      disabled={Boolean(app.runtime)}
                      title={app.runtime ? "Stop this App before editing its launch settings" : "Edit App"}
                      onClick={() => setDraft({ ...app })}
                    >
                      Edit
                    </button>
                  </div>
                  <div className="app-meta">
                    <div><span>Port</span><strong>{app.port}</strong></div>
                    <div><span>Process</span><strong>{app.listener?.processName ?? app.lastProcessName ?? "—"}</strong></div>
                    <div><span>PID</span><strong>{app.listener?.pid ?? app.runtime?.rootPid ?? "—"}</strong></div>
                  </div>
                  {app.launchConfigured ? (
                    <>
                      <div className="command-line" title={app.command}>{app.command}</div>
                      <div className="cwd-line" title={app.cwd}>{app.cwd}</div>
                    </>
                  ) : (
                    <div className="launch-note">{t(uiLanguage, "monitoringOnlyNote")}</div>
                  )}

                  {app.identityChanged && app.listener && (
                    <div className="conflict-note">{t(uiLanguage, "differentProcessWarning")}</div>
                  )}

                  {app.recoveredManaged && (
                    <div className="managed-origin-note">{t(uiLanguage, "previouslyManagedNote")}</div>
                  )}

                  {app.lastExit && !app.runtime && (
                    <div className={`exit-note ${app.lastExit.earlyExit ? "early" : ""}`}>
                      {t(uiLanguage, app.lastExit.earlyExit ? "earlyExitNotice" : "lastExitNotice", {
                        code: app.lastExit.exitCode ?? "?",
                        seconds: elapsedSeconds(app.lastExit.elapsedMs),
                      })}
                    </div>
                  )}

                  <div className="card-actions">
                    {app.runtime ? (
                      <>
                        <button disabled={appBusy || !canRestart} onClick={() => void perform(`restart:${app.id}`, () => restartManagedApp(app.id), true)}>
                          Restart
                        </button>
                        <button className="danger-soft" disabled={appBusy} onClick={() => void perform(`stop:${app.id}`, () => stopManagedApp(app.id), true)}>
                          Stop
                        </button>
                      </>
                    ) : (
                      <button
                        className="primary-button"
                        disabled={appBusy || !canStart}
                        title={!app.launchConfigured ? "Configure launch settings in Edit" : app.listener ? "Port is already online" : "Start app"}
                        onClick={() => void perform(`start:${app.id}`, () => startManagedApp(app.id), true)}
                      >
                        Start
                      </button>
                    )}
                    <button disabled={!app.listener} onClick={() => void openUrl(`http://localhost:${app.port}`)}>
                      Open
                    </button>
                    <button
                      title="Open captured stdout/stderr logs. Output is captured when Port Lens starts this App."
                      onClick={() => void openAppLogs(app.id)}
                    >
                      Logs
                    </button>
                    <button className="ghost-button" disabled={Boolean(app.runtime)} onClick={() => void deleteApp(app)}>
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
            <p>{t(uiLanguage, "listenersDescription")}</p>
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
                <th>App / Process</th>
                <th>PID</th>
                <th>Bind</th>
                <th>Protocol</th>
                <th className="action-column">Action</th>
              </tr>
            </thead>
            <tbody>
              {filteredListeners.map((listener) => {
                const presentation = presentationFor(listener);
                const registered = apps.some((app) => app.port === listener.port);
                const portLensRuntime = managedRows.some(
                  (app) => Boolean(app.runtime) && app.port === listener.port,
                );
                return (
                  <tr key={`${listener.protocol}-${listener.localAddress}-${listener.port}-${listener.pid}`}>
                    <td><span className="port-chip">:{listener.port}</span></td>
                    <td className="process-cell" title={listener.commandLine ?? listener.processName}>
                      <span className="process-primary">{presentation.primary}</span>
                      {presentation.secondary && (
                        <span className="process-secondary">{presentation.secondary}</span>
                      )}
                    </td>
                    <td className="mono-cell">{listener.pid}</td>
                    <td className="mono-cell muted-cell">{listener.localAddress}</td>
                    <td><span className="protocol-chip">{listener.protocol}</span></td>
                    <td className="action-column"><div className="action-group">
                      <button
                        className={registered ? "monitor-button active" : "monitor-button"}
                        disabled={registered || busy === `register:${listener.port}`}
                        onClick={() => void registerListener(listener)}
                      >
                        {registered ? "Registered" : "Register"}
                      </button>
                      <button
                        className="danger-link"
                        disabled={portLensRuntime || busy === `kill:${listener.pid}`}
                        title={portLensRuntime ? "Use the App Stop action" : "Terminate this process"}
                        onClick={() => setKillTarget(listener)}
                      >
                        Kill
                      </button>
                    </div></td>
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
                <span className="eyebrow">APP</span>
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
                <span>Start command <em>optional</em></span>
                <input
                  value={draft.command ?? ""}
                  onChange={(event) => setDraft({ ...draft, command: event.currentTarget.value })}
                  placeholder="npm run dev"
                />
              </label>
            </div>
            <label>
              <span>Working directory <em>optional</em></span>
              <div className="directory-field">
                <input
                  value={draft.cwd ?? ""}
                  onChange={(event) => setDraft({ ...draft, cwd: event.currentTarget.value })}
                  placeholder="C:\\0.Coding\\my-project"
                />
                <button type="button" onClick={() => void browseWorkingDirectory()}>Browse…</button>
              </div>
            </label>
            <p className="form-help">{t(uiLanguage, "formHelp")}</p>
            <div className="modal-actions">
              <button type="button" onClick={() => setDraft(null)}>Cancel</button>
              <button className="primary-button" type="submit" disabled={busy === `save:${draft.id}`}>
                {busy === `save:${draft.id}` ? "Saving…" : "Save app"}
              </button>
            </div>
          </form>
        </div>
      )}

      {settingsOpen && (
        <SettingsModal
          settings={settings}
          language={uiLanguage}
          onChange={savePreferences}
          onOpenLogs={openDiagnosticLogs}
          onClose={() => setSettingsOpen(false)}
        />
      )}

      {killTarget && (
        <div className="modal-backdrop" onMouseDown={() => setKillTarget(null)}>
          <div className="modal-card confirm-card" onMouseDown={(event) => event.stopPropagation()}>
            <div className="warning-mark">!</div>
            <h2>{t(uiLanguage, "terminateTitle")}</h2>
            <p>{t(uiLanguage, "terminateWarning")}</p>
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
