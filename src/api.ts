import { invoke } from "@tauri-apps/api/core";
import type { ListenerInfo, ManagedApp, ManagedRuntime } from "./types";

export const getListeners = () => invoke<ListenerInfo[]>("get_listeners");
export const getManagedApps = () => invoke<ManagedApp[]>("get_managed_apps");
export const getManagedRuntimes = () => invoke<ManagedRuntime[]>("get_managed_runtimes");

export const saveManagedApp = (app: ManagedApp) =>
  invoke<ManagedApp>("save_managed_app", { app });

export const removeManagedApp = (appId: string) =>
  invoke<void>("remove_managed_app", { appId });

export const startManagedApp = (appId: string) =>
  invoke<ManagedRuntime>("start_managed_app", { appId });

export const stopManagedApp = (appId: string) =>
  invoke<void>("stop_managed_app", { appId });

export const restartManagedApp = (appId: string) =>
  invoke<ManagedRuntime>("restart_managed_app", { appId });

export const killListenerProcess = (pid: number, port: number) =>
  invoke<void>("kill_listener_process", { pid, port });
