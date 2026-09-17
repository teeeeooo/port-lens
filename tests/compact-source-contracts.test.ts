// Static architecture checks. These do not simulate native Windows behavior.
import { readFileSync } from "node:fs";
import { expect, it } from "vitest";

it("position persistence never owns hover visibility", () => {
  const source = readFileSync("src-tauri/src/window_state.rs", "utf8");
  expect(source).not.toContain("hide_hover_panel");
});

it("hover remains a separate non-focusing child window", () => {
  const config = JSON.parse(readFileSync("src-tauri/tauri.conf.json", "utf8"));
  const hover = config.app.windows.find((window: { label: string }) => window.label === "compact-hover");
  expect(hover).toMatchObject({
    parent: "main", focusable: false, visible: false, resizable: false,
    decorations: false, skipTaskbar: true,
  });
});
