import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { emitTo, listen } from "@tauri-apps/api/event";
import type { CompactHoverPayload } from "./types";
import "./CompactHover.css";

const DATA_EVENT = "port-lens://compact-hover-data";
const PRESENCE_EVENT = "port-lens://compact-hover-presence";
const READY_EVENT = "port-lens://compact-hover-ready";
const RENDERED_EVENT = "port-lens://compact-hover-rendered";

const EMPTY_PAYLOAD: CompactHoverPayload = {
  apps: [],
  bubbleScale: 1,
  revision: 0,
};

export default function CompactHover() {
  const [payload, setPayload] = useState<CompactHoverPayload>(EMPTY_PAYLOAD);
  const receivedData = useRef(false);

  useEffect(() => {
    document.documentElement.classList.add("compact-hover-mode");
    let active = true;
    let cleanup: (() => void) | undefined;
    let readyTimer: number | undefined;

    const announceReady = () => {
      if (active && !receivedData.current) void emitTo("main", READY_EVENT, null);
    };

    void listen<CompactHoverPayload>(DATA_EVENT, (event) => {
      if (!active) return;
      receivedData.current = true;
      if (readyTimer !== undefined) {
        window.clearInterval(readyTimer);
        readyTimer = undefined;
      }
      setPayload(event.payload);
    }).then((unlisten) => {
      if (!active) {
        unlisten();
        return;
      }
      cleanup = unlisten;
      readyTimer = window.setInterval(announceReady, 250);
      announceReady();
    });

    return () => {
      active = false;
      if (readyTimer !== undefined) window.clearInterval(readyTimer);
      cleanup?.();
      document.documentElement.classList.remove("compact-hover-mode");
    };
  }, []);

  useEffect(() => {
    document.documentElement.style.setProperty("--bubble-scale", String(payload.bubbleScale));
  }, [payload.bubbleScale]);

  useLayoutEffect(() => {
    if (payload.revision === 0) return;
    void emitTo("main", RENDERED_EVENT, { revision: payload.revision });
  }, [payload.revision]);

  const reportPresence = (inside: boolean) => {
    void emitTo("main", PRESENCE_EVENT, { inside });
  };

  return (
    <main
      className="compact-hover-shell"
      aria-label="Registered Apps"
      onMouseEnter={() => reportPresence(true)}
      onMouseLeave={() => reportPresence(false)}
    >
      {payload.apps.map((app) => (
        <div className="compact-hover-row" key={app.id} title={app.name}>
          <span className={`compact-hover-dot ${app.online ? "online" : "offline"}`} aria-hidden="true" />
          <span className="compact-hover-name">{app.name}</span>
        </div>
      ))}
    </main>
  );
}
