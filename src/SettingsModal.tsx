import { useEffect, useState } from "react";
import type { AppSettings } from "./types";
import type { UiLanguage } from "./i18n";
import { t } from "./i18n";

interface Props {
  settings: AppSettings;
  language: UiLanguage;
  onChange: (next: Partial<AppSettings>) => Promise<void>;
  onClose: () => void;
}

export default function SettingsModal({ settings, language, onChange, onClose }: Props) {
  const [bubblePercent, setBubblePercent] = useState(Math.round(settings.bubbleScale * 100));

  useEffect(() => {
    setBubblePercent(Math.round(settings.bubbleScale * 100));
  }, [settings.bubbleScale]);

  const commitBubbleSize = (value: number) => void onChange({ bubbleScale: value / 100 });

  return (
    <div className="modal-backdrop" onMouseDown={onClose}>
      <div className="modal-card settings-card" onMouseDown={(event) => event.stopPropagation()}>
        <div className="modal-header">
          <div>
            <span className="eyebrow">PORT LENS</span>
            <h2>{t(language, "settingsTitle")}</h2>
            <p>{t(language, "settingsDescription")}</p>
          </div>
          <button type="button" className="icon-button" onClick={onClose}>Close</button>
        </div>
        <div className="settings-row">
          <div>
            <strong>Language</strong>
            <p>{t(language, "languageDescription")}</p>
          </div>
          <select
            value={settings.language}
            onChange={(event) => void onChange({ language: event.currentTarget.value as AppSettings["language"] })}
          >
            <option value="system">System</option>
            <option value="en">English</option>
            <option value="ko">한국어</option>
          </select>
        </div>

        <div className="settings-row settings-slider-row">
          <div>
            <strong>Bubble size</strong>
            <p>{t(language, "bubbleSizeDescription")}</p>
          </div>
          <div className="settings-slider-control">
            <input
              type="range"
              min="70"
              max="150"
              step="10"
              value={bubblePercent}
              onChange={(event) => setBubblePercent(Number(event.currentTarget.value))}
              onPointerUp={(event) => commitBubbleSize(Number(event.currentTarget.value))}
              onKeyUp={(event) => commitBubbleSize(Number(event.currentTarget.value))}
              onBlur={(event) => commitBubbleSize(Number(event.currentTarget.value))}
            />
            <output>{bubblePercent}%</output>
          </div>
        </div>
      </div>
    </div>
  );
}
