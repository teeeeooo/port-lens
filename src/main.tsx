import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import CompactHover from "./CompactHover";
import { getStartupReady } from "./api";

const isCompactHoverWindow = new URLSearchParams(window.location.search).get("compactHover") === "1";

function MainStartupGate() {
  const [ready, setReady] = React.useState(false);

  React.useEffect(() => {
    let stopped = false;
    let timer: number | undefined;

    const check = async () => {
      try {
        if (await getStartupReady()) {
          if (!stopped) setReady(true);
          return;
        }
      } catch {
        // The bootstrap command is the only command allowed before backend setup completes.
      }
      if (!stopped) timer = window.setTimeout(() => void check(), 25);
    };

    void check();
    return () => {
      stopped = true;
      if (timer !== undefined) window.clearTimeout(timer);
    };
  }, []);

  return ready ? <App /> : null;
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {isCompactHoverWindow ? <CompactHover /> : <MainStartupGate />}
  </React.StrictMode>,
);
