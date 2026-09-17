import { act, StrictMode } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const bus = vi.hoisted(() => ({ handlers: new Map<string, Set<(event: any) => void>>() }));
const api = vi.hoisted(() => Object.fromEntries([
  "expandFromBubble", "getBubbleState", "getListeners", "getMonitoredListeners", "getSettings",
  "getManagedApps", "getManagedExits", "getManagedRuntimes", "killListenerProcess",
  "minimizeMainWindow", "moveCompactBubble", "showCompactHover", "hideCompactHover",
  "openLogs", "openManagedAppLogs", "removeManagedApp", "restartManagedApp", "saveManagedApp",
  "startManagedApp", "stopManagedApp", "updateSettings",
].map(name => [name, vi.fn()])));
const events = vi.hoisted(() => ({ listen: vi.fn(), emitTo: vi.fn() }));
vi.mock("../src/api", () => api);
vi.mock("../src/devMock", () => ({ isDevMockMode: false }));
vi.mock("@tauri-apps/api/event", () => events);
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn() }));
import App from "../src/App";
import CompactHover from "../src/CompactHover";

function deferred<T = void>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
function send(name: string, payload: unknown) {
  bus.handlers.get(name)?.forEach(handler => handler({ payload }));
}
let host: HTMLDivElement;
let root: Root;
async function mount(component = <App />) {
  await act(async () => root.render(component));
}
async function tick(ms: number) {
  await act(async () => { await vi.advanceTimersByTimeAsync(ms); });
}
async function pointer(type: string, id = 1, x = 10) {
  const element = host.querySelector(".bubble-bar")!;
  expect(element).not.toBeNull();
  await act(async () => {
    const event = new MouseEvent(type, { bubbles: true, button: 0, buttons: type === "pointerup" ? 0 : 1, clientX: x, clientY: 10, screenX: x, screenY: 10 });
    Object.defineProperty(event, "pointerId", { value: id });
    element.dispatchEvent(event);
  });
}
async function open() {
  await act(async () => (host.querySelector(".bubble-expand") as HTMLButtonElement).click());
}

beforeEach(() => {
  vi.useFakeTimers();
  vi.resetAllMocks();
  bus.handlers.clear();
  (globalThis as any).IS_REACT_ACT_ENVIRONMENT = true;
  HTMLElement.prototype.setPointerCapture = vi.fn();
  HTMLElement.prototype.releasePointerCapture = vi.fn();
  HTMLElement.prototype.getBoundingClientRect = () => ({ left: 0, top: 0, right: 276, bottom: 46, width: 276, height: 46, x: 0, y: 0, toJSON() {} });
  for (const fn of Object.values(api)) fn.mockResolvedValue(undefined);
  api.getListeners.mockResolvedValue([]);
  api.getMonitoredListeners.mockResolvedValue([]);
  api.getManagedApps.mockResolvedValue([{ id: "app", name: "Test App", port: 3000 }]);
  api.getManagedRuntimes.mockResolvedValue([]);
  api.getManagedExits.mockResolvedValue([]);
  api.getSettings.mockResolvedValue({ language: "en", bubbleScale: 1, compactModeEnabled: true });
  api.getBubbleState.mockResolvedValue({ collapsed: true });
  api.expandFromBubble.mockResolvedValue({ collapsed: false });
  events.listen.mockImplementation(async (name, handler) => {
    let handlers = bus.handlers.get(name);
    if (!handlers) bus.handlers.set(name, handlers = new Set());
    handlers.add(handler);
    return () => handlers!.delete(handler);
  });
  events.emitTo.mockImplementation(async (_target, name, payload) => {
    if (name === "port-lens://compact-hover-data" && payload.revision > 0) {
      send("port-lens://compact-hover-rendered", { revision: payload.revision });
    }
  });
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
});
afterEach(async () => {
  await act(async () => root.unmount());
  host.remove();
  vi.useRealTimers();
});

