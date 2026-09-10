export interface ListenerInfo {
  protocol: string;
  localAddress: string;
  port: number;
  pid: number;
  processName: string;
}

export interface ManagedApp {
  id: string;
  name: string;
  port: number;
  command: string;
  cwd: string;
}

export interface ManagedRuntime {
  appId: string;
  rootPid: number;
}

export interface BubbleState {
  collapsed: boolean;
}

export type ManagedStatus = "running" | "occupied" | "stopped";
