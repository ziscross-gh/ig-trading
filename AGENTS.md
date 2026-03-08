# AGENTS.md — IG Trading Engine

Shared project instructions for **all AI agents** — read this every session before making changes.

Currently active agents on this project:

| Agent | Tool | Entry point |
|-------|------|-------------|
| Claude | Anthropic Cowork | Auto-loads `CLAUDE.md` → reads this file |
| Gemini | Google Antigravity | Auto-loads this file directly |

For deeper references, read on demand:
- `PROJECT_ARCHITECTURE.md` — module breakdown, interfaces, concurrency model
- `TASK_TRACKER.md` — current task status, bugs, backlog
- `TECH_DEBT_AUDIT.md` — full debt audit and phase history

---

## Project Overview

An autonomous algorithmic trading system for **IG Markets**, built with:
- **Rust engine** (`ig-engine/`) — trading logic, risk management, IG API integration
- **Next.js 16 / React 19 dashboard** (`src/`) — live monitoring and control UI

**Current status:** Phase 5 active. Demo mode fully operational. Live mode in preparation.

---

## Repository Layout

```
ig-trading/
├── ig-engine/                  # Rust trading engine (Tokio async)
│   ├── src/
│   │   ├── main.rs             # Entry point
│   │   ├── api/                # IG REST + Lightstreamer API layer
│   │   │   ├── rest_client.rs  # IGRestClient — orders, positions, prices
│   │   │   ├── streaming_client.rs  # Lightstreamer WebSocket feed
│   │   │   ├── traits.rs       # TraderAPI trait (testable abstraction)
│   │   │   └── mock_client.rs  # Mock for integration tests
│   │   ├── engine/
│   │   │   ├── config.rs       # EngineConfig — from config/default.toml
│   │   │   ├── state.rs        # EngineState and sub-states
│   │   │   ├── order_manager.rs # Deal confirmation with retries
│   │   │   ├── backtester.rs   # Historical backtesting
│   │   │   └── event_loop/     # Main async loop
│   │   │       ├── mod.rs      # tokio::select! over timers + events
│   │   │       ├── analysis.rs # Signal gen → risk gate → execution
│   │   │       ├── handlers.rs # Position monitoring, SL/TP
│   │   │       ├── learning.rs # Adaptive learning snapshot
│   │   │       └── validation.rs # Config pre-flight checks
│   │   ├── indicators/         # SMA, EMA, RSI, MACD, Bollinger, ATR, ADX, Stochastic
│   │   ├── strategy/           # MA Crossover, RSI Reversal, MACD, Bollinger + EnsembleVoter
│   │   ├── risk/               # RiskManager (hard gate) + position_sizer
│   │   ├── learning/           # StrategyScorecard + AdaptiveWeightManager
│   │   ├── data/               # CandleStore ring buffer
│   │   ├── ipc/                # Axum HTTP + WebSocket server (port 9090)
│   │   └── notifications/      # Telegram alerts
│   └── Cargo.toml
│
├── src/                        # Next.js 16 frontend (App Router)
│   ├── app/
│   │   ├── page.tsx            # Main dashboard
│   │   ├── error.tsx           # Global error boundary
│   │   └── api/engine/[...path]/route.ts  # Proxy → port 9090
│   ├── components/dashboard/   # EnginePanel, MarketOverview, PriceChart,
│   │                           # TradeHistory, LearningPanel, EquityCurvePanel,
│   │                           # StrategyLab, setup-panel + setup/
│   ├── context/EngineContext.tsx
│   ├── hooks/useEngine.ts      # Facade → useEngineAPI, useEngineWebSocket,
│   │                           #          useEngineControl, useEngineConfig
│   └── types/ig.ts
│
├── config/default.toml         # All runtime config (mode, markets, risk, strategies)
├── AGENTS.md                   # ← You are here (shared AI instructions)
├── CLAUDE.md                   # Claude-specific additions
├── PROJECT_ARCHITECTURE.md     # Deep architecture reference
├── TASK_TRACKER.md             # Live task status
├── TECH_DEBT_AUDIT.md          # Debt audit + phase history
├── .env / .env.example         # Secrets — never commit .env
├── docker-compose.yml
└── .github/workflows/ci.yml
```

---

## Engine Modes

| Mode | Description |
|------|-------------|
| `paper` | Simulated — no real orders, virtual $10,000 |
| `demo` | Real IG Demo account — real API, fake money |
| `live` | Real IG Live account — **real money, use with extreme care** |

Set via `config/default.toml → [general] mode`.

---

## Markets Traded (defaults)

