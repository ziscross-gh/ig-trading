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

**Token discipline (keep agent sessions cheap):**
1. For engine health/status checks run `scripts/engine_status.sh [YYYY-MM-DD]` — one call
   replaces the process-check + fills + closes + P&L + error grep chain. Don't hand-grep the log
   until the digest shows something worth drilling into.
2. `TASK_TRACKER.md`: read the **header only** (first ~40 lines) for current state. It now holds
   only recent phases — older history lives in `docs/PHASE_HISTORY.md`, load on demand.
3. This file is the single always-loaded brief. If you find a fact here that's wrong, fixing it is
   the highest-leverage edit you can make (stale facts here corrupt every future session).

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

Consensus (config/default.toml): H1 min 3 agree + avg strength ≥ 7.5 → full position. VOLATILE scalp tier: min 2 → 0.5× size.

**M15 (60s refresh, 15-min candles) — 3 vote sources:**

| Strategy | Signal Logic | Active Regimes |
|---|---|---|
| M15_MomentumBurst | M15 RSI 55–75 + MACD sign + H1 EMA200 (17.E: expansion is a strength penalty, not a gate) | Trending, Volatile, Ranging-extremes |
| M15_EmaMicrotrend | M15 EMA9>EMA21 + slope + H1 EMA21 confirm + RSI 30/70 exhaustion guard (17.E) | Trending, Volatile |
| M15_BollingerReversion | M15 %B<0.05 + RSI<35 + H1 RSI>35 | Ranging only |

M15 consensus: min 2/3 + avg strength ≥ 6.5 (VOLATILE relaxes 2→1). Size: 0.5× H1.
H1 Direction Gate blocks M15 signals contradicting H1 bias; VOLATILE H1-zero bypass lets
strength ≥ 8 signals through when H1 has no votes. ×1.2 bonus for aligned signals.
Per-instrument M15 SL/TP overrides (17.F/G): `[strategies.instrument_overrides."<epic>"]`
`m15_atr_sl_multiplier`/`m15_atr_tp_multiplier` — EURUSD and USDJPY ship 2.5×/6.5× (whipsaw
protection); TP must stay ≥ min_risk_reward × SL or the risk gate rejects (guarded by
`tests/config_load.rs`). Same-instrument entries are spaced ≥ 45 min apart
(`m15_min_entry_spacing_secs`, 17.G — stacked entries die together).

> ⛔ **Parameter freeze until 2026-07-03** (see TASK_TRACKER 17.G): no strategy/risk/gate tuning;
> observe + propose only. Bug fixes exempt.

Weights auto-adjust every 10 trades via `AdaptiveWeightManager` (rolling 50-trade window).

---

## Risk Rules (Hard Gate — Nothing Bypasses This)

Values below mirror `config/default.toml` — if they disagree, the TOML wins; fix this file.

- Max risk per trade: 1% of balance
- Max daily loss: 2% → trading halts
- Max daily trades: 20
- Max weekly drawdown: 5%
- Max open positions: 9 (3 per instrument × 3 instruments)
- Max positions per instrument: 3 (each 1/3 normal size — same total risk)
- Max margin usage: 30%
- Min risk/reward: 2.5 (⚠️ any SL widening must scale TP or the gate silently rejects)
- Circuit breaker (Phase 17.H — wired into the live gate; was **dead** before 2026-06-16):
  halts new entries for the rest of the SGT day when consecutive losses reach
  `circuit_breaker.consecutive_losses_pause` (5) **or** daily P&L breaches `max_daily_loss_pct`
  (2%). Set by `EngineState::update_circuit_breaker()` on every close; gates `can_trade()`.
  Clears on a winning close (streak resets) or the 00:00 UTC daily reset.
