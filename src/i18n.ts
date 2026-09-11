export type LanguageSetting = "system" | "en" | "ko";
export type UiLanguage = "en" | "ko";

export function resolveLanguage(setting: LanguageSetting): UiLanguage {
  if (setting === "en" || setting === "ko") return setting;
  const primary = navigator.languages[0] ?? navigator.language ?? "en";
  return primary.toLowerCase().startsWith("ko") ? "ko" : "en";
}

type MessageKey = keyof typeof EN;
type Params = Record<string, string | number>;

function format(template: string, params: Params = {}) {
  return template.replace(/\{(\w+)\}/g, (_, key: string) => String(params[key] ?? `{${key}}`));
}

const EN = {
  headerDescription: "See what is listening, then start or stop the services you actually manage.",
  managedDescription: "Registered Ports stay monitored even when offline. Add launch settings only when you want Start / Stop / Restart control.",
  emptyTitle: "No Apps registered yet",
  emptyDescription: "Register a discovered Port below, or add an App manually.",
  conflict: "Port {port} is already held by {process} (PID {pid}).",
  monitoredDescription: "Only selected Ports are checked frequently and enriched with process details.",
  monitoredEmptyTitle: "No monitored Ports yet",
  monitoredEmptyDescription: "Select Monitor in the Listening ports list to keep a Port under active watch.",
  listenersDescription: "All active TCP listeners. This inventory refreshes periodically without heavy process inspection.",
  formHelp: "Working directory and Start command are optional, but must be configured together. Without them, the App remains monitoring-only.",
  monitoringOnlyNote: "Monitoring only. Add a working directory and Start command in Edit to enable lifecycle controls.",
  differentProcessWarning: "This Port is now owned by a different process than the one originally registered.",
  earlyExitNotice: "Start process exited shortly after launch: code {code} after {seconds}s. Open Logs for details.",
  lastExitNotice: "Last Port Lens start process exited: code {code} after {seconds}s. Open Logs for details.",
  selectWorkingDirectory: "Select working directory",
  removeConfirm: "Remove {name} from Port Lens?",
  terminateTitle: "Terminate process?",
  terminateWarning: "This will terminate the selected process tree. Unsaved work in that process can be lost.",
  settingsTitle: "Settings",
  settingsDescription: "Interface preferences are saved for the next launch.",
  languageDescription: "System follows the operating-system language. Technical labels remain in English.",
  compactModeDescription: "When enabled, minimizing Port Lens collapses it into the floating compact monitor. When disabled, minimize hides the window to the tray.",
  bubbleSizeDescription: "Adjust the floating bubble from 70% to 150%. The size is saved automatically.",
  diagnosticsDescription: "Open Port Lens diagnostic logs for startup, scan, action, timeout, and crash information.",
} as const;