- `CS.D.EURUSD.CSD.IP` — EUR/USD
- `CS.D.USDJPY.CSD.IP` — USD/JPY
- `CS.D.CFIGOLD.CFI.IP` — Gold (XAU/USD)

---

## Strategy Ensemble

Trade executes only when ensemble reaches **min 2 strategies agreeing** at **avg strength ≥ 6.0/10**.

| Strategy | Indicators | Weight |
|---|---|---|
| MA Crossover | SMA 9/21 + ADX > 25 | 1.0 |
| RSI Reversal | RSI 14 + divergence | 0.9 |
| MACD Momentum | MACD 12/26/9 | 1.0 |
| Bollinger Reversion | BB 20, 2σ | 0.8 |

Weights auto-adjust every 10 trades via `AdaptiveWeightManager` (rolling 50-trade window).

---

## Risk Rules (Hard Gate — Nothing Bypasses This)

- Max risk per trade: 1% of balance
- Max daily loss: 3% → trading halts
- Max weekly drawdown: 5%
- Max open positions: 3
- Max margin usage: 30%
- Min risk/reward: 1.5
- Circuit breaker: size reduction after 3 losses, 60 min pause after 5
- Trading hours: 07:00–20:00 UTC (configurable)
- Sessions: Asia / London / US Overlap
- Guaranteed stops required (limited-risk account)
- Position sizing: Half-Kelly (default)

---

## Environment Variables

```
IG_API_KEY=          # IG Developer Portal
IG_IDENTIFIER=       # IG username
IG_PASSWORD=         # IG password
IG_ACCOUNT_ID=       # Optional — auto-detected
IG_ENVIRONMENT=demo  # demo | live

POSTGRES_USER=ig
POSTGRES_PASSWORD=ig_secret_change_me

TELEGRAM_BOT_TOKEN=  # Optional
TELEGRAM_CHAT_ID=    # Optional

RUST_LOG=info
```

> ⚠️ `.env` must never be committed to git. IG API requires direct internet access — fails in proxied/sandboxed environments with `ProxyError: 403 Forbidden`.

---

## How to Run

```bash
# Frontend only
bun install && bun dev              # http://localhost:3000

# Rust engine only
cd ig-engine && cargo run --release # http://localhost:9090

# Full stack
docker-compose up --build           # Engine + PostgreSQL + Redis
```

---

## Internal API (port 9090)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | Liveness probe |
| GET | `/status` | Engine status + daily stats |
| GET | `/positions` | Open positions |
| GET | `/trades` | Trade history |
| GET | `/signals` | Recent signals |
| GET | `/config` | Current config |
| POST | `/start` | Start engine |
| POST | `/stop` | Stop engine |
| POST | `/pause` | Pause trading |
| POST | `/trigger` | Manual trade `{ epic, direction }` |
| GET | `/learning` | Adaptive weight state |
| WS | `/ws` | Real-time EngineEvent stream |

---

## Testing

```bash
# Rust
cd ig-engine
cargo test
cargo clippy -- -D warnings
cargo fmt --check

# Frontend
npx tsc --noEmit
npm run lint
npm test                # vitest
```

---

## Hard Rules for All Agents

These apply regardless of which AI tool is being used:

1. **Never call `open_position()` without going through `RiskManager`** — no exceptions.
2. **Never commit `.env`** — secrets stay local.
3. **No `.unwrap()` on user-facing paths** — use `?` or return a proper error.
4. **Never hardcode pip values or instrument specs** — they live in `config/default.toml → [risk.instrument_specs]`.
5. **Never use `println!` in Rust** — use `tracing` macros (`info!`, `warn!`, `error!`).
6. **Never use raw `fetch()` in frontend components** — always go through `useEngine()` / `EngineContext`.
7. **TypeScript `any` is banned** — `noImplicitAny: true` is enforced.
8. **Hold `Arc<RwLock<EngineState>>` locks minimally** — always drop before `.await`.
9. **`test_ig_trade*.py` files are debugging artefacts** — do not modify or treat as production code.
10. **Do not modify `config/default.toml` defaults when working on features** — use a local override instead.

---

## Active Work (Phase 5)

| Item | Status | File(s) |
|------|--------|---------|
| Trailing Stop Loss | 🏗️ In Progress | `event_loop/handlers.rs` |
| Session filters / news exclusion | 🏗️ In Progress | `event_loop/validation.rs` |
| Equity Curve panel | 🏗️ In Progress | `EquityCurvePanel.tsx` |
| WebSocket market data (replace polling) | 🏗️ In Progress | `useMarketData.ts`, `streaming_client.rs` |
| Strategy Lab backtesting UI | 🏗️ In Progress | `StrategyLab.tsx` |

See `TASK_TRACKER.md` for full details and backlog.
