# AGENTS.md — IG Trading Engine

Shared project instructions for **all AI agents** — read this every session before making changes.

Currently active agents on this project:

| Agent | Tool | Entry point |
|-------|------|-------------|
| Claude | Claude Code CLI | Auto-loads `CLAUDE.md` → reads this file |
| Gemini | Gemini CLI | Auto-loads `GEMINI.md` → reads this file |

For deeper references, read on demand:
- `PROJECT_ARCHITECTURE.md` — module breakdown, interfaces, concurrency model
- `TASK_TRACKER.md` — current task status, bugs, backlog
- `TECH_DEBT_AUDIT.md` — full debt audit and phase history

---

## Project Overview

An autonomous algorithmic trading system for **IG Markets**, built with:
- **Rust engine** (`ig-engine/`) — trading logic, risk management, IG API integration, AI/ML layers
- **Telegram notifications** — trade alerts, daily P&L, risk events

> 📦 A Next.js dashboard exists in `src/` but is **archived** — not maintained. Focus is bot engine + Telegram only.

**Current status:** Phases 1–17 + 14.A–I complete. Demo mode live and trading. M15 dual-timeframe scheme fully operational. Regime cooldown system active (7-day VOLATILE → relaxed SL/TP). Concurrent multi-position mode: up to 3 positions per instrument at 1/3 size each. RL position sizing (8.6) is the only long-term remaining item.

---

## Repository Layout

```
ig-trading/
├── ig-engine/                  # Rust trading engine (Tokio async)
│   ├── src/
│   │   ├── main.rs             # Entry point
│   │   ├── api/                # IG REST + Lightstreamer API layer
│   │   │   ├── rest_client.rs  # IGRestClient — orders, positions, prices
│   │   │   ├── streaming_client.rs  # Lightstreamer WS feed + H1/M15 bar accumulators
│   │   │   ├── traits.rs       # TraderAPI trait (testable abstraction)
│   │   │   └── mock_client.rs  # Mock for integration tests
│   │   ├── engine/
│   │   │   ├── config.rs       # EngineConfig — from config/default.toml
│   │   │   ├── state.rs        # EngineState sub-states incl. H1DirectionBias, bar_accumulator_m15
│   │   │   ├── order_manager.rs # Deal confirmation with retries
│   │   │   ├── backtester.rs   # Historical backtesting
│   │   │   └── event_loop/     # Main async loop
│   │   │       ├── mod.rs      # tokio::select! — timers, M15 refresh, self-heal
│   │   │       ├── analysis.rs # H1 + M15 signal gen → H1 gate → risk gate → execution
│   │   │       ├── handlers.rs # Position monitoring, SL/TP, trailing stop
│   │   │       ├── learning.rs # Adaptive learning snapshot
│   │   │       └── validation.rs # Config pre-flight checks
│   │   ├── indicators/         # SMA, EMA, RSI, MACD, Bollinger, ATR, ADX, Stochastic
│   │   ├── strategy/           # 6 H1 strategies + 3 M15 strategies + EnsembleVoter
│   │   ├── regime/             # VOLATILE/TRENDING/RANGING multipliers from ML classifier
│   │   ├── risk/               # RiskManager (hard gate) + check_trade_m15() + position_sizer
│   │   ├── learning/           # StrategyScorecard + AdaptiveWeightManager
│   │   ├── data/               # CandleStore + BarAccumulator + JSONL disk persistence
│   │   ├── ipc/                # Axum HTTP + WebSocket server (port 9090)
│   │   └── notifications/      # Telegram alerts + command listener
│   ├── config/
│   │   ├── default.toml        # Demo config (active)
│   │   ├── live.toml           # Live config (validated, ready)
│   │   └── live-ramp.toml      # Live ramp-up (USD/JPY only, 0.25% risk)
│   ├── data/
│   │   ├── candles/            # *_HOUR.jsonl + *_MINUTE_15.jsonl (disk-first warmup)
│   │   └── regime_latest.json  # Symlink → ../../data/regime_latest.json (cron-updated)
│   └── Cargo.toml
│
├── scripts/                    # Python AI agents + tools
│   ├── sentiment_agent.py      # Gold news sentiment → SQLite (runs every 15 min via cron)
│   ├── run_regime_classifier.py # ML regime → regime_latest.json (runs every hour via cron)
│   ├── train_regime_classifier.py # Train LightGBM regime model
│   ├── fetch_historical_data.py # Fetch 2yr OHLCV for backtesting
│   ├── backtest.py             # Portfolio backtest with ensemble + trailing stop
│   ├── optimize.py             # Grid search over strategy parameters
│   ├── compare_params.py       # Diff optimize output vs current TOML
│   ├── weekly_reoptimise.sh    # Sunday cron: fetch → optimize → compare → reload
│   └── api_lab.py              # Manual trade tools (list, close, inject)
│
├── src/                        # [ARCHIVED] Next.js dashboard — not maintained
│
├── AGENTS.md                   # ← You are here (shared AI instructions)
├── CLAUDE.md                   # Claude-specific additions
├── GEMINI.md                   # Gemini-specific additions
├── PROJECT_ARCHITECTURE.md     # Deep architecture reference
├── TASK_TRACKER.md             # Live task status + bug log
├── TECH_DEBT_AUDIT.md          # Debt audit + phase history
├── AI_ROADMAP.md               # ML/AI enhancement roadmap (8.1–8.6)
├── LIVE_PREFLIGHT_CHECKLIST.md # Steps before switching to live account
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

**H1 (hourly bar close) — 6 vote sources:**

| Strategy | Indicators | VOLATILE mute |
|---|---|---|
| MA Crossover | EMA 9/21 + EMA200 + ADX | 0.5× |
| RSI Reversal | RSI 14 + divergence | 0.5× |
| MACD Momentum | MACD 12/26/9 | 0.5× |
| Bollinger Reversion | BB 20, 2σ | 0.5× |
| Multi_Timeframe | EMA alignment across TFs, dynamic strength | 0.5× |
| Stochastic_Momentum | %K/%D crossover + ADX/RSI bonuses | **0.8×** |
| Gold_Sentiment | RSS news → LLM score ≥ ±0.55 (Gold only) | **1.0×** |

Consensus: min 3 agree + avg strength ≥ 6.0 → full position. VOLATILE scalp tier: min 2 + avg ≥ 6.0 → 0.5× size.

**M15 (60s refresh, 15-min candles) — 3 vote sources:**

| Strategy | Signal Logic | Active Regimes |
|---|---|---|
| M15_MomentumBurst | M15 RSI 55–75 + MACD expanding + H1 EMA200 | Trending, Volatile |
| M15_EmaMicrotrend | M15 EMA9>EMA21 + slope + H1 EMA21 confirm | Trending, Volatile |
| M15_BollingerReversion | M15 %B<0.05 + RSI<35 + H1 RSI>35 | Ranging only |

M15 size: 0.5× H1. H1 Direction Gate blocks M15 signals contradicting H1 bias. ×1.2 bonus for aligned signals.

Weights auto-adjust every 10 trades via `AdaptiveWeightManager` (rolling 50-trade window).

---

## Risk Rules (Hard Gate — Nothing Bypasses This)

- Max risk per trade: 1% of balance
- Max daily loss: 3% → trading halts
- Max weekly drawdown: 5%
- Max open positions: 9 (3 per instrument × 3 instruments)
- Max positions per instrument: 3 (each 1/3 normal size — same total risk)
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
# Rust engine (main)
cd ig-engine && cargo run --release # http://localhost:9090

# Docker
docker-compose up --build           # Engine + PostgreSQL + Redis
```

