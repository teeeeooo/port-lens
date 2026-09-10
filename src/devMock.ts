import type { ListenerInfo, ManagedApp, ManagedRuntime } from "./types";

interface MockSnapshot {
  listeners: ListenerInfo[];
  apps: ManagedApp[];
  runtimes: ManagedRuntime[];
}

const requested = new URLSearchParams(window.location.search).get("mock") === "1";
export const isDevMockMode = import.meta.env.DEV && requested;

let statePromise: Promise<MockSnapshot> | null = null;

async function loadState(): Promise<MockSnapshot> {
  if (!isDevMockMode) throw new Error("Port Lens dev mock mode is disabled");
  statePromise ??= fetch("/__port-lens-mock", { cache: "no-store" }).then(async (response) => {
    if (!response.ok) throw new Error(`Mock fixture unavailable (${response.status})`);
    return response.json() as Promise<MockSnapshot>;
  });
  return statePromise;
}

const clone = <T,>(value: T): T => structuredClone(value);

export async function mockGetListeners() {
  return clone((await loadState()).listeners);
}

export async function mockGetApps() {
  return clone((await loadState()).apps);
}

export async function mockGetRuntimes() {
  return clone((await loadState()).runtimes);
}

export async function mockSaveApp(app: ManagedApp) {
  const state = await loadState();
  const index = state.apps.findIndex((item) => item.id === app.id);
  if (index >= 0) state.apps[index] = clone(app);
  else state.apps.push(clone(app));
  return clone(app);
}

export async function mockRemoveApp(appId: string) {
  const state = await loadState();
  state.apps = state.apps.filter((app) => app.id !== appId);
  state.runtimes = state.runtimes.filter((runtime) => runtime.appId !== appId);
}

export async function mockStartApp(appId: string) {
  const state = await loadState();
  const app = state.apps.find((item) => item.id === appId);
  if (!app) throw new Error("Mock managed app not found");
  const runtime = { appId, rootPid: 40000 + state.runtimes.length + 1 };
  state.runtimes = state.runtimes.filter((item) => item.appId !== appId);
  state.runtimes.push(runtime);
  if (!state.listeners.some((listener) => listener.port === app.port)) {
    state.listeners.push({
      protocol: "TCP",
      localAddress: "127.0.0.1",
      port: app.port,
      pid: runtime.rootPid,
      processName: "node",
      commandLine: app.command,
    });
  }
  return clone(runtime);
}

export async function mockStopApp(appId: string) {
  const state = await loadState();
  const app = state.apps.find((item) => item.id === appId);
  state.runtimes = state.runtimes.filter((runtime) => runtime.appId !== appId);
  if (app) state.listeners = state.listeners.filter((listener) => listener.port !== app.port);
}

export async function mockRestartApp(appId: string) {
  await mockStopApp(appId);
  return mockStartApp(appId);
}

export async function mockKillListener(pid: number, port: number) {
  const state = await loadState();
  state.listeners = state.listeners.filter((listener) => !(listener.pid === pid && listener.port === port));
}
