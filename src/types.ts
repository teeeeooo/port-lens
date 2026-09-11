export interface ListenerInfo {
  protocol: string;
  localAddress: string;
  port: number;
  pid: number;
  processName: string;
  commandLine?: string;
}

export interface ManagedApp {
  id: string;
  name: string;
  port: number;
  command?: string;
  cwd?: string;
  lastProcessName?: string;
  lastCommandLine?: string;
  lastManagedPid?: number;
  lastManagedProcessName?: string;
  lastManagedCommandLine?: string;
  lastManagedListenerCreationTime?: string;
  lastManagedRootPid?: number;
  lastManagedRootCreationTime?: string;
  lastManagedRootCommandLine?: string;
}

export interface ManagedRuntime {
  appId: string;
  rootPid: number;
  reattached?: boolean;
}

export interface ManagedExitInfo {
  appId: string;
  appName: string;
  rootPid: number;
  exitCode?: number;
  elapsedMs: number;
  timestampMs: number;
  earlyExit: boolean;
}

export interface BubbleState {
  collapsed: boolean;
}

export interface AppSettings {
  language: "system" | "en" | "ko";
  bubbleScale: number;
  compactModeEnabled: boolean;
}

export interface SettingsPatch {
  language?: AppSettings["language"];
  bubbleScale?: number;
  compactModeEnabled?: boolean;
}

export type ManagedStatus = "running" | "starting" | "online" | "changed" | "offline";
