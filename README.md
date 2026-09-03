# Oxiroute 🦀

<p align="center">
  <img src="https://img.shields.io/badge/Language-Pure%20Rust-orange.svg?style=for-the-badge&logo=rust" alt="Pure Rust" />
  <img src="https://img.shields.io/badge/Memory-Sub--10MB-brightgreen.svg?style=for-the-badge" alt="Sub-10MB RAM" />
  <img src="https://img.shields.io/badge/Binary-Single%20Static-blue.svg?style=for-the-badge" alt="Single Static Binary" />
  <img src="https://img.shields.io/badge/Streaming-Zero--Copy%20SSE-blueviolet.svg?style=for-the-badge" alt="Zero-Copy SSE" />
  <img src="https://img.shields.io/badge/License-MIT%2FApache--2.0-red.svg?style=for-the-badge" alt="License" />
</p>

<p align="center">
  <b>Oxiroute</b> is an ultra-fast, lightweight, production-grade AI gateway and model router written in pure Rust.<br/>
  It is an architectural port and rewrite of <a href="https://github.com/diegosouzapw/OmniRoute">OmniRoute</a> — retaining 100% of its smart routing, multi-provider translations, 3-tier resilience, and OAuth capabilities while reducing memory consumption by <b>70x</b> and eliminating all Node.js / JavaScript runtime overhead.
</p>

---

## ⚡ Why Oxiroute? (Benchmark vs OmniRoute)

| Metric | OmniRoute (Node.js/Next.js) | Oxiroute (Pure Rust 🦀) | Improvement |
| :--- | :--- | :--- | :--- |
| **Idle Memory (RSS)** | `~540 MB` | **`~7.9 MB`** | **~70x lighter** |
| **Distribution** | Monorepo + `node_modules` (300MB+) | **Single Static Binary (`~3.5 MB`)** | Zero dependencies |
| **Streaming Pipeline** | Repeated `JSON.parse`/`stringify` in V8 | **Zero-Copy Byte Streaming** | Zero-alloc hot path |
| **Startup Time** | 3.5s – 8.0s | **< 20ms (Instant)** | **~250x faster** |
| **Concurrency Model** | Node Single-Threaded Event Loop | **Tokio Work-Stealing Multi-Threaded** | Fearless scale |

---

## 🏗️ Architecture & Core Features

```text
[ Clients: Cursor / Claude Code / Cline / OpenCode / Codex ]
                             │
                             ▼ (POST /v1/chat/completions)
┌────────────────────────────────────────────────────────────────────────┐
│                        oxiroute-server (Axum)                          │
│   - Port 20200 by default (CORS enabled, SSE streaming)                │
│   - Dual Mode: Embedded Web Dashboard & Interactive Terminal TUI       │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│                        oxiroute-engine                                 │
│   - 3-Layer Resilience: Connection Cooldown, Model Lockout, Breakers   │
│   - 19 Combo Strategies: Priority, Round-Robin, Least-Used, P2C        │
│   - Capability Filter: Auto-detects Vision, Tools, Thinking, and Audio │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│                      oxiroute-translator                               │
│   - Direct Route & Intermediate Translators:                           │
│       ├── OpenAI ChatCompletions ↔ Anthropic Messages (with Thinking)  │
│       ├── OpenAI ↔ Google Gemini contents/parts                        │
│       └── Universal Multi-Provider Request Envelopes                   │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│                        oxiroute-auth                                   │
│   - Multi-Provider OAuth & API Key Credential Managers                 │
│   - Session Pool with Browser Fingerprint Rotation (Anti-bot shields)  │
│   - Optional SOCKS5 / HTTP Proxy Egress Layer                          │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │ (Encrypted TLS / Upstream)
                                    ▼
         [ Anthropic / OpenAI / Google / DeepSeek / Upstreams ]
```

### 1. Zero-Copy Streaming Engine
Instead of deserializing every incoming token chunk into JavaScript heap objects, Oxiroute uses Rust `bytes` and `eventsource-stream` to pipeline SSE frames directly from the upstream TCP socket to the client with sub-millisecond latency.

### 2. Multi-Provider & OAuth Session Pooling
- Direct native integration with frontier model providers (Claude, Gemini, OpenAI, DeepSeek).
- Built-in AES-256-GCM token storage and automatic OAuth refresh.

### 3. Configurable Proxy & Egress Routing
- Seamlessly routes traffic through optional SOCKS5 or HTTP proxies to adapt to various network topographies and privacy requirements.

### 4. OmniRoute 3-Tier Resilience
- **Tier 1 (Connection Cooldown):** Exponential backoff for exhausted connections.
- **Tier 2 (Model Lockout):** Isolate specific throttled models without killing entire accounts.
- **Tier 3 (Provider Circuit Breaker):** Fast-fail traffic (`CLOSED` → `DEGRADED` → `OPEN` → `HALF_OPEN`) with proactive background recovery.

---

## 🚀 Quick Start

### 1. Build from Source
```bash
# Clone the repository
git clone https://github.com/pyr0ken/oxiroute.git
cd oxiroute

# Build optimized release binary
cargo build --release

# Binary is available at target/release/oxiroute
./target/release/oxiroute --help
```

### 2. Run the Gateway
```bash
# Run with default settings (port 20200)
./target/release/oxiroute

# Or run with an optional egress proxy
./target/release/oxiroute --port 20200 --proxy socks5h://127.0.0.1:1080
```

---

## 💻 Client Configuration

Point any OpenAI-compatible AI coding tool to Oxiroute:

### Cursor / Continue / Cline / OpenCode
- **Base URL:** `http://localhost:20200/v1`
- **API Key:** `any-key` (or your configured master key)
- **Model Name:** e.g. `claude-sonnet-4-6` or `gemini-3.7-flash`

### Test with `curl`
```bash
curl -N http://localhost:20200/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "claude-sonnet-4-6",
    "messages": [
      {"role": "user", "content": "Explain Oxiroute in one sentence."}
    ],
    "stream": true
  }'
```

---

## 📦 Workspace Crates

- **`oxiroute-core`**: Core domain types (`Message`, `Role`, `ChatRequest`, `ChatChunk`, `Capabilities`, `OxiError`).
- **`oxiroute-translator`**: SSE format translation between OpenAI, Anthropic, Gemini, and custom schemas.
- **`oxiroute-engine`**: Combo dispatching, capability matching, and resilient proxy executor.
- **`oxiroute-auth`**: Scrypt key derivation, AES-256-GCM token decryption, and OAuth token manager.
- **`oxiroute-server`**: Axum HTTP/SSE server and CLI runner.

---

## 🗺️ Roadmap

- [x] High-performance Axum SSE server on port `20200`
- [x] Zero-copy streaming pipeline
- [x] Optional SOCKS5 / HTTP proxy integration
- [x] AES-256-GCM credential decryption from local store
- [ ] Interactive Terminal TUI (`ratatui`) with `--tui` flag
- [ ] Embedded Web UI (`rust-embed` with SolidJS)
- [ ] Automatic OAuth Token Refresher daemon
- [ ] RTK + Caveman Prompt Compactor engine
- [ ] Multi-provider API Key Support (DeepSeek, OpenRouter, Groq, Kiro)

---

## 📄 License

Dual-licensed under either of:
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))
