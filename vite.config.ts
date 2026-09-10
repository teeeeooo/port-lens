import { readFile } from "node:fs/promises";
import process from "node:process";
import { defineConfig, type Plugin } from "vite";
import react from "@vitejs/plugin-react";

const host = process.env.TAURI_DEV_HOST;

function devMockPlugin(): Plugin {
  return {
    name: "port-lens-dev-mock",
    apply: "serve",
    configureServer(server) {
      server.middlewares.use("/__port-lens-mock", async (_request, response) => {
        try {
          const fixture = await readFile(
            new URL("./dev-mock/port-lens.json", import.meta.url),
            "utf8",
          );
          response.statusCode = 200;
          response.setHeader("Content-Type", "application/json; charset=utf-8");
          response.setHeader("Cache-Control", "no-store");
          response.end(fixture);
        } catch {
          response.statusCode = 404;
          response.end("Port Lens dev mock fixture is unavailable");
        }
      });
    },
  };
}

export default defineConfig(() => ({
  plugins: [react(), devMockPlugin()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**", "**/dev-mock/**"],
    },
  },
}));
