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
}

export interface ManagedRuntime {
  appId: string;
  rootPid: number;
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
