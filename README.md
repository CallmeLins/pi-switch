<div align="center">

# pi-switch

[![Version](https://img.shields.io/badge/version-0.2.0-blue.svg)](https://github.com/user/pi-switch/releases)
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
  <h3>Profiles</h3>
  <img src="assets/screenshots/profiles.png" alt="Profiles" width="70%"/>
</div>

<br/>

<table>
  <tr>
    <th>Home</th>
    <th>Settings</th>
  </tr>
  <tr>
    <td><img src="assets/screenshots/home.png" alt="Home" width="100%"/></td>
    <td><img src="assets/screenshots/settings.png" alt="Settings" width="100%"/></td>
  </tr>
</table>

## 🚀 Quick Start

**TUI Mode (Recommended)**
```bash
pi-switch tui
```
Use the full-screen interface to manage providers, browse presets, inspect proxy status, and configure settings.

**Command-Line Mode**
```bash
pi-switch provider list              # List all provider profiles
pi-switch provider add <name> [--preset <id>] [--api-key <key>]  # Add a profile
pi-switch use <name>                 # Switch pi to this profile
pi-switch provider show <name>       # Show profile details
pi-switch provider delete <name>     # Delete a profile
pi-switch presets list               # List built-in provider presets
pi-switch config show                # Display current config
pi-switch config backups             # List backup files
pi-switch stats                      # View proxy request statistics
pi-switch doctor                     # Run environment diagnostics
```

---

## 📥 Installation

### npm (Recommended)

```bash
npm install -g pi-switch
```

### Pi Package

```bash
pi install npm:pi-switch
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

**Features:** add, edit, delete, duplicate, current marker, proxy badge, provider ID display, search/filter.

```bash
pi-switch provider list              # List all provider profiles
pi-switch provider show <name>       # Show profile details
pi-switch provider add <name> [--preset <preset>]
pi-switch provider delete <name>     # Delete profile
pi-switch provider duplicate <name> [--as <new-name>]
pi-switch use <name> [--mode merge|exclusive]  # Switch pi to profile
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

OpenAI-compatible proxy with Anthropic auto-conversion, failover chain, and circuit breaker.

```bash
pi-switch proxy start  [--host <ip>] [--port <port>] [--profile <name>]
pi-switch proxy stop
pi-switch proxy status
```

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
