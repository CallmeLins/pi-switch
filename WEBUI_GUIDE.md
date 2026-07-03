# pi-switch WebUI Guide

pi-switch now offers **three ways** to manage the same configuration:

| Interface | Entry | Lives in |
|-----------|-------|----------|
| **CLI**   | `pi-switch <cmd>`        | `bin/pi-switch.js` → napi |
| **TUI**   | `pi-switch tui`          | `src-rust/tui/` (ratatui) |
| **WebUI** | `pi-switch webui start`  | `src-rust/web.rs` (axum) + `webui/` (React) |

All three are **thin adapters over the same Rust core** (`src-rust/ops.rs` +
`src-rust/config.rs` + `src-rust/service.rs`). No business logic lives in the UI
layers, so behaviour stays identical across them.

---

## Architecture

```
                 ┌──────────── shared Rust core ────────────┐
   CLI (node) ──►│  service.rs  (reads / shaping)           │
   TUI  (rust) ─►│  ops.rs      (mutations)                 │──► ~/.pi-switch/config.json
   WebUI(axum) ─►│  config.rs   (data model + load/save)    │──► ~/.pi/agent/models.json
                 └──────────────────────────────────────────┘
        ▲                    ▲                      ▲
   bin/pi-switch.js     src-rust/tui/          src-rust/web.rs  ← REST /api/*
   (napi bindings                              + embedded webui/dist (rust-embed)
    in lib.rs)
```

The WebUI backend is an **axum server** that:
1. serves the React SPA (compiled into the `.node` via `rust-embed`), and
2. exposes `REST /api/*` where every route delegates to `ops`/`service`/`daemon`/`sync`.

It runs as a second **daemon-managed service** alongside the proxy (own pid/log/port),
using the generalized machinery in `src-rust/daemon.rs`.

---

## Usage

### Build (frontend + native)

```bash
# one-shot: builds webui/dist then embeds it into the .node
npm run build

# or step by step
npm run build:webui      # vite build → webui/dist
npm run build:native     # napi build --release  (embeds webui/dist)
```

> Stop any running TUI/daemon before `build:native` — they hold a lock on the `.node`.

### Run

```bash
# foreground (Ctrl+C to stop)
pi-switch webui start --port 43110

# background daemon
pi-switch webui start --daemon
pi-switch webui status
pi-switch webui stop
```

Then open `http://127.0.0.1:43110` in a browser. Defaults come from
`settings.web` in `~/.pi-switch/config.json` (host `127.0.0.1`, port `43110`).

### Dev workflow (hot reload)

```bash
# terminal 1 — Rust API server
pi-switch webui start --port 43110

# terminal 2 — vite dev server (proxies /api to :43110)
npm run dev:webui        # http://localhost:43111
```

---

## Security

- **Loopback binds run open** (`127.0.0.1`/`localhost`/`::1`) — intended for local use.
- **Non-loopback binds require HTTP Basic auth** (user `admin`). A password is
  auto-generated on first start and stored in `~/.pi-switch/webui_password`; the
  browser prompts for it natively.
- For public exposure, put the server behind a TLS reverse proxy (Nginx/Caddy/Cloudflare).

---

## Maintainability: adding a new operation (the 4-step recipe)

Because the UIs are thin, a new capability is added **once in the core** and then
wired into each adapter:

1. **Core** — implement the logic in `src-rust/ops.rs` (mutation) or
   `src-rust/service.rs` (read/shape). This is the single source of truth.
2. **CLI** — add a napi wrapper in `src-rust/lib.rs`, export it in `index.js`,
   and add a subcommand in `bin/pi-switch.js`.
3. **WebUI backend** — add one route in `src-rust/web.rs` that calls the core fn.
4. **WebUI frontend** — add a method in `webui/src/api.ts` and use it in the
   relevant panel under `webui/src/components/`.

The TUI (`src-rust/tui/`) already calls the core directly, so it usually needs a
change only if the feature has a TUI screen.

### REST ↔ core map (current)

| Route | Core call |
|-------|-----------|
| `GET /api/state` | `service::get_state` |
| `GET /api/presets` · `/presets/:id` | `service::presets_info` · `show_preset` |
| `GET /api/profiles/:name` | `service::get_profile` |
| `GET /api/doctor` · `/config/validate` | `service::run_doctor` · `config::validate_config` |
| `GET /api/backups` · `/stats` | `service::list_backups` · `service::stats_value` |
| `POST /api/profiles` · `PUT /api/profiles/:name` | `ops::upsert_profile` |
| `DELETE /api/profiles/:name` | `ops::remove_profile` |
| `POST /api/profiles/:name/{duplicate,use,test,fetch-models}` | `ops::{duplicate_profile,use_profile,test_provider,fetch_models}` |
| `PUT /api/profiles/:name/{models,expose,spoof}` | `ops::{update_provider_models,update_exposed_models,set_profile_spoof}` |
| `POST /api/proxy/{start,stop}` | `daemon::daemon_{start,stop}(&PROXY, …)` |
| `PUT /api/proxy/failover` | `ops::set_failover` |
| `PUT /api/settings` | `ops::update_settings` |
| `POST /api/config/{export,import,restore}` | `sync::{encrypt_config,import_config}` · `config::restore_config` |
| `POST /api/init` | `ops::init` |

---

## Type sync (frontend ↔ Rust)

`webui/src/types.ts` is a hand-written mirror of the Rust structs in
`src-rust/config.rs` (the source of truth). Keep them in sync when the config
model changes.

**Future option:** auto-generate `types.ts` from the Rust structs with
[`typeshare`](https://github.com/1Password/typeshare) or `ts-rs` to eliminate drift.
Not wired up yet to keep the toolchain lean.
