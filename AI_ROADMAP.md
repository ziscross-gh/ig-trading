# AI_ROADMAP.md — Autonomous Learning & AI Enhancements
# IG Trading Engine

**Created:** 2026-03-08
**Goal:** Evolve the engine from rule-based to self-improving — keeping classical trend-following as the core,
adding AI layers on top for sentiment signals and autonomous parameter adaptation.

---

## Philosophy

> "Classical trend following has a 50-year track record. AI doesn't replace it — it augments it."
>
> - Core strategy stays rule-based (interpretable, robust)
> - AI adds: better regime detection, news sentiment signals, autonomous re-optimisation
> - No black-box models replacing the ensemble — AI is an additional signal layer only

---

## Roadmap Overview

| Phase | Name | Status | Priority |
|-------|------|--------|----------|
| 8.1 | Walk-Forward Auto Re-optimisation | ✅ Done | 🔴 High |
| 8.2 | Performance-Based Strategy Weighting | ✅ Done | 🔴 High |
| 8.3 | Gold News Sentiment Signal | ✅ Done | 🟠 Medium |
| 8.4 | ML Regime Classifier (replace fixed ADX threshold) | ⏳ Planned | 🟠 Medium |
| 8.5 | Macro Calendar Awareness (CPI, NFP, FOMC) | ✅ Done | 🟡 Low |
| 8.6 | Reinforcement Learning for Position Sizing | ⏳ Planned | 🔵 Long-term |

---

## Phase 8.1 — Walk-Forward Auto Re-optimisation

**What it is:** Every week the engine re-runs `optimize.py` on the most recent 6 months of data and
auto-updates `default.toml` if better parameters are found. This is what institutional CTAs call
"walk-forward analysis" — the engine adapts to the current market regime without manual intervention.

**Why first:** We already have `optimize.py` — this is mostly scheduling + diff logic.

### Flow

```
Every Sunday 00:00 UTC:
  1. fetch_historical_data.py   → refresh data/*.json (latest 6 months)
  2. optimize.py                → grid search → find best params per instrument
  3. compare_params.py (new)    → diff new vs current default.toml
  4. if improvement > 5% Sharpe → auto-patch default.toml
  5. engine restarts with new params (graceful reload)
  6. Telegram notification:
       "🤖 Auto-optimise complete
        GOLD: SL 1.0→1.0, TP 5.0→6.0 (+2.1% Sharpe)
        USDJPY: no change
        EURUSD: ADX max 25→20 (+0.8% Sharpe)"
```

### Files to Create/Modify

| File | Action | Description |
|------|--------|-------------|
| `scripts/compare_params.py` | Create | Diffs optimize output vs current TOML, applies if better |
| `scripts/weekly_reoptimise.sh` | Create | Shell wrapper: fetch → optimise → compare → reload |
| `ig-engine/src/engine/config.rs` | Modify | Add hot-reload support (watch config file for changes) |
| `TASK_TRACKER.md` | Update | Track status |

### Safety Guards

