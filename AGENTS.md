# Oxiroute Agent Guide 🦀

> **Single Source of Truth for AI Coding Agents.**
> This file defines the architecture, project rules, conventions, and engineering roadmap for **Oxiroute** — the pure, ultra-fast, zero-bloat Rust rewrite of [OmniRoute](https://github.com/diegosouzapw/OmniRoute).

---

## 1. Project Mission & Philosophy

OmniRoute is a feature-rich, multi-provider AI gateway with 350+ providers, intelligent combos, and 3-layer resilience. However, its TypeScript/Node.js/Next.js foundation suffers from:
- **Massive Memory Footprint:** 500MB–800MB+ RSS due to V8 heap allocation, SSR, and unoptimized JSON streaming thrash.
- **Runtime Bloat:** Hundreds of megabytes of `node_modules` and heavy ecosystem dependencies.
- **Garbage Collection Pauses:** SSE token chunks repeatedly parsed and serialized, causing latency jitter.

**Oxiroute** rebuilds OmniRoute's entire core logic in **pure, idiomatic Rust**:
1. **Ultra-Low Memory:** `< 10 MB` idle RAM (vs 500MB+ in Node.js).
2. **Single Static Binary:** `< 5 MB` stripped release binary (`oxiroute`) with zero external runtime dependencies.
3. **Zero-Copy Streaming:** Sub-millisecond SSE token delivery using `bytes`, `eventsource-stream`, and zero unnecessary allocations on the hot path.
4. **Lock-Free Concurrency:** Non-blocking rate-limiting, circuit breakers, and connection state management using atomics and `DashMap`.
5. **100% OmniRoute Protocol Fidelity:** Full support for Direct Route format translations (OpenAI, Anthropic, Gemini, Kiro), 3-tier resilience, session pools with fingerprint rotation, and token compaction.

---

## 2. Workspace Architecture

Oxiroute is structured as a modular **Cargo Workspace**:

```text
oxiroute/
├── Cargo.toml                      # Workspace manifest and shared dependencies
├── AGENTS.md                       # Architectural guide & agent conventions
└── crates/
    ├── oxiroute-core/              # Standard types, traits, error definitions
    │   ├── src/
    │   │   ├── types/
    │   │   │   ├── message.rs      # Unified Message, Role, ContentPart, ToolCall, Thinking
    │   │   │   ├── request.rs      # ChatRequest, FormatType, ThinkingConfig
    │   │   │   ├── response.rs     # ChatResponse, ChatChunk, Delta, Usage
    │   │   │   └── provider.rs     # ProviderConfig, Capabilities, ModelSpec, AuthType
    │   │   ├── error.rs            # OxiError and Result type definitions
    │   │   └── lib.rs
    │
    ├── oxiroute-translator/        # Protocol & Streaming Format Converter
    │   ├── src/
    │   │   ├── openai.rs           # Standard OpenAI ChatCompletions schema
    │   │   ├── anthropic.rs        # Anthropic Messages API (system extraction, tool blocks)
    │   │   ├── gemini.rs           # Google Gemini contents/parts schema
    │   │   ├── kiro.rs             # Direct Route translation for Kiro / AWS SSO
    │   │   └── lib.rs              # RequestTranslator & ResponseChunkTranslator traits
    │
    ├── oxiroute-engine/            # Routing, Combos, 3-Layer Resilience & Compactor
    │   ├── src/
    │   │   ├── resilience/
    │   │   │   ├── classification.rs # 11-type failure classifier (rate limit vs quota vs 5xx)
    │   │   │   ├── circuit.rs        # 4-state Circuit Breaker (Closed, Degraded, Open, HalfOpen)
    │   │   │   └── lockout.rs        # Model-level lockout with exponential backoff
    │   │   ├── combos/
    │   │   │   ├── selector.rs       # Capability filter (Vision, PDF, Tools)
    │   │   │   └── strategies.rs     # Priority, Round-Robin, P2C, Reset-Aware
    │   │   ├── compactor/            # RTK (Tool-result pruning) + Caveman prompt compaction
    │   │   └── lib.rs                # Core routing engine state
    │
    ├── oxiroute-auth/              # Session Pools, Credential Health & OAuth
    │   ├── src/
    │   │   ├── session_pool.rs       # Multi-account lease isolation & FingerprintRotator
    │   │   ├── health_probe.rs       # Background proactive credential health scheduler
    │   │   ├── providers/            # Antigravity (Google), Kiro, Copilot, Kimi, xAI
    │   │   └── lib.rs
    │
    └── oxiroute-server/            # Axum HTTP/SSE Web Server & CLI Binary
        └── src/
            └── main.rs             # CLI args (default port 20200), /health, /v1/models, /v1/chat/completions
```

---

## 3. Core Principles & Hard Rules for AI Agents

1. **Zero-Copy on the Hot Path:**
   - Never deserialize and re-serialize SSE stream chunks unless format translation is strictly required.
   - Use `bytes::Bytes` and borrowed string slices (`&str`) wherever possible to prevent heap churn.
2. **Never Drop Reasoning/Thinking Metadata:**
   - Preserve `<thinking>` tags, `budget_tokens`, and Claude thinking signatures across all translations.
3. **OmniRoute 3-Tier Resilience Model:**
   - **Tier 1 (Connection Cooldown):** Isolate exhausted connections with exponential backoff.
   - **Tier 2 (Model Lockout):** Lock out specific overloaded models without disabling the entire provider connection.
   - **Tier 3 (Provider Circuit Breaker):** Fast-fail traffic for dead upstreams (`CLOSED` -> `DEGRADED` -> `OPEN` -> `HALF_OPEN`).
4. **Proactive Probing & Session Diversity:**
   - Rotate browser fingerprints (`Sec-CH-UA`, `User-Agent`) for web sessions as defined in OmniRoute's `FingerprintRotator`.
   - Re-authenticate before tokens expire, not mid-stream during user requests.
5. **No Runtime Dependencies:**
   - No Node.js, no Python, no external daemon requirements. All static web UI assets will be compiled directly into the binary using `rust-embed`.

---

## 4. Development & Build Commands

```bash
# Check code across the entire workspace
cargo check

# Run unit and integration tests
cargo test

# Build optimized release binary
cargo build --release

# Run Oxiroute on default port 20200
./target/release/oxiroute --port 20200
```

---

## 5. API Endpoints Contract

- `GET  /health` : Live health check probe.
- `GET  /v1/models` : OpenAI-compatible model catalog and active combos.
- `POST /v1/chat/completions` : Core OpenAI-compatible streaming/non-streaming completions router.
- `POST /v1/messages` : Anthropic Messages API compatibility endpoint.
