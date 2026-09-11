import { invoke } from "@tauri-apps/api/core";
import {
  isDevMockMode,
  mockGetApps,
  mockGetListeners,
  mockGetMonitoredListeners,
  mockGetRuntimes,
  mockGetSettings,
  mockKillListener,
  mockOpenLogs,
  mockRemoveApp,
  mockRestartApp,
  mockSaveApp,
  mockStartApp,
  mockStopApp,
  mockUpdateSettings,
} from "./devMock";
import type { AppSettings, BubbleState, ListenerInfo, ManagedApp, ManagedRuntime, SettingsPatch } from "./types";

export const getListeners = () =>
  isDevMockMode ? mockGetListeners() : invoke<ListenerInfo[]>("get_listeners");

export const getMonitoredListeners = () =>
  isDevMockMode ? mockGetMonitoredListeners() : invoke<ListenerInfo[]>("get_monitored_listeners");

export const getManagedApps = () =>
  isDevMockMode ? mockGetApps() : invoke<ManagedApp[]>("get_managed_apps");

export const getManagedRuntimes = () =>
  isDevMockMode ? mockGetRuntimes() : invoke<ManagedRuntime[]>("get_managed_runtimes");

export const saveManagedApp = (app: ManagedApp) =>
  isDevMockMode ? mockSaveApp(app) : invoke<ManagedApp>("save_managed_app", { app });

export const removeManagedApp = (appId: string) =>
  isDevMockMode ? mockRemoveApp(appId) : invoke<void>("remove_managed_app", { appId });

export const startManagedApp = (appId: string) =>
  isDevMockMode ? mockStartApp(appId) : invoke<ManagedRuntime>("start_managed_app", { appId });

export const stopManagedApp = (appId: string) =>
  isDevMockMode ? mockStopApp(appId) : invoke<void>("stop_managed_app", { appId });

export const restartManagedApp = (appId: string) =>
  isDevMockMode ? mockRestartApp(appId) : invoke<ManagedRuntime>("restart_managed_app", { appId });

export const killListenerProcess = (pid: number, port: number) =>
  isDevMockMode
    ? mockKillListener(pid, port)
    : invoke<void>("kill_listener_process", { pid, port });

export const getSettings = () =>
  isDevMockMode ? mockGetSettings() : invoke<AppSettings>("get_settings");

export const updateSettings = (patch: SettingsPatch) =>
  isDevMockMode ? mockUpdateSettings(patch) : invoke<AppSettings>("update_settings", { patch });

export const openLogs = () =>
  isDevMockMode ? mockOpenLogs() : invoke<void>("open_logs");

const mockBubbleState = (): BubbleState => ({
  collapsed: new URLSearchParams(window.location.search).get("bubble") === "1",
});

export const getBubbleState = () =>
  isDevMockMode ? Promise.resolve(mockBubbleState()) : invoke<BubbleState>("get_bubble_state");

export const collapseToBubble = () =>
  isDevMockMode
    ? Promise.resolve<BubbleState>({ collapsed: true })
    : invoke<BubbleState>("collapse_to_bubble");

export const minimizeMainWindow = () =>
  isDevMockMode
    ? Promise.resolve<BubbleState>({ collapsed: true })
    : invoke<BubbleState>("minimize_main_window");

export const moveCompactBubble = (offsetRatioX: number, offsetRatioY: number, persist = false) =>
  isDevMockMode
    ? Promise.resolve(mockBubbleState())
    : invoke<BubbleState>("move_compact_bubble", {
        offset: { offsetRatioX, offsetRatioY, persist },
      });

export const expandFromBubble = () =>
  isDevMockMode
    ? Promise.resolve<BubbleState>({ collapsed: false })
    : invoke<BubbleState>("expand_from_bubble");
