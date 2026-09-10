import { invoke } from "@tauri-apps/api/core";
import {
  isDevMockMode,
  mockGetApps,
  mockGetListeners,
  mockGetRuntimes,
  mockKillListener,
  mockRemoveApp,
  mockRestartApp,
  mockSaveApp,
  mockStartApp,
  mockStopApp,
} from "./devMock";
import type { ListenerInfo, ManagedApp, ManagedRuntime } from "./types";

export const getListeners = () =>
  isDevMockMode ? mockGetListeners() : invoke<ListenerInfo[]>("get_listeners");

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
