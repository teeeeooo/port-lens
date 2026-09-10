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
  managedDescription: "Start commands you trust. Port Lens only stops processes it started in this session.",
  emptyTitle: "No managed apps yet",
  emptyDescription: "Add a dev server to get one-click Start / Stop / Restart.",
  conflict: "Port {port} is already held by {process} (PID {pid}).",
  listenersDescription: "Active TCP listeners discovered directly from the operating system.",
  formHelp: "Port Lens will launch this command in the working directory and keep its root PID for safe Stop / Restart.",
  removeConfirm: "Remove {name} from Port Lens?",
  terminateTitle: "Terminate process?",
  terminateWarning: "This will terminate the selected process tree. Unsaved work in that process can be lost.",
  settingsTitle: "Settings",
  settingsDescription: "Interface preferences are saved for the next launch.",
  languageDescription: "System follows the operating-system language. Technical labels remain in English.",
  bubbleSizeDescription: "Adjust the floating bubble from 70% to 150%. The size is saved automatically.",
} as const;

const KO: Record<MessageKey, string> = {
  headerDescription: "현재 열려 있는 Port를 확인하고, 직접 관리하는 서비스만 안전하게 시작하거나 중지할 수 있습니다.",
  managedDescription: "신뢰하는 command만 등록하세요. Port Lens는 현재 세션에서 직접 시작한 process만 Stop합니다.",
  emptyTitle: "등록된 App이 없습니다",
  emptyDescription: "개발 서버를 추가하면 Start / Stop / Restart를 한 번에 제어할 수 있습니다.",
  conflict: "Port {port}는 현재 {process} (PID {pid})가 사용 중입니다.",
  listenersDescription: "운영체제에서 직접 탐지한 현재 TCP LISTEN Port입니다.",
  formHelp: "Port Lens는 지정한 working directory에서 command를 실행하고, 안전한 Stop / Restart를 위해 root PID를 보관합니다.",
  removeConfirm: "{name} App을 Port Lens에서 제거할까요?",
  terminateTitle: "Process를 종료할까요?",
  terminateWarning: "선택한 process tree 전체가 종료됩니다. 해당 process의 저장되지 않은 작업은 손실될 수 있습니다.",
  settingsTitle: "설정",
  settingsDescription: "인터페이스 설정은 저장되며 다음 실행에도 유지됩니다.",
  languageDescription: "System은 운영체제 언어를 따릅니다. App, Port, Process 같은 기술 용어는 영어로 유지합니다.",
  bubbleSizeDescription: "Floating bubble 크기를 70%~150%로 조절합니다. 변경값은 자동으로 저장됩니다.",
};

export function t(language: UiLanguage, key: MessageKey, params?: Params) {
  return format((language === "ko" ? KO : EN)[key], params);
}

export function localizeError(language: UiLanguage, message: string) {
  if (language !== "ko") return message;
  const fixed: Record<string, string> = {
    "Name, command, and working directory are required.": "Name, command, working directory는 모두 입력해야 합니다.",
    "Stop this app before editing its configuration.": "이 App의 설정을 변경하려면 먼저 Stop하세요.",
    "Stop this app before removing it.": "이 App을 제거하려면 먼저 Stop하세요.",
    "Managed app was not found.": "등록된 App을 찾을 수 없습니다.",
    "This app was not started by Port Lens in the current session.": "이 App은 현재 세션에서 Port Lens가 시작한 process가 아닙니다.",
    "The selected listener changed. Refresh the list and try again.": "선택한 listener 상태가 변경되었습니다. Refresh 후 다시 시도하세요.",
    "Use the managed app Stop action for processes started by Port Lens.": "Port Lens가 시작한 process는 Managed App의 Stop을 사용하세요.",
    "This process owns a port assigned to a running managed app. Use Stop instead.": "이 process는 실행 중인 Managed App의 Port를 사용하고 있습니다. Kill 대신 Stop을 사용하세요.",
  };
  if (fixed[message]) return fixed[message];
  const occupied = message.match(/^Port (\d+) is already occupied by (.+) \(PID (\d+)\)\.$/);
  if (occupied) return `Port ${occupied[1]}는 현재 ${occupied[2]} (PID ${occupied[3]})가 사용 중입니다.`;
  return message;
}
