<div align="center">

# pi-switch

[![Version](https://img.shields.io/badge/version-0.3.5-blue.svg)](https://github.com/user/pi-switch/releases)
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey.svg)](https://github.com/user/pi-switch/releases)
[![Built with Rust](https://img.shields.io/badge/built%20with-Rust-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)

**TUI + CLI dual-mode profile switcher for pi agent**

Manage provider profiles and run a local model-name routing gateway with failover — via an interactive TUI or CLI.

[English](#) | [中文](README_ZH.md)

</div>

---

## 📸 Screenshots

<div align="center">
  <img src="assets/main.png" alt="pi-switch TUI" width="80%"/>
</div>

---

## 📥 Installation

```bash
# npm (recommended)
npm install -g @cokefenta/pi-switch

# or via pi
pi install npm:@cokefenta/pi-switch
```

**Build from source** (requires Node.js >= 20, Rust 1.80+):

```bash
git clone https://github.com/user/pi-switch.git
cd pi-switch
npm install
npm run build:native
node bin/pi-switch.js tui
```

---

## 🚀 Quick Start

```bash
pi-switch tui          # Interactive TUI (recommended)
pi-switch doctor       # Run environment diagnostics
```

### Essential CLI Commands

```bash
# Provider management
pi-switch provider add <name> [--preset <id>] [--api-key <key>]
pi-switch provider list
pi-switch provider show <name>
pi-switch provider delete <name>
pi-switch provider expose <name> <model-ids...>    # Expose models to pi agent
pi-switch provider fetch-models <name>             # Fetch models from API

# Proxy (gateway)
pi-switch proxy failover <p1,p2,...>               # Same-model fallback chain
pi-switch proxy start --daemon                     # Start proxy daemon
pi-switch proxy status

# Other
pi-switch presets list                             # List built-in presets
pi-switch config show                               # Display current config
pi-switch config backups                            # List backup files
pi-switch config export <passphrase>                # Encrypted export
pi-switch config import <path> <passphrase>         # Encrypted import
pi-switch stats                                     # View request statistics
```

---

## ✨ Features

| Category | Highlights |
|----------|------------|
| 🔌 **Provider Management** | CRUD, duplicate, search/filter, model management, expose to pi agent |
| 💡 **Built-in Presets** | OpenRouter, Anthropic, DeepSeek, SiliconFlow, OpenAI — add profiles instantly |
| 🌉 **Model-Name Gateway** | Stateless routing by `profile/model` in the request body, custom User-Agent, OpenAI ↔ Anthropic conversion, failover, circuit breaker |
| 🖥️ **Interactive TUI** | ratatui-powered, Dracula theme, mouse support, vim keys (`hjkl`) |
| 🌐 **Bilingual** | English / 中文, persisted to config, toggle in Settings |
| 📊 **Usage Stats** | Per-provider, per-model request metrics & latency |
| 💾 **Backup & Sync** | Auto-backup on mutation, AES-256-CBC encrypted export/import |
| 🩺 **Diagnostics** | `doctor` command checks config, models.json, structure |

---

## 🎯 Core Workflow

### Gateway Routing & Failover

```mermaid
graph LR
    subgraph Setup["⚙️ Setup"]
        A[Add Provider] --> B[Configure Models]
        B --> C[Expose to Pi]
        C --> D[Set Failover Chain]
    end

    subgraph Runtime["🚀 Runtime"]
        E["Request<br/>model: provider-a/gpt-5.4"] --> F{Resolve Route}
        F --> G[Try provider-a]
        G --> H{Success?}
        H -->|✓| I[Response]
        H -->|✗ 429/5xx| J[Try provider-b]
        J --> K{Success?}
        K -->|✓| I
        K -->|✗| L[Circuit Breaker]
        L --> M[60s Cooldown]
        M --> N[Half-Open Probe]
        N -->|✓| G
        N -->|✗| M
    end

    Setup --> Runtime

    style A fill:#50fa7b,stroke:#50fa7b,color:#282a36
    style E fill:#8be9fd,stroke:#8be9fd,color:#282a36
    style I fill:#50fa7b,stroke:#50fa7b,color:#282a36
    style L fill:#ff5555,stroke:#ff5555,color:#f8f8f2
```

### Step by Step

**1. Add a provider** (CLI or TUI)
```bash
pi-switch provider add provider-a --api openai-completions --base-url https://api.example.com/v1 \
    --api-key '$API_KEY' --models gpt-5.4,claude-sonnet-4-5
```
In TUI: `Profiles → a → fill form → Ctrl+S`

**2. Expose models to pi agent** — choose which models appear in `~/.pi/agent/models.json`
```bash
pi-switch provider expose provider-a gpt-5.4
```
In TUI: `Profiles → select provider → x`

**3. Start the proxy** — it writes a single `pi-switch` gateway provider to pi
```bash
pi-switch proxy failover provider-b,provider-c          # optional same-model fallback
pi-switch proxy start --daemon
```

**4. Use in pi** — select the `pi-switch` provider, then pick a `profile/model` like `provider-a/gpt-5.4`

### How Gateway Routing Works

Requests are routed by the model name in the request body — no out-of-band state, no "current target":

- **Model-name routing** — `"model": "provider-a/gpt-5.4"` resolves to profile `provider-a`, real model `gpt-5.4`; the proxy rewrites the body before forwarding upstream
- **Single gateway provider** — pi sees one `pi-switch` provider advertising every exposed model as `profile/realModelId`; switching model in pi = sending a different model string = instant routing change
- **Automatic failover** — same-model fallback across the configured chain on 429/5xx errors or network failures
- **Circuit breaker** — after 3 consecutive failures, provider enters 60s cooldown; auto-recovery on half-open probe success
- **OpenAI ↔ Anthropic** — transparently converts between chat completions and messages APIs
- **Custom User-Agent** — set a custom User-Agent string (e.g. `Codex/1.0`) to pass client checks from upstream channels

---

## 🏗️ Architecture

```
pi-switch/
├── bin/pi-switch.js         # CLI entry point
├── index.js                 # ESM wrapper for native addon
├── pi-switch-native.cjs     # NAPI loader (auto platform detection)
├── src-rust/                # Rust native core (napi-rs)
│   ├── lib.rs               # NAPI function exports
│   ├── config.rs            # Config load/save, types
│   ├── ops.rs               # Core operations
│   ├── presets.rs           # Built-in provider presets
│   ├── proxy.rs             # Proxy server (gateway routing, failover, circuit breaker)
│   ├── daemon.rs            # Daemon lifecycle
│   ├── stats.rs             # Request log aggregation
│   ├── sync.rs              # Encrypted export/import
│   └── tui/                 # Interactive terminal UI (ratatui)
│       ├── app.rs           # State machine + key handler
│       ├── form.rs          # Provider form state
│       ├── i18n.rs          # Bilingual (EN/ZH)
│       └── ui/              # Rendering (chrome, pages, overlays)
├── src/                     # JavaScript layer (pi extension support)
├── extensions/index.ts      # Pi agent extension (/piswitch)
└── Cargo.toml
```

**Config files:**
- `~/.pi-switch/config.json` — profiles, proxy settings, failover chain
- `~/.pi-switch/backups/` — timestamped auto-backups on every mutation
- `~/.pi/agent/models.json` — pi's provider registry (pi-switch writes a single gateway provider)

---

## ❓ FAQ

<details>
<summary><b>How do I switch models in pi?</b></summary>
<br>

In pi, open `/model` and pick any advertised `profile/model` (e.g. `provider-a/gpt-5.4`). The proxy routes by the model name in each request — no extra step needed.

To add more models, expose them in TUI (`Profiles → select provider → x`) or via CLI:
```bash
pi-switch provider expose <name> <model-id>...
```

</details>

<details>
<summary><b>How do I set up failover?</b></summary>
<br>

In TUI: `Settings → Failover` → `Enter` → enter comma-separated profile names → `Enter`.
Or via CLI:
```bash
pi-switch proxy failover provider-b,provider-c
```

Profiles in the failover chain that expose the same model are tried in order when the primary fails.

</details>

<details>
<summary><b>What does the [proxy] badge mean?</b></summary>

<br>

The `[proxy]` badge indicates this profile is a meta-profile (with `"proxy": true`). Proxy profiles are used to register a pi provider that points to the local gateway. They are excluded from upstream routing.

In the current gateway mode, proxy profiles are typically not needed — the proxy automatically writes a single `pi-switch` gateway provider to pi's models.json on startup.

</details>

<details>
<summary><b>How does gateway routing work?</b></summary>

<br>

The proxy advertises every exposed model as `profile/realModelId` under a single `pi-switch` provider. When pi sends a request with `"model": "provider-a/gpt-5.4"`, the proxy:

1. Splits on the first `/` — profile `provider-a`, real model `gpt-5.4`
2. Routes to the `provider-a` profile's upstream, rewriting `body.model` to `gpt-5.4`
3. On failure (429/5xx), tries the failover chain for any other profile exposing `gpt-5.4`

```bash
# 1. Expose models (per profile)
pi-switch provider expose provider-a gpt-5.4
pi-switch provider expose provider-b gpt-5.4

# 2. Set failover chain (optional)
pi-switch proxy failover provider-b

# 3. Start proxy daemon
pi-switch proxy start --daemon
```

In pi, select the `pi-switch` provider, then `provider-a/gpt-5.4`. The model name in each request determines the route — no "target" to manage.

</details>

<details>
<summary><b>Where is my data stored?</b></summary>
<br>

Everything under `~/.pi-switch/`. Pi's own registry is `~/.pi/agent/models.json`. No data leaves your machine.

</details>

---

## 🛠️ Development

```bash
npm run build:native:debug     # Build Rust addon (debug)
npm run build:native           # Build Rust addon (release)
cargo build                    # Rust-only build
cargo clippy                   # Lint
cargo fmt                      # Format
cargo test --release --lib     # Run unit tests
```

**Note:** Stop the TUI/daemon before `npm run build:native` to avoid file-lock errors on Windows.

---

## 🙏 Acknowledgments

- **[cc-switch](https://github.com/farion1231/cc-switch)** — the original TUI-based profile switcher for Claude Code, which pioneered the interactive terminal UI pattern and proxy failover design
- **[cc-switch-cli](https://github.com/SaladDay/cc-switch-cli)** — the CLI counterpart, providing a clean command-line interface for provider management

Thanks also to the **[LINUX DO](https://linux.do/)** community for the discussions that sparked this project.

---

## 📜 License

MIT
