# radio

> A voice-first remote control plane for coding agents.

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/Platform-macOS%20%7C%20iOS-lightgrey.svg)]()
[![Daemon: Rust](https://img.shields.io/badge/Daemon-Rust-orange.svg)]()
[![Client: React](https://img.shields.io/badge/Client-React%20%2B%20TypeScript-blue.svg)]()

`radio` is a lightweight remote interface for controlling coding agents from your phone. A **Rust**, **Tokio**, and **Axum** daemon runs next to your repository and owns the agent process, while a mobile-first **React** client lets you talk, send prompts, stream responses, hear completed answers, stop work, and inspect Git diffs from Safari.

Radio is designed around a simple idea: your development machine remains the execution environment; your phone becomes the control plane.

---

## Table of Contents

- [Overview & Architecture](#overview--architecture)
- [Installation](#installation)
  - [Daemon](#daemon)
  - [Mobile Client](#mobile-client)
- [Quick Start](#quick-start)
- [Key Features](#key-features)
- [Voice Workflow](#voice-workflow)
- [Remote Access with Tailscale](#remote-access-with-tailscale)
- [Protocol & Endpoints](#protocol--endpoints)
- [Security Model](#security-model)
- [Development](#development)
- [License](#license)

---

## Overview & Architecture

```text
                    iPhone / Safari
                          │
                Speech Recognition / TTS
                          │
                          ▼
                 React + TypeScript
                          │
                   HTTP / WebSocket
                          │
                   Tailscale tailnet
                          │
                          ▼
                ┌───────────────────┐
                │   radio daemon    │
                │   Rust + Axum     │
                │                   │
                │ Session Manager   │
                │ Agent Protocol    │
                │ Git Diff          │
                └─────────┬─────────┘
                          │
                          ▼
                     Codex CLI
                          │
                          ▼
                    local repo
```

The phone never becomes the development environment. Source files, credentials, Git state, shell access, and the coding-agent process remain on the host machine. Radio only transports commands, status, text output, and repository diffs between the two.

---

## Installation

### Daemon

Clone and build Radio with Cargo:

```bash
git clone https://github.com/delaudio/radio.git
cd radio
cargo build --release
```

Start the daemon against the repository you want Codex to control:

```bash
cargo run -- serve /path/to/repository
```

Radio listens on `127.0.0.1:8787` by default.

### Mobile Client

Install the web client dependencies:

```bash
cd web
npm install
npm run dev
```

The Vite development server proxies HTTP and WebSocket traffic to the local Rust daemon.

---

## Quick Start

Start the daemon:

```bash
cargo run -- serve ~/Developer/my-project
```

Start the client in another terminal:

```bash
cd web
npm run dev
```

Open the Vite URL on your phone. From the mobile interface you can:

- create a Codex session;
- type a prompt or hold the voice control to dictate one;
- edit the transcript before sending;
- watch agent output stream back in real time;
- hear the final response through client-side text-to-speech;
- stop the active agent;
- inspect the current working-tree diff.

---

## Key Features

### 🎙 Voice-First Control
- **Push to Talk**: Dictate instructions using browser speech recognition.
- **Editable Transcript**: Voice input lands in the same composer as typed prompts.
- **Local Audio Boundary**: Radio sends recognized text to the daemon, not microphone audio.

### 🔊 Spoken Responses
- **Client-Side TTS**: Completed responses can be read aloud by the phone.
- **Noise Reduction**: Streaming terminal output is not spoken line by line.
- **Immediate Mute**: Active speech can be stopped at any time.

### ⚡ Persistent Agent Sessions
- **Long-Lived Codex Process**: Multiple prompts can be sent to the same session.
- **Reconnect Friendly**: Closing the mobile WebSocket does not immediately kill Codex.
- **Graceful Cleanup**: Radio stops managed child processes when the daemon shuts down.

### ↔ Real-Time WebSocket Protocol
- **Streaming Output**: Agent output is forwarded incrementally to connected clients.
- **Small Message Model**: Prompt, stop, status, output, error, and done events.
- **Transport Separation**: Agent lifecycle is independent from the mobile UI.

### ± Git Diff Inspection
- **Read Only**: The mobile client can request `git diff` without mutating the repository.
- **Mobile Rendering**: Added, removed, and hunk lines are visually separated.
- **Automatic Refresh**: The diff refreshes after an agent task completes.

### 🔐 Private Remote Access
- **Tailscale First**: The intended MVP deployment stays inside a trusted tailnet.
- **No Public Port Forwarding**: The Rust daemon can remain bound to localhost.
- **Host Remains in Control**: Repository files and agent credentials never move to the phone.

---

## Voice Workflow

```text
Hold to talk
     │
     ▼
Browser speech recognition
     │
     ▼
Editable transcript
     │
     ▼
WebSocket prompt
     │
     ▼
Codex on the host
     │
     ├── streaming output ──► phone UI
     │
     └── completion ─────────► text-to-speech
```

Speech recognition and text-to-speech are progressive enhancements. If browser voice APIs are unavailable, the text composer remains fully usable.

---

## Remote Access with Tailscale

Install Tailscale on both the host Mac and the iPhone and connect them to the same tailnet.

Start Radio on the Mac while keeping the daemon on localhost:

```bash
cargo run -- serve /path/to/repository
```

Then start the Vite client:

```bash
cd web
npm run dev
```

Find the Mac Tailscale address:

```bash
tailscale ip -4
```

With Tailscale connected on the iPhone, open Safari using the Vite port shown in the terminal:

```text
http://100.x.y.z:5173
```

If MagicDNS is enabled, the Mac hostname can be used instead:

```text
http://your-mac-name:5173
```

Vite is the tailnet-facing development server. It proxies session, diff, health, and WebSocket traffic locally to Radio on `127.0.0.1:8787`. You do not need to expose the Rust daemon or configure router port forwarding.

---

## Protocol & Endpoints

| Endpoint | Method | Purpose |
| :--- | :--- | :--- |
| `/health` | `GET` | Daemon health check |
| `/sessions` | `POST` | Create a Codex session |
| `/sessions/{id}/ws` | `WebSocket` | Send prompts and receive realtime agent events |
| `/diff` | `GET` | Read the current working-tree diff |

WebSocket client messages:

```json
{ "type": "prompt", "text": "Refactor the sampler and run the tests" }
```

```json
{ "type": "stop" }
```

Server events use the same tagged JSON model: `status`, `output`, `error`, and `done`.

---

## Security Model

Radio is a remote control surface for a coding agent, not a read-only dashboard. A connected client can instruct an agent that may have filesystem and shell access under the permissions of the host user.

For the current MVP:

- keep Radio inside a trusted Tailscale tailnet;
- do not expose Vite or the daemon directly to the public internet;
- keep the Rust daemon on `127.0.0.1` for the normal mobile workflow;
- treat access to the Radio UI as access to the configured repository and agent session;
- keep the coding agent permission and approval controls enabled according to your own workflow.

Radio currently has no application-level authentication. Tailscale is part of the MVP security boundary.

---

## Development

### Rust daemon

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo build
```

### Web client

```bash
cd web
npm install
npm run build
npm run dev
```

Daemon host and port can be configured with flags:

```bash
cargo run -- serve /path/to/repository --host 127.0.0.1 --port 9000
```

or environment variables:

```bash
RADIO_HOST=127.0.0.1 RADIO_PORT=9000 cargo run -- serve /path/to/repository
```

---

## License

This project is licensed under the [MIT License](LICENSE).