describe("compact gesture and restore lifecycle", () => {
  it("StrictMode effect replay refreshes snapshots invalidated by cleanup", async () => {
    api.getListeners.mockResolvedValue([{ port: 3000, pid: 5, processName: "node" }]);
    await mount(<StrictMode><App /></StrictMode>);
    expect(host.querySelector(".bubble-summary")?.textContent).toContain("1ports");
  });

  it("repeated cancellation waits for the same in-flight move before resuming", async () => {
    const movement = deferred();
    api.moveCompactBubble.mockImplementationOnce(() => movement.promise);
    await mount();
    await pointer("pointerdown");
    await pointer("pointermove", 1, 20);
    const polls = api.getMonitoredListeners.mock.calls.length;
    await act(async () => window.dispatchEvent(new Event("blur")));
    await act(async () => send("port-lens://bubble-state", { collapsed: false }));
    await tick(3100);
    expect(api.getMonitoredListeners.mock.calls.length).toBe(polls);
    await act(async () => movement.resolve());
    expect(api.getMonitoredListeners.mock.calls.length).toBeGreaterThan(polls);
  });

  it.each(["pointercancel", "lostpointercapture"])("%s cancels without a final cursor-following move", async type => {
    await mount();
    await pointer("pointerdown");
    await pointer("pointermove", 1, 20);
    const calls = api.moveCompactBubble.mock.calls.length;
    await pointer(type, 1, 300);
    expect(api.moveCompactBubble).toHaveBeenCalledTimes(calls);
  });

  it("an old drag completion cannot unpause a newer drag", async () => {
    await mount();
    const oldFinish = deferred();
    api.moveCompactBubble.mockResolvedValueOnce(undefined).mockImplementationOnce(() => oldFinish.promise);
    await pointer("pointerdown");
    await pointer("pointermove", 1, 20);
    await pointer("pointerup", 1, 20);
    await pointer("pointerdown", 2);
    await pointer("pointermove", 2, 20);
    const calls = api.getMonitoredListeners.mock.calls.length;
    await act(async () => oldFinish.resolve());
    await tick(3100);
    expect(api.getMonitoredListeners).toHaveBeenCalledTimes(calls);
    await pointer("pointerup", 2, 20);
    expect(api.getMonitoredListeners.mock.calls.length).toBeGreaterThan(calls);
  });

  it("settles foreground loading after a refresh completes during drag", async () => {
    const inventory = deferred<any[]>();
    api.getListeners.mockImplementationOnce(() => inventory.promise);
    await mount();
    await pointer("pointerdown");
    await pointer("pointermove", 1, 20);
    await act(async () => inventory.resolve([]));
    await pointer("pointerup", 1, 20);
    await open();
    const refresh = [...host.querySelectorAll("button")].find(b => /Refresh/.test(b.textContent ?? ""));
    expect(refresh?.textContent).toBe("Refresh");
    expect(refresh?.disabled).toBe(false);
  });

  it("a slow full inventory does not block the post-drag monitored catch-up", async () => {
    const inventory = deferred<any[]>();
    api.getListeners.mockImplementationOnce(() => inventory.promise);
    await mount();
    await pointer("pointerdown");
    await pointer("pointermove", 1, 20);
    const calls = api.getMonitoredListeners.mock.calls.length;
    await pointer("pointerup", 1, 20);
    expect(api.getMonitoredListeners.mock.calls.length).toBeGreaterThan(calls);
    await act(async () => inventory.resolve([]));
  });

  it("a native/tray Open event cancels capture and resumes monitoring", async () => {
    await mount();
    await pointer("pointerdown");
    await pointer("pointermove", 1, 20);
    const calls = api.getMonitoredListeners.mock.calls.length;
    await act(async () => send("port-lens://bubble-state", { collapsed: false }));
    await tick(3100);
    expect(host.querySelector(".bubble-bar")).toBeNull();
    expect(api.getMonitoredListeners.mock.calls.length).toBeGreaterThan(calls);
  });

  it("window blur cancels drag without another cursor sample", async () => {
    await mount();
    await pointer("pointerdown");
    await pointer("pointermove", 1, 20);
    const moves = api.moveCompactBubble.mock.calls.length;
    const polls = api.getMonitoredListeners.mock.calls.length;
    await act(async () => window.dispatchEvent(new Event("blur")));
    await tick(3100);
    expect(api.moveCompactBubble).toHaveBeenCalledTimes(moves);
    expect(api.getMonitoredListeners.mock.calls.length).toBeGreaterThan(polls);
  });

  it("a stale initial bubble snapshot cannot override a newer mode event", async () => {
    const initial = deferred<{ collapsed: boolean }>();
    api.getBubbleState.mockImplementationOnce(() => initial.promise);
    await mount();
    await act(async () => send("port-lens://bubble-state", { collapsed: true }));
    await act(async () => initial.resolve({ collapsed: false }));
    expect(host.querySelector(".bubble-bar")).not.toBeNull();
  });

  it("the first move hides even an in-flight, not-yet-acknowledged hover show", async () => {
    const shown = deferred();
    api.showCompactHover.mockImplementationOnce(() => shown.promise);
    await mount();
    await act(async () => {
      send("port-lens://compact-hover-ready", null);
      host.querySelector(".bubble-shell")!.dispatchEvent(new MouseEvent("mouseover", { bubbles: true }));
    });
    await tick(250);
    expect(api.showCompactHover).toHaveBeenCalledTimes(1);
    await pointer("pointerdown");
    await pointer("pointermove", 1, 20);
    expect(api.moveCompactBubble).toHaveBeenLastCalledWith(expect.any(Number), expect.any(Number), true);
    await act(async () => shown.resolve());
  });

  it("hover-closed movement keeps the native hide path out of the hot path", async () => {
    await mount();
    await pointer("pointerdown");
    await pointer("pointermove", 1, 20);
    expect(api.moveCompactBubble).toHaveBeenLastCalledWith(expect.any(Number), expect.any(Number), false);
  });

  it("a sub-threshold click does not move or suspend idle polling", async () => {
    await mount();
    await pointer("pointerdown");
    await pointer("pointermove", 1, 12);
    const polls = api.getMonitoredListeners.mock.calls.length;
    await tick(3100);
    expect(api.moveCompactBubble).not.toHaveBeenCalled();
    expect(api.getMonitoredListeners.mock.calls.length).toBeGreaterThan(polls);
    await pointer("pointerup", 1, 12);
  });

  it("a rejected move still releases the polling gate on pointerup", async () => {
    await mount();
    api.moveCompactBubble.mockRejectedValueOnce(new Error("native move failed"));
    await pointer("pointerdown");
    await pointer("pointermove", 1, 20);
    const polls = api.getMonitoredListeners.mock.calls.length;
    await pointer("pointerup", 1, 20);
    expect(api.getMonitoredListeners.mock.calls.length).toBeGreaterThan(polls);
  });

  it("Open is single-flight while the native restore is pending", async () => {
    const restored = deferred<{ collapsed: boolean }>();
    api.expandFromBubble.mockImplementationOnce(() => restored.promise);
    await mount();
    await open();
    await open();
    expect(api.expandFromBubble).toHaveBeenCalledTimes(1);
    await act(async () => restored.resolve({ collapsed: false }));
  });

  it("a failed Open releases its frontend guard for a retry", async () => {
    api.expandFromBubble.mockRejectedValueOnce(new Error("restore failed"));
    await mount();
    await open();
    expect(host.querySelector(".bubble-bar")).not.toBeNull();
    await open();
    expect(api.expandFromBubble).toHaveBeenCalledTimes(2);
    expect(host.querySelector(".bubble-bar")).toBeNull();
  });

  it("bounds invoke traffic during a pointermove burst and drops pending work on cancel", async () => {
    const movement = deferred();
    api.moveCompactBubble.mockImplementationOnce(() => movement.promise);
    await mount();
    await pointer("pointerdown");
    for (let x = 20; x < 120; x++) await pointer("pointermove", 1, x);
    expect(api.moveCompactBubble).toHaveBeenCalledTimes(1);
    await pointer("pointercancel", 1, 400);
    await act(async () => movement.resolve());
    expect(api.moveCompactBubble).toHaveBeenCalledTimes(1);
  });

  it("pre-drag inventory data resolving after release is discarded until catch-up", async () => {
    const stale = deferred<any[]>();
    const fresh = deferred<any[]>();
    api.getListeners.mockImplementationOnce(() => stale.promise).mockImplementationOnce(() => fresh.promise);
    await mount();
    await pointer("pointerdown");
    await pointer("pointermove", 1, 20);
    await pointer("pointerup", 1, 20);
    await act(async () => stale.resolve([{ port: 9000, pid: 5, processName: "stale" }]));
    expect(host.querySelector(".bubble-summary")?.textContent).toContain("0ports");
    await act(async () => fresh.resolve([]));
  });

  it("the main window cleans up event listeners that register after unmount", async () => {
    const registered = deferred<() => void>();
    const cleanup = vi.fn();
    events.listen.mockImplementationOnce(() => registered.promise);
    await mount();
    await act(async () => root.unmount());
    root = createRoot(host);
    await act(async () => registered.resolve(cleanup));
    expect(cleanup).toHaveBeenCalledTimes(1);
  });
});

describe("separate hover window handshake", () => {
  it("passive revision zero cannot erase a batched render acknowledgement", async () => {
    await mount(<CompactHover />);
    const data = { apps: [{ id: "app", name: "Test", online: true }], bubbleScale: 1 };
    await act(async () => {
      send("port-lens://compact-hover-data", { ...data, revision: 7 });
      send("port-lens://compact-hover-data", { ...data, revision: 0 });
    });
    expect(events.emitTo).toHaveBeenCalledWith("main", "port-lens://compact-hover-rendered", { revision: 7 });
  });
});