- Trading hours: 07:00–20:00 UTC — **source of truth is `[trading_hours]` in default.toml**
  (the engine overwrites `risk.trading_hours_utc` from it at startup; don't set it under `[risk]`)
- Sessions: Asia / London / US Overlap
- Guaranteed stops required (limited-risk account)
- Position sizing: quarter-Kelly (`sizing_method` in TOML)
- BE-snap (VOLATILE-birth trades): SL → breakeven at 90% of trail distance
  (`volatile_breakeven_trigger = 0.9`, Phase 17.F — 0.7 sterilized USDJPY to 7/7 scratches)

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

## Testing — match CI exactly before every push

```bash
cd ig-engine
cargo fmt -- --check
cargo clippy -- -D warnings      # must exit 0
cargo test                       # 84 tests (incl. tests/config_load.rs — parses the real default.toml)
cargo audit
cargo deny check                 # CI runs BOTH audit and deny — deny escalates unsound advisories
```

> Lesson (06-09): a push verified with `cargo audit` alone still failed CI because `cargo deny`
> flags advisories audit doesn't. Always run the full set above.

---

## Live Ops Runbook (don't re-derive these)

- **Service:** launchd `com.igengine.plist`. Restart:
  `launchctl unload ~/Library/LaunchAgents/com.igengine.plist && launchctl load ~/Library/LaunchAgents/com.igengine.plist`
  (engine re-auths, warms M15 from disk, syncs open positions from IG — safe with positions open)
- **Feed watchdog (autonomous, live 2026-06-30):** launchd `com.igengine.feedwatchdog.plist` runs
  `scripts/feed_watchdog.sh` every 2 min — auto-restarts the engine on a `⚠️ STALE` feed during
  market hours, **independent of any Claude/agent loop**. Install: copy
  `scripts/com.igengine.feedwatchdog.plist` → `~/Library/LaunchAgents/` then `launchctl load` it.
  Audit log: `/tmp/ig-feed-watchdog.log`. Thresholds: >22 min stale → restart; 20-min cooldown.
- **Log:** `/tmp/ig-engine-launchd.log` — JSON lines (`timestamp`, `level`, `fields.message`)
- **Status digest:** `scripts/engine_status.sh [YYYY-MM-DD]` — process + API snapshot + day digest
  (fills, closes with per-instrument P&L, M15 consensus histogram, gate blocks, 17.F markers, errors)
- **Recurring monitoring:** standing loop instructions in `docs/MONITORING.md` — recurring prompts
  should reference it plus a short context delta instead of restating the rules each cycle
- **HTTP API (authoritative state):** `curl -s localhost:9090/api/status` / `/api/positions`
- **Key log markers:** fills `Trade execution confirmed` · approvals `Trade approved:` ·
  closes + P&L `OPU P&L recomputed` (`pnl=` suffix) · per-bar telemetry `Bar analysis: N/3 fired` ·
  gate `H1 direction gate` / `H1-zero bypass` · 17.F `instrument SL/TP override`
- **Known noise (do NOT escalate):** 403 `exceeded-account-historical-data-allowance` = weekly
  REST quota — Phase 17.D backs off 60 min and builds M15 bars from live ticks; Telegram send errors.
- **Gotchas:** `data/regime_latest.json` RSI is H1/daily-based — NOT the M15 RSI strategies use,
  never treat it as a trade predictor. M15 bars close on :00/:15/:30/:45 — no `Bar analysis` lines
  for <18 min is normal, not a stall. Deal sizes are rounded to instrument precision at the single
  execution choke point `order_manager.rs::execute_trade` (Fix #6).
- **Stale-feed outage (2026-06-24, recurred 2026-06-30):** the Lightstreamer tick feed can die
  silently at the weekend close and NOT auto-reconnect at the Sunday reopen (half-open socket;
  auth/tokens stay healthy, so it's not obvious) — the engine runs "alive" but blind for days (no
  bars → no signals → no trades). `engine_status.sh` flags `⚠️ STALE` (no bar >20 min during market
  hours) but only *detects*; on 06-30 the agent monitoring loop meant to act on it had also stalled,
  so it went unnoticed ~3.5 days. **Durable fix (live 2026-06-30): an autonomous OS-level watchdog**
  — `scripts/feed_watchdog.sh` + launchd `com.igengine.feedwatchdog` (every 2 min, independent of
  any agent loop) auto-restarts the engine when a bar is >22 min stale during market hours (20-min
  cooldown anti-flap; no-op when closed; log `/tmp/ig-feed-watchdog.log`). Feed-liveness is now
  machine-guaranteed; manual restart is the fallback. **Lesson: the agent monitoring loop is NOT a
  reliable safety net (it has stalled repeatedly) — safety-critical recovery must run machine-local.**
  Durable *in-engine* auto-reconnect-on-staleness remains a queued follow-up (deeper root cause;
  risky critical-path code; needs weekend validation).
- **Live-money rule:** NEVER change strategy/risk/gate parameters without explicit human approval —
  propose, show evidence, wait.

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

## Model Routing (which model for which task)

This project runs on **Claude Code CLI** and **Gemini CLI**. Pick the model that matches the task's
risk and context profile — frontier reasoning where a wrong edit can silently break live trading,
cheaper/faster models for mechanical work, large-context models for data crunching.

| Task type | Use | Why |
|-----------|-----|-----|
| Engine/strategy/risk logic (event loop, consensus, regime, order/risk managers) | **Claude Opus** | Multi-file async + borrow-checker reasoning; a wrong edit can kill trading for weeks |
| Live log diagnosis / root-causing stalls | **Claude Opus** (Sonnet if budget-tight) | Needs hypothesis → instrument → verify loops |
| Routine edits, doc sync (TASK_TRACKER/AGENTS/etc.), config tweaks | **Claude Sonnet** | Fast, cheap, deterministic — no deep reasoning needed |
| Python ML pipeline (regime classifier, backtests, feature eng.) | **Gemini** | Large context for data files; cheaper on long numeric tables |
| Bulk log/CSV scanning, big-context summarization | **Gemini** (long context) | Cost-efficient on huge inputs |
| One-off shell / cron / launchd ops | **Claude Sonnet/Haiku** | Mechanical; frontier model unnecessary |

**Rule of thumb:** *Opus for anything touching trade-entry logic or risk; Sonnet for edits/docs/ops;
Gemini for Python ML + large-context data crunching.* Never tune a live strategy gate or risk rule on
a cheaper model without a human sign-off.

---

## Current Status

Phases 1–17.F complete. Engine live on demo, first live fills 2026-06-09 after the 17.E consensus
fixes + Fix #6 (deal-size rounding). First-week P&L is GOLD-carried; EURUSD/USDJPY tuning landed
in 17.F (wider EURUSD SL, BE-snap 0.9) — observing. Key systems:
- **H1**: 6 strategies + sentiment, VOLATILE scalp tier, regime file from ML cron
- **M15**: 3 strategies, 2/3 consensus, H1 gate + VOLATILE bypasses, tick-built candles, disk-first warmup
- **Telegram**: send + receive (startup ping, /status, /positions commands)
- **Long-term remaining**: 8.6 RL position sizing (needs 3+ months live data)

For the live current state always check the **TASK_TRACKER.md header** (first ~40 lines), not this section.