- Never change `max_risk_per_trade` or `max_daily_loss_pct` automatically
- Only update: `sl_pct`, `tp_pct`, `adx_range_max`, `trail_dist_pct`, `trail_act_pct`
- Require Sharpe improvement > 5% before applying (don't over-optimise on noise)
- Keep last 4 weeks of `default.toml` backups (`config/snapshots/YYYY-MM-DD.toml`)
- Telegram alert on every change — human can override

---

## Phase 8.2 — Performance-Based Strategy Weighting

**What it is:** Track each strategy's rolling win rate over the last 20 trades. Dynamically adjust
ensemble weights — reward strategies that are working, reduce weight on those that aren't.

**Why:** Markets rotate between trending and ranging. If RSI_Reversal has lost 4 of last 5 trades,
it's likely mis-fitted to current regime. Reduce its vote weight automatically.

### Design

```rust
// In state.rs — new StrategyPerformance tracker
pub struct StrategyPerformance {
    pub name: String,
    pub recent_trades: VecDeque<TradeOutcome>,  // last 20
    pub rolling_win_rate: f64,
    pub current_weight: f64,
    pub base_weight: f64,
}

// Weight adjustment formula:
// if win_rate > 0.60 → weight = base_weight * 1.2  (boost)
// if win_rate < 0.40 → weight = base_weight * 0.5  (penalise)
// if win_rate 0.40-0.60 → weight = base_weight     (neutral)
```

### Ensemble Vote Change

Current: `min_consensus = 3` (hard count)
New: Sum of weights for agreeing strategies must exceed `min_weight_threshold = 2.5`

### Files to Modify

| File | Change |
|------|--------|
| `ig-engine/src/engine/state.rs` | Add `StrategyPerformance` tracker per epic |
| `ig-engine/src/engine/event_loop/analysis.rs` | Use weighted consensus instead of count |
| `ig-engine/src/engine/event_loop/handlers.rs` | Update `StrategyPerformance` on trade close |
| `ig-engine/config/default.toml` | Add `min_weight_threshold` config param |

---

## Phase 8.3 — Gold News Sentiment Signal

**What it is:** A background process reads Gold-relevant news headlines every 15 minutes, scores
sentiment (Bullish / Bearish / Neutral) using an LLM, and feeds the result as a 5th signal into
the ensemble vote. Gold is uniquely sensitive to macro news (inflation, rates, geopolitics).

**Why Gold only:** EURUSD/USDJPY react to news too but are more correlated to technical levels.
Gold is the clearest news-driven asset in our portfolio.

### Architecture

```
[NewsAPI / RSS Feed]
       ↓ (every 15min)
[sentiment_agent.py]
  - Fetch last 10 Gold headlines
  - Score with LLM (local Ollama OR Claude API)
  - Output: {score: -1.0 to +1.0, confidence: 0-1, keywords: [...]}
       ↓
[Redis / SQLite sentiment store]
       ↓
[Rust engine reads sentiment at analysis time]
  - If score > 0.6 → Bullish signal (weight 0.8)
  - If score < -0.6 → Bearish signal (weight 0.8)
  - If |score| < 0.6 → Neutral (no vote)
```

### LLM Options (Cost vs Quality)

| Option | Cost | Latency | Privacy | Notes |
|--------|------|---------|---------|-------|
| **Ollama + llama3** (local) | Free | ~2s | ✅ Full | Best for self-hosted. Runs on Mac M-series. |
| **Claude API (claude-haiku)** | ~$0.001/call | ~0.5s | Cloud | Cheapest Anthropic API tier. ~$1–2/month |
| **OpenAI GPT-4o-mini** | ~$0.001/call | ~0.5s | Cloud | Similar cost to Haiku |

**Recommendation:** Start with **Ollama + llama3** (free, private, works offline). Upgrade to API if latency matters.

### News Sources (Free)

| Source | Feed | Gold Coverage |
|--------|------|---------------|
| Reuters | RSS `feeds.reuters.com/reuters/businessNews` | ✅ |
| Yahoo Finance | RSS `finance.yahoo.com/rss/headline?s=GC=F` | ✅ |
| Kitco | RSS `kitco.com/rss/` | ✅ Gold-specific |
| FT | RSS (limited) | ✅ |

### Prompt Template

```python
SENTIMENT_PROMPT = """
You are a Gold (XAUUSD) trading signal analyst.
Analyse these headlines and return a JSON sentiment score.

Headlines:
{headlines}

Return ONLY valid JSON:
{{
  "score": <float -1.0 (very bearish) to +1.0 (very bullish)>,
  "confidence": <float 0.0 to 1.0>,
  "reasoning": "<one sentence>",
  "key_drivers": ["<driver1>", "<driver2>"]
}}

Key Gold drivers: USD strength (bearish for Gold), inflation (bullish),
geopolitical risk (bullish), Fed rate hikes (bearish), recession fear (bullish).
"""
```

### Files to Create

| File | Description |
|------|-------------|
| `scripts/sentiment_agent.py` | Polls news, scores with LLM, writes to SQLite |
| `scripts/sentiment_schema.sql` | Schema for sentiment store |
| `ig-engine/src/sentiment/mod.rs` | Rust reader: polls SQLite for latest Gold sentiment |
| `ig-engine/src/engine/event_loop/analysis.rs` | Inject sentiment as 5th signal for Gold epic |

---

## Phase 8.4 — ML Regime Classifier

**What it is:** Replace the fixed `adx_range_max = 25.0` with a trained ML classifier that
dynamically detects the current market regime (Trending / Ranging / Volatile) and adapts
strategy weights accordingly.

**Why:** The ADX threshold of 25 is static — learned from our 2-year backtest. A classifier
trained on recent rolling windows will adapt faster to regime shifts.

### Design

```python
# Features (computed on rolling 20-candle window)
features = [
    adx_14,           # Trend strength
    atr_14_pct,       # Volatility as % of price
    bb_width,         # Bollinger Band width (expansion/contraction)
    rsi_14,           # Momentum
    volume_ratio,     # Volume vs 20-bar average
    price_vs_sma200,  # Position relative to long-term trend
    hurst_exponent,   # H>0.5 = trending, H<0.5 = mean-reverting
]

# Labels (auto-generated from backtest results)
# If trend strategies (MA/MACD) win → TRENDING
# If reversion strategies (RSI/Bollinger) win → RANGING
# If both lose → VOLATILE (reduce all position sizes)

# Model: LightGBM or sklearn GradientBoostingClassifier
# - Train on historical candle data
# - Retrain weekly (walk-forward, Phase 8.1 triggers this too)
# - Output: {regime: "TRENDING"|"RANGING"|"VOLATILE", confidence: 0-1}
```

### Integration with Rust

```
[regime_classifier.py]  →  SQLite  →  [Rust engine reads regime at bar close]
                                        if TRENDING:
                                          - Enable MA + MACD (weight 1.2)
                                          - Disable RSI + Bollinger
                                        if RANGING:
                                          - Disable MA + MACD
                                          - Enable RSI + Bollinger (weight 1.2)
                                        if VOLATILE:
                                          - Reduce all position sizes by 50%
                                          - Increase min_consensus to 4
```

---

## Phase 8.5 — Macro Calendar Awareness

**What it is:** The engine fetches the economic calendar (CPI, NFP, FOMC, BOJ, ECB decisions)
and extends/adjusts the news blackout windows dynamically based on event impact rating.

**Current state:** We have static blackout windows in `default.toml`. This makes them dynamic.

### Data Source

- **ForexFactory calendar API** (free, JSON)
- **Investing.com calendar** (scrape)
- **TradingEconomics API** (free tier)

### Logic

```python
# Event impact mapping
HIGH_IMPACT_EVENTS = ["NFP", "CPI", "FOMC", "BOJ Rate", "ECB Rate", "GDP"]
MEDIUM_IMPACT_EVENTS = ["PMI", "Retail Sales", "Unemployment Claims"]

# Blackout window by impact
impact_window_mins = {
    "HIGH": 30,     # ±30 min (vs current static 15 min)
    "MEDIUM": 15,   # ±15 min
    "LOW": 0,       # No blackout
}

# Write to engine config hot-reload file before event
```

---

## Phase 8.6 — Reinforcement Learning for Position Sizing

**What it is:** A RL agent learns optimal position sizing based on current regime, recent P&L,
and volatility — going beyond the fixed Quarter-Kelly formula.

**Why long-term:** Needs months of live trade data to train meaningfully. Start collecting now.

### Approach

```
Environment: Market state (features from 8.4 + account state)
Action space: Position size multiplier [0.25, 0.5, 0.75, 1.0, 1.25, 1.5]
Reward: Risk-adjusted P&L (Sharpe-weighted)
Algorithm: PPO (Proximal Policy Optimisation) — stable for finance

Framework: stable-baselines3 + custom TradingEnv
Training data: Live trade logs accumulated over 3–6 months
```

**Trigger:** Start Phase 8.6 after 3 months of live demo data.

---

## Data Collection (Start Now)

To train future ML models, we need structured logs. The engine already logs trades — ensure these
are persisted properly:

| Data | Where | Format | Used For |
|------|-------|--------|----------|
| OHLCV candles | `data/*.json` | JSON | Backtesting, regime classifier |
| Trade outcomes | `logs/trades.jsonl` | JSONL | Strategy weighting, RL training |
| Strategy signals | `logs/signals.jsonl` | JSONL | Win rate tracking |
| Sentiment scores | `data/sentiment.db` | SQLite | Sentiment signal validation |
| Regime labels | `data/regimes.db` | SQLite | Classifier training |

### Minimum Data Before Training

| Model | Minimum Live Data |
|-------|-----------------|
| Walk-forward re-optimise | 3 months of candles |
| Strategy weighting | 20 trades per instrument |
| Sentiment signal | 50 news events with known outcomes |
| Regime classifier | 6 months of labelled candles |
| RL position sizer | 3 months of live trades |

---

## Implementation Order (Recommended)

```
Phase 8.1 → Walk-forward re-optimise  (builds on optimize.py — quickest win)
Phase 8.3 → Gold sentiment signal      (high impact for Gold performance)
Phase 8.2 → Strategy weighting         (needs ~20 live trades first)
Phase 8.4 → ML regime classifier       (needs 6 months data)
Phase 8.5 → Macro calendar             (low effort, medium impact)
Phase 8.6 → RL position sizing         (needs 3+ months live data — last)
```

---

## How This Compares to Institutional AI

| Feature | Two Sigma / Citadel | Our Roadmap |
|---------|---------------------|-------------|
| Alternative data | Satellite, credit card | News sentiment (free) |
| Regime detection | Proprietary ML | Lightweight LightGBM |
| Auto re-optimisation | Daily | Weekly |
| RL execution | Yes | Phase 8.6 (long-term) |
| Sentiment | NLP on Bloomberg Terminal | RSS + local LLM |
| Cost | $millions/yr | ~$0–$5/month |

> **The gap is data cost and compute scale — not approach.**
> Our architecture mirrors what institutional funds do, built for retail capital.

---

## Notes

- All ML models use **walk-forward validation only** — no look-ahead bias
- Models are **additive signals**, never replace core rule-based ensemble
- Every AI decision is logged and Telegram-notified — full transparency
- Hot-reload config means engine never needs to restart for param updates
- Start data collection **immediately** — every trade logged is training data
