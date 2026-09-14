import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import CompactHover from "./CompactHover";

const isCompactHoverWindow = new URLSearchParams(window.location.search).get("compactHover") === "1";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {isCompactHoverWindow ? <CompactHover /> : <App />}
  </React.StrictMode>,
);