const KO: Record<MessageKey, string> = {
  headerDescription: "현재 열려 있는 Port를 확인하고, 직접 관리하는 서비스만 안전하게 시작하거나 중지할 수 있습니다.",
  managedDescription: "등록한 Port는 Offline이 되어도 계속 모니터링합니다. Start / Stop / Restart가 필요할 때만 launch 설정을 추가하세요.",
  emptyTitle: "등록된 App이 없습니다",
  emptyDescription: "아래 Listening ports에서 Port를 등록하거나 App을 직접 추가할 수 있습니다.",
  conflict: "Port {port}는 현재 {process} (PID {pid})가 사용 중입니다.",
  monitoredDescription: "선택한 Port만 자주 확인하고 process 세부 정보를 보강합니다.",
  monitoredEmptyTitle: "모니터링 중인 Port가 없습니다",
  monitoredEmptyDescription: "Listening ports 목록에서 Monitor를 선택하면 해당 Port를 계속 확인합니다.",
  listenersDescription: "현재 열려 있는 전체 TCP LISTEN Port입니다. 이 목록은 가벼운 inventory 방식으로 주기적으로 갱신됩니다.",
  formHelp: "Working directory와 Start command는 선택 사항이지만 둘은 함께 설정해야 합니다. 설정하지 않으면 monitoring-only App으로 동작합니다.",
  monitoringOnlyNote: "Monitoring-only App입니다. Edit에서 working directory와 Start command를 추가하면 Start / Stop / Restart를 사용할 수 있습니다.",
  differentProcessWarning: "처음 등록한 process와 다른 process가 현재 이 Port를 사용하고 있습니다.",
  earlyExitNotice: "Start 직후 root process가 종료되었습니다: {seconds}초 후 종료, code {code}. Logs에서 상세 내용을 확인하세요.",
  lastExitNotice: "마지막 Port Lens start process가 종료되었습니다: {seconds}초 후 종료, code {code}. Logs에서 상세 내용을 확인하세요.",
  selectWorkingDirectory: "Working directory 선택",
  removeConfirm: "{name} App을 Port Lens에서 제거할까요?",
  terminateTitle: "Process를 종료할까요?",
  terminateWarning: "선택한 process tree 전체가 종료됩니다. 해당 process의 저장되지 않은 작업은 손실될 수 있습니다.",
  settingsTitle: "설정",
  settingsDescription: "인터페이스 설정은 저장되며 다음 실행에도 유지됩니다.",
  languageDescription: "System은 운영체제 언어를 따릅니다. App, Port, Process 같은 기술 용어는 영어로 유지합니다.",
  compactModeDescription: "켜면 최소화할 때 floating compact monitor로 전환합니다. 끄면 최소화할 때 창을 tray로 숨깁니다.",
  bubbleSizeDescription: "Floating bubble 크기를 70%~150%로 조절합니다. 변경값은 자동으로 저장됩니다.",
  diagnosticsDescription: "startup, scan, action, timeout, crash 정보를 확인할 수 있는 Port Lens diagnostic log 폴더를 엽니다.",
};

export function t(language: UiLanguage, key: MessageKey, params?: Params) {
  return format((language === "ko" ? KO : EN)[key], params);
}

export function localizeError(language: UiLanguage, message: string) {
  if (language !== "ko") return message;
  const fixed: Record<string, string> = {
    "Name is required.": "Name은 반드시 입력해야 합니다.",
    "Start command and working directory must be configured together.": "Start command와 working directory는 함께 설정해야 합니다.",
    "Configure a start command and working directory before starting this App.": "이 App을 Start하려면 start command와 working directory를 먼저 설정하세요.",
    "Compact mode is disabled.": "Compact mode가 꺼져 있습니다.",
    "Stop this app before editing its configuration.": "이 App의 설정을 변경하려면 먼저 Stop하세요.",
    "Stop this app before removing it.": "이 App을 제거하려면 먼저 Stop하세요.",
    "App was not found.": "등록된 App을 찾을 수 없습니다.",
    "This app was not started by Port Lens in the current session.": "이 App은 현재 세션에서 Port Lens가 시작한 process가 아닙니다.",
    "The selected listener changed. Refresh the list and try again.": "선택한 listener 상태가 변경되었습니다. Refresh 후 다시 시도하세요.",
    "Use the App Stop action for processes started by Port Lens.": "Port Lens가 시작한 process는 App의 Stop을 사용하세요.",
    "This process owns a Port assigned to a running App. Use Stop instead.": "이 process는 실행 중인 App의 Port를 사용하고 있습니다. Kill 대신 Stop을 사용하세요.",
  };
  if (fixed[message]) return fixed[message];
  const occupied = message.match(/^Port (\d+) is already occupied by (.+) \(PID (\d+)\)\.$/);
  if (occupied) return `Port ${occupied[1]}는 현재 ${occupied[2]} (PID ${occupied[3]})가 사용 중입니다.`;
  const registered = message.match(/^Port (\d+) is already registered as an App\.$/);
  if (registered) return `Port ${registered[1]}는 이미 App으로 등록되어 있습니다.`;
  return message;
}
