# Port Lens

Port Lens is a lightweight desktop dashboard for local development servers.
It shows which TCP ports are listening, maps them to processes and PIDs, and lets you register trusted development services for one-click Start / Stop / Restart.

The primary target is Windows 11. macOS listener discovery is also implemented so the project can be developed and validated from macOS.

## Why

Local development often leaves several servers running at once. Finding the owner of a busy port usually means jumping between `netstat`, Task Manager, PowerShell, and terminal windows.

Port Lens keeps that workflow in one small local application:

- discover active TCP listening ports
- show process name, PID, bind address, and protocol
- search by port, process, PID, or address
- register trusted development apps with a command, working directory, and preferred port
- Start / Stop / Restart registered apps
- open a registered localhost service in the browser
- detect port conflicts before starting an app
- explicitly confirm before terminating an unmanaged process
- stay available from the system tray

## Current scope

| Capability | Status |
| --- | --- |
| Windows TCP listener discovery | Implemented |
| macOS TCP listener discovery | Implemented |
| Process / PID mapping | Implemented |
| Managed app registry | Implemented |
| Start / Stop / Restart | Implemented |
| Port conflict detection | Implemented |
| Unmanaged process termination | Implemented with confirmation |
| Tray resident mode | Implemented |
| Single-instance behavior | Implemented |
| Light / dark appearance | Follows the OS |
| UDP listeners | Not in v0.1 |
| Health checks | Planned |
| Safe adoption of servers started outside Port Lens | Planned |
| Windows packaged release | Not yet published |

## Safety model

Port Lens deliberately distinguishes a **managed app** from an arbitrary process that happens to own a port.
A managed app can only be stopped through the managed controls when Port Lens started it during the current application session.

For unmanaged listeners, Port Lens presents a separate **Kill** action and asks for confirmation before terminating the process tree. It also refuses to terminate protected PIDs such as PID 0, PID 4, and Port Lens itself.

When a managed app is started, Port Lens checks the preferred port first. If another process already owns it, the app is not started and the conflict is shown instead of automatically killing the blocker.

This is intentionally more conservative than aggressively taking over a port.

## Architecture

```text
React / TypeScript UI
        │
        │ Tauri commands
        ▼
Rust backend
 ├─ ports.rs            listener discovery + normalization
 ├─ process_control.rs  spawn / stop / process checks
 ├─ registry.rs         persisted managed-app configuration
 └─ lib.rs              commands, tray, window lifecycle
```

### Windows listener discovery

Windows uses `Get-NetTCPConnection -State Listen` and `Get-Process`, serialized as JSON by PowerShell. Parsing structured output avoids depending on localized `netstat` state strings.

### macOS listener discovery

macOS uses `lsof -nP -iTCP -sTCP:LISTEN -Fpcn` and parses its machine-oriented field output.

## Tech stack

- Tauri 2
- Rust
- React 19
- TypeScript
- Vite

Tauri was selected instead of Electron to keep the desktop shell and runtime footprint comparatively small while retaining native process and tray integration.

## Development

Requirements:

- Node.js
- npm
- Rust toolchain
- Tauri desktop prerequisites for the host OS

```bash
npm install
npm run tauri dev
```

Validation:

```bash
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

## Design references

Port Lens is an independent implementation, but its initial product direction was informed by several established or relevant tools:

- **PortPilot** — unified managed-app and active-port workflow, conflict visibility, tray-oriented local development UX
- **PortManager** — dense searchable port/process table and explicit confirmation before process termination
- **Microsoft TCPView / Sysinternals** — straightforward network-endpoint-to-process visibility
- **System Informer** — clear separation between process inspection and process control

No source code from those projects is copied into Port Lens. The references above are product and interaction patterns only.

## v0.1 principles

1. Local only — no account, cloud service, or telemetry is required.
2. Show the OS truth — active listeners come from the operating system rather than a manually maintained list.
3. Manage only what is explicit — registered apps get Start / Stop / Restart; unknown listeners remain separate.
4. Prefer a visible conflict over silently killing a process.
5. Keep the UI dense enough to answer “what is using this port?” immediately.

## Repository

`https://github.com/teeeeooo/port-lens`
