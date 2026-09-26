# radio

Radio is a remote, voice-oriented control plane for coding agents. The Rust daemon owns the coding-agent process; the mobile web client sends prompts and receives output over HTTP/WebSocket.

## Local development

Start Radio against the repository you want the agent to control:

```sh
cargo run -- serve /path/to/repository
```

The daemon listens on `127.0.0.1:8787` by default. The Vite development server proxies `/sessions`, `/diff`, `/health`, and WebSocket traffic to that local daemon.

In another terminal:

```sh
cd web
npm install
npm run dev
```

Open the Vite URL shown in the terminal.

## Remote access with Tailscale

The MVP is designed to be reached over a private Tailscale tailnet rather than by opening a public internet port. Install Tailscale on both the host Mac and the iPhone, sign both devices into the same tailnet, and confirm they can see each other in Tailscale.

### 1. Start the daemon on the Mac

Keep the Rust daemon bound to localhost. The phone does not need direct access to port 8787 because Vite proxies API and WebSocket traffic:

```sh
cargo run -- serve /path/to/repository
```

### 2. Start the mobile web client

Vite is configured with `host: true`, so it listens on network interfaces while forwarding Radio traffic to `127.0.0.1:8787`:

```sh
cd web
npm install
npm run dev
```

By default Vite normally uses port `5173`. Use the actual port printed by Vite if it chooses another one.

### 3. Find the Mac tailnet address

On the Mac you can use its Tailscale IPv4 address:

```sh
tailscale ip -4
```

or its MagicDNS hostname if MagicDNS is enabled for the tailnet.

### 4. Connect from the iPhone

With Tailscale connected on the iPhone, open Safari and navigate to the Vite server on the Mac, for example:

```text
http://100.x.y.z:5173
```

or, with MagicDNS:

```text
http://your-mac-name:5173
```

The browser talks to Vite over the tailnet. Vite then proxies session creation, Git diff requests, and WebSocket traffic locally to the Radio daemon. No Radio port needs to be forwarded on the router or exposed to the public internet.

## Security model

Radio is not a read-only dashboard. A connected client can send instructions to a coding agent running with the permissions of the host user. Depending on the coding agent configuration, that can result in filesystem changes and shell commands.

For the MVP:

- keep access restricted to a trusted Tailscale tailnet;
- do not expose Vite or the Radio daemon directly to the public internet;
- do not bind the Rust daemon to `0.0.0.0` merely to make phone access work; the Vite proxy makes that unnecessary;
- treat access to the Radio UI as access to the configured repository and coding-agent session;
- review agent permission and approval settings separately; Radio does not bypass or replace the agent safety controls.

Radio currently has no application-level authentication. Tailscale is therefore part of the MVP security boundary, not just a convenience.

## Daemon configuration

For non-mobile/local use, host and port can still be changed explicitly:

```sh
cargo run -- serve /path/to/repository --host 127.0.0.1 --port 9000
```

or with environment variables:

```sh
RADIO_HOST=127.0.0.1 RADIO_PORT=9000 cargo run -- serve /path/to/repository
```