---

## Internal API (port 9090)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/health` | Liveness probe |
| GET | `/api/status` | Engine status + daily stats |
| GET | `/api/positions` | Open positions |
| GET | `/api/trades` | Trade history |
| GET | `/api/signals` | Recent signals |
| GET | `/api/config` | Current config |
| POST | `/api/start` | Start engine |
| POST | `/api/stop` | Stop engine |
| POST | `/api/pause` | Pause trading |
| POST | `/api/trigger` | Manual trade `{ epic, direction }` |
| GET | `/api/learning` | Adaptive weight state |
| WS | `/api/ws` | Real-time EngineEvent stream |

---

## Testing

```bash
cd ig-engine
cargo test                       # 76 tests passing
cargo clippy -- -D warnings      # must exit 0
cargo fmt --check
```

---

## Hard Rules for All Agents

These apply regardless of which AI tool is being used:

1. **Never call `open_position()` without going through `RiskManager`** — no exceptions.
2. **Never commit `.env`** — secrets stay local.
3. **Zero `.unwrap()` Policy:** Strictly forbidden in the codebase. Use `?`, `.expect("reason")`, or proper error handling. In tests, `.expect("context")` is mandatory over `.unwrap()` to provide failure rationale.
4. **Never hardcode pip values or instrument specs** — they live in `config/default.toml → [risk.instrument_specs]`.
5. **Never use `println!` in Rust** — use `tracing` macros (`info!`, `warn!`, `error!`).
6. **Hold `Arc<RwLock<EngineState>>` locks minimally** — always drop before `.await`.
7. **`test_ig_trade.py` is a debugging artefact** — do not modify or treat as production code.
8. **Do not modify `config/default.toml` defaults when working on features** — use a local override instead.
9. **`cargo clippy -- -D warnings` must exit 0** — zero warnings policy enforced since Phase 8.7.
10. **Doc-Update Protocol — mandatory after every code change:**
    - Check AGENTS.md FIRST (auto-loaded — stale facts here corrupt every future session). Ask: "Does AGENTS.md state something now wrong?" If yes, fix it before anything else.
    - Always update TASK_TRACKER.md (flip status, update header, move fixed bugs).
    - Only update PROJECT_ARCHITECTURE.md if architecture changed; TECH_DEBT_AUDIT.md if debt resolved; AI_ROADMAP.md if Phase 8.x changed.
    - See CLAUDE.md / GEMINI.md for the full checklist and operational commands.

---

## Current Status

Phases 1–15 + 14.A–I complete. Engine live on demo account, actively trading VOLATILE regime via M15 scalp tier. Key systems:
- **H1**: 6 strategies, VOLATILE scalp tier (need 2), regime file from ML cron
- **M15**: 3 strategies, H1 direction gate + ×1.2 alignment bonus, tick-built candles (BarAccumulator), disk-first warmup
- **Telegram**: send + receive (startup ping, /status, /positions commands)
- **Only long-term remaining**: 8.6 RL position sizing (needs 3+ months live data)

See `TASK_TRACKER.md` for full phase history and open bugs.
