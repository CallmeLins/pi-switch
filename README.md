<div align="center">

# pi-switch

[![Version](https://img.shields.io/badge/version-0.3.4-blue.svg)](https://github.com/user/pi-switch/releases)
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey.svg)](https://github.com/user/pi-switch/releases)
[![Built with Rust](https://img.shields.io/badge/built%20with-Rust-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)

**TUI + CLI dual-mode profile switcher for pi agent**

Manage provider profiles, switch models.json, and run a local proxy with failover. Interactive TUI with full CRUD, Dracula theme, and bilingual support.

[English](#) | [中文](README_ZH.md)

</div>

---

## 📖 About

pi-switch is a lightweight profile switcher for [pi coding agent](https://pi.dev). It manages `~/.pi/agent/models.json` provider profiles — add, edit, remove, and switch between them via CLI or an interactive terminal UI.

Built with Rust (napi-rs) as a native Node.js addon. The interactive TUI is modeled after cc-switch.

---

## 📸 Screenshots

<div align="center">
  <img src="assets/main.png" alt="pi-switch TUI" width="80%"/>
</div>

## 🚀 Quick Start

**TUI Mode (Recommended)**
```bash
pi-switch tui
```
Use the full-screen interface to manage providers, browse presets, inspect proxy status, and configure settings.

**Command-Line Mode**
```bash
# Provider Management
pi-switch provider list              # List all provider profiles
pi-switch provider add <name> [--preset <id>] [--api-key <key>]  # Add a profile
pi-switch provider show <name>       # Show profile details
pi-switch provider delete <name>     # Delete a profile
pi-switch provider models <name> <model-ids...>  # Update provider's model list
pi-switch provider expose <name> <model-ids...>  # Expose models to pi agent

# Transparent Proxy
pi-switch proxy target <name>        # Set proxy target profile
pi-switch proxy failover <p1,p2>     # Set failover chain
pi-switch proxy start --daemon       # Start proxy daemon
pi-switch proxy status               # Check proxy status

# Other
pi-switch presets list               # List built-in provider presets
pi-switch config show                # Display current config
pi-switch config backups             # List backup files
pi-switch stats                      # View proxy request statistics
pi-switch doctor                     # Run environment diagnostics
```

---

## 🎯 Core Workflow

### Provider Management & Intelligent Failover

```mermaid
graph LR
    subgraph Setup["⚙️ Setup Phase"]
        A[Add Provider] --> B[Configure Models]
        B --> C[Expose to Pi]
        C --> D[Set Failover Chain]
    end
    
    subgraph Runtime["🚀 Runtime Phase"]
        E[Request] --> F{Filter by Model}
        F --> G[Try Target]
        G --> H{Success?}
        H -->|✓| I[Response]
        H -->|✗ 429/5xx| J[Try Failover]
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

#### Workflow Steps

pi-switch provides a complete workflow for managing providers and enabling smart model-based failover:

#### 1️⃣ Add Provider (Manual Model Input)

Add a provider with manually specified models:

```bash
# CLI
pi-switch provider add relay-a --api openai --base-url https://relay.example.com/v1 --api-key '$API_KEY' --models deepseek-v4-pro,deepseek-chat

# TUI
Profiles → 'a' → fill form → models: "deepseek-v4-pro, deepseek-chat" → Ctrl+S
```

**Result:** Provider created with `models` list configured.

#### 2️⃣ Update Provider Models (Optional)

Update which models the provider supports:

```bash
# CLI
pi-switch provider models relay-a deepseek-v4-pro deepseek-chat

# Or fetch from provider's API
pi-switch provider fetch-models relay-a
```

**Purpose:** `models` defines which models this provider **supports** (used for failover routing).

#### 3️⃣ Expose Models to Pi Agent

Select which models to expose in `~/.pi/agent/models.json`:

```bash
# CLI
pi-switch provider expose relay-a deepseek-v4-pro

# TUI
Profiles → select provider → 'x' (expose models)
```

Check the models you want pi agent to see:
```
Provider: relay-a
Available models:
  [√] deepseek-v4-pro      ← Expose
  [ ] deepseek-chat        ← Don't expose
  [√] deepseek-v4-flash    ← Expose
```

**Result:** Only checked models are written to `~/.pi/agent/models.json`.

**Purpose:** `exposedModels` controls which models **appear in pi agent** (prevents config bloat).

#### 4️⃣ Configure Transparent Proxy

Set up proxy target and failover chain:

```bash
# Set the primary target
pi-switch proxy target deepseek-official

# Set failover chain
pi-switch proxy failover relay-a,relay-b

# Start proxy daemon
pi-switch proxy start --daemon

# Or configure in TUI
Settings → Target: deepseek-official
Settings → Failover: relay-a, relay-b
```

**Result:** The `deepseek-official` profile's baseUrl automatically points to the proxy server. When pi uses this profile, requests are transparently routed through the proxy with failover to `relay-a` → `relay-b`.

#### 5️⃣ Install Proxy Provider in Pi

Add the proxy provider to pi agent:

```bash
# CLI
pi-switch provider add proxy --preset proxy

# Or in TUI
Profiles → 'a' → select "proxy" preset → Ctrl+S
```

Now when you select models from the proxy-routed profile in pi's `/model`, all requests go through the proxy with intelligent failover.

---

### How Intelligent Failover Works

When a request comes in, the proxy intelligently routes based on model availability:

```
User requests: deepseek-v4-pro
↓
1. Filter candidates by model support
   Check each provider's `models` list:
   - deepseek-official.models: ["deepseek-v4-pro", ...] ✓
   - relay-a.models: ["deepseek-v4-pro", "deepseek-chat"] ✓
   - relay-b.models: ["deepseek-chat"] ✗ (no match, skip)

2. Try candidates in priority order
   - Try deepseek-official → 429 Rate Limit → Record failure
   - Try relay-a → Success ✓

3. Circuit breaker protection
   - After 3 consecutive failures, provider enters cooldown
   - Half-open state allows single probe after cooldown
   - Auto-recovery when probe succeeds
```

**Key Benefits:**
- **Smart routing**: Only tries providers that have the requested model
- **Automatic failover**: Seamlessly switches on 429/5xx errors or network failures
- **Circuit breaker**: Prevents cascade failures, auto-recovery
- **Model isolation**: `exposedModels` keeps pi config clean while `models` enables full failover

**Failover Triggers:**
- HTTP 429 (Rate Limit), 500, 502, 503, 504
- Network errors (timeout, connection failure)
- Circuit breaker open (3+ failures, 60s cooldown)

**Non-failover Cases:**
- 4xx client errors (400, 401, 403, 404) → returned directly
- 2xx success → returned directly

---

## 📥 Installation

### npm (Recommended)

```bash
npm install -g cokefenta@pi-switch
```

### Pi Package

```bash
pi install npm:cokefenta@pi-switch
```

### Build from Source

**Prerequisites:**
- Node.js >= 20
- Rust 1.80+ ([install via rustup](https://rustup.rs/))

**Build:**
```bash
git clone https://github.com/user/pi-switch.git
cd pi-switch
npm install
npm run build:native
node bin/pi-switch.js tui
```

---

## ✨ Features

### 🔌 Provider Management

Manage provider configurations for pi agent. Built-in presets: OpenRouter, Anthropic, DeepSeek, SiliconFlow, OpenAI.

**Features:** add, edit, delete, duplicate, model management, expose to pi agent, proxy badge, provider ID display, search/filter.

```bash
pi-switch provider list              # List all provider profiles
pi-switch provider show <name>       # Show profile details
pi-switch provider add <name> [--preset <preset>]
pi-switch provider delete <name>     # Delete profile
pi-switch provider duplicate <name> [--as <new-name>]
pi-switch provider models <name> <model-ids...>  # Update provider's model list
pi-switch provider expose <name> <model-ids...>  # Expose models to pi agent
pi-switch provider fetch-models <name>           # Fetch available models from API
```

### 💡 Built-in Presets

Ready-to-use provider templates with pre-configured API endpoints and models.

```bash
pi-switch presets list               # List all presets
pi-switch presets show <id>          # Show preset detail
```

In the TUI: Presets → Enter to create a profile from a preset template.

### ⚙️ Configuration Management

Manage config backups, imports, and exports with encryption.

```bash
pi-switch config show                # Display full config
pi-switch config path                # Show config file path
pi-switch config backups             # List backup files
pi-switch config export <passphrase> # Encrypted export (AES-256-CBC)
pi-switch config import <path> <passphrase>  # Encrypted import
```

### 🌉 Local Proxy

OpenAI-compatible proxy with transparent routing, Anthropic auto-conversion, intelligent failover, and circuit breaker.

```bash
pi-switch proxy target <name>        # Set proxy target profile
pi-switch proxy failover <p1,p2,...> # Set failover chain
pi-switch proxy start  [--host <ip>] [--port <port>] [--daemon]
pi-switch proxy stop
pi-switch proxy status
```

**Transparent Routing:** When a profile is set as proxy target, its baseUrl automatically points to the proxy server. Pi requests are transparently routed through the proxy with intelligent failover.

Endpoints:
- `GET /health`
- `GET /v1/models`
- `POST /v1/chat/completions` (OpenAI → Anthropic auto-conversion)
- `POST /v1/messages` (Anthropic native forwarding)

### 📊 Usage Statistics

Request metrics aggregated from proxy logs.

```bash
pi-switch stats
```

Displays: total requests, success rate, per-provider breakdown, per-model breakdown, average latency.

### 🩺 Diagnostics

```bash
pi-switch doctor
```

Checks: config file existence, models.json validity, JSON structure, profile count, backup directory.

### 🌐 Multi-language Support

Interactive TUI supports English and Chinese. Language is persisted to config.

- Default language: English (set `PI_SWITCH_LANG=zh` for initial Chinese)
- In TUI: ⚙️ Settings → Language → `←→/Space` to switch

### 🖥️ Interactive TUI

```bash
pi-switch tui
```

Full interactive terminal UI built with ratatui:

- **Profiles**: table with proxy badge, provider ID, current marker, add/edit/delete/duplicate/switch/search
- **Presets**: browse preset templates, create profile from preset
- **Proxy**: start/stop daemon, view status with target/failover/listen info
- **Stats**: request metrics by provider and model
- **Backups**: browse config backup history
- **Settings**: language (English / 中文), proxy host/port/target/failover editing

Key bindings:
- `←→` switch between menu and content
- `↑↓ / j k` move selection
- `Enter` open detail / confirm
- `?` help overlay
- `/` filter
- `q` quit

---

## 🏗️ Architecture

### Core Design

- **napi-rs native addon**: Rust core compiled to `.node` binary for Node.js
- **pi-switch config**: `~/.pi-switch/config.json` with profiles, proxy settings, backup metadata
- **pi models.json**: `~/.pi/agent/models.json` — the file pi reads for provider definitions
- **Atomic writes**: Temp file + rename pattern prevents corruption
- **Backup rotation**: Auto-backup on every mutation, stored in `~/.pi-switch/backups/`

### Config Files

- `~/.pi-switch/config.json` — Profiles, current selection, proxy settings
- `~/.pi-switch/backups/` — Timestamped config backups
- `~/.pi/agent/models.json` — pi agent provider registry (written by `pi-switch use`)

### Code Structure

```
pi-switch/
├── bin/pi-switch.js         # CLI entry point
├── index.js                 # NAPI re-exports
├── pi-switch-native.cjs     # NAPI loader (auto platform detection)
├── src-rust/
│   ├── lib.rs               # NAPI function exports
│   ├── config.rs            # Config load/save, types
│   ├── ops.rs               # Core operations (use/upsert/remove/duplicate)
│   ├── presets.rs           # Built-in provider presets
│   ├── daemon.rs            # Proxy daemon management
│   ├── stats.rs             # Request log aggregation
│   └── tui/                 # Interactive terminal UI
│       ├── app.rs           # State machine + key handler
│       ├── form.rs          # Provider form state
│       ├── text_edit.rs     # Readline-style text input
│       ├── theme.rs         # Dracula theme + color fallback
│       ├── route.rs         # Navigation routes
│       ├── i18n.rs          # Bilingual text (English / 中文)
│       └── ui/              # Rendering (chrome/pages/profiles/overlay)
├── package.json
└── Cargo.toml
```

---

## ❓ FAQ

<details>
<summary><b>How do I switch pi to a different provider?</b></summary>

<br>

```bash
pi-switch use <name>
```

This updates `~/.pi/agent/models.json` so pi picks up the new provider. If pi is already running, use `/model` inside pi to refresh.

Alternatively: open the TUI, navigate to Profiles, and press `Space` on any profile.

</details>

<details>
<summary><b>How do I add a custom provider?</b></summary>

<br>

**CLI:**
```bash
pi-switch provider add my-provider --api openai-completions --base-url https://api.example.com/v1 --api-key '$MY_API_KEY' --model gpt-4
```

**TUI:** Profiles → `a` → fill in form → `Ctrl+S`

</details>

<details>
<summary><b>What does the [proxy] badge mean?</b></summary>

<br>

A profile with the `[proxy]` badge has `"proxy": true` in its config. This means it's configured to route through the local proxy. The proxy can auto-convert between OpenAI and Anthropic formats and apply failover/circuit-breaker policies.

</details>

<details>
<summary><b>How do I set up failover?</b></summary>

<br>

In the TUI: ⚙️ Settings → Failover chain → `Enter` → enter comma-separated profile names → `Enter` to save.

Or directly in `~/.pi-switch/config.json` under `settings.proxy.failover`.

</details>

<details>
<summary><b>Where is my data stored?</b></summary>

<br>

All pi-switch data is under `~/.pi-switch/`. pi's own provider registry is `~/.pi/agent/models.json`. No data is sent anywhere.

</details>

---

## 🛠️ Development

### Requirements

- **Node.js**: >= 20
- **Rust**: 1.80+ ([rustup](https://rustup.rs/))
- **npm**: bundled with Node.js

### Commands

```bash
cd pi-switch

npm run build:native:debug      # Build Rust native addon (debug)
npm run build:native            # Build Rust native addon (release)
node bin/pi-switch.js tui       # Run TUI

cargo build                     # Rust-only build
cargo clippy                    # Lint
cargo fmt                       # Format
```

---

## 📜 License

MIT
