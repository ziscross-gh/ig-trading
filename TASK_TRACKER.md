# TASK_TRACKER.md — IG Trading Engine

**Last updated:** 2026-06-08 (Phase 17.E — M15 analysis amplification fix + per-strategy FIRE/silent telemetry; root-caused the 1/3-consensus stall)
**Current phase:** Production-ready + Active trading. VOLATILE regime live + cooldown system. Concurrent multi-position mode live. H1 gate dual bypass (cold-start + zero-signal).
**Current focus:** 🤖 Engine live & trading | 📊 Multi-position mode: max 3/instrument at 1/3 size | 🔄 Regime cooldown active (7-day VOLATILE → relaxed SL/TP) | 🕐 Trading hours: 07:00–20:00 UTC only | 🚪 H1 gate: VOLATILE bypass for 0-signal & cold-start

> 📦 Dashboard (`src/`) is **archived** — not maintained. All dashboard tasks removed.

For the full history of completed work and debt items, see `TECH_DEBT_AUDIT.md`.

---

## Phase 17.E — M15 Analysis Amplification Fix + Per-Strategy Telemetry (✅ 2026-06-08)

> **Motivation:** After 17.D restored signals, the M15 ensemble still placed no trades — every bar
> stalled at 1/3 consensus (barrier ≥2 never reached). Needed to (1) stop redundant analysis and
> (2) identify *which* strategies stay silent.

**Fix #1 — O(epics²) → O(epics) analysis amplification** (`event_loop/mod.rs`):
- `analyze_market_m15()` already loops over all epics internally, but it was being called *inside*
  the per-epic refresh loop (3 inline call sites: backoff path, happy path, error fallback).
  → analysis ran ~3× per refresh tick.
- Replaced all three inline calls with `should_analyze_m15 = true;` flag, then a single call after
  the for-loop. Verified: each epic now gets exactly **1 bar analysis per minute** (was ~3).

**Fix #2 — Per-strategy FIRE/silent telemetry** (`event_loop/analysis.rs` ~line 1125):
- Promoted per-strategy result to INFO. Each bar now logs:
  `[M15] [EPIC] Bar analysis: N/3 fired [names] silent [names]`

**Diagnostic result (root cause of the 1/3 stall):**
| Strategy | Status | Why |
|----------|--------|-----|
| `M15_EmaMicrotrend` | ✅ Fires (only one) | Always Sell strength=9.0 — sole active vote |
| `M15_BollingerReversion` | ❌ 100% silent | Hard-gated **RANGING-only**; live regimes are TRENDING (EURUSD, ADX 56.9) / VOLATILE (USDJPY) → returns `None` immediately |
| `M15_MomentumBurst` | ❌ Silent | 4-way compound gate (RSI zone + MACD hist *expanding* + price vs H1 EMA200). RSI is in the bearish zone but MACD histogram isn't expanding in a mature trend → no fire |

> **Conclusion:** Consensus barrier ≥2 was mathematically unreachable in TRENDING/VOLATILE because
> only EmaMicrotrend was ever eligible — Bollinger is regime-locked to RANGING, and MomentumBurst's
> MACD-expansion requirement rarely coincided.

**Fix #3 — option A applied (`m15_momentum_burst.rs`, ✅ 2026-06-08):** Removed the hard MACD-histogram
*expansion* gate (`macd_hist vs prev_macd_hist`). Entry now requires RSI momentum zone + MACD sign +
price vs H1 EMA200. The expansion check survives as a **soft quality score**: a decelerating histogram
subtracts 1.5 from strength (`macd_decelerating` flag) instead of vetoing the signal. Min strength stays
≥5.0 so it remains eligible past `ensemble_signal_floor`. Goal: let MomentumBurst co-fire with
EmaMicrotrend in trends → reach barrier 2 → unblock M15 trades. Engine restarted (PID 25643).

**Fix #4 — near-miss + missing-indicator diagnostics (✅ 2026-06-08):** Added two INFO logs to
`m15_momentum_burst.rs` to root-cause why it stayed silent even after Fix #3. Result: NO missing
indicators; the binding blockers are genuine signal disagreements (per-epic, 2026-06-08 10:15 UTC):

| Epic | RSI | bear[25,45] | MACD hist <0 | price<H1EMA200 | Blocker |
|------|-----|-------------|--------------|----------------|---------|
| GOLD | 25.2 | ✓ | ✗ (+1.18) | ✓ | MACD histogram **positive** (momentum turning up) |
| EURUSD | 24.0 | ✗ (<25) | ✗ (+0.0001) | ✓ | RSI **too oversold** + MACD positive |
| USDJPY | 35.2 | ✓ | ✓ | ✗ (price>EMA200) | **No confirmed downtrend** |

> **Root cause (revised):** MomentumBurst is working *as designed* — it correctly refuses to add a
> Sell vote into exhausted/pulling-back moves. The structural issue is that **EmaMicrotrend has NO
> exhaustion guard** and keeps voting Sell at RSI ~24–25 (selling the bottom). The two strategies
> rarely agree because EmaMicrotrend fires late in exhausted trends where MomentumBurst (rightly)
> abstains. Loosening MomentumBurst further would degrade entry quality / win rate.
>
**Fix #5 — EmaMicrotrend exhaustion guard (✅ 2026-06-08, user-approved option 1):**
`m15_ema_microtrend.rs` now reads M15 RSI and refuses entries in the exhaustion band:
no Sell if RSI < 30, no Buy if RSI > 70 (`EXHAUSTION_OVERSOLD`/`OVERBOUGHT` consts). RSI added to
the reason string. Both temporary MomentumBurst diagnostics (`SILENT` / `MISSING-IND`) removed.

> **Expected behaviour:** the low-quality 1/3 "sell the bottom" signals disappear — when a downtrend
> is exhausted (RSI < 30) EmaMicrotrend now abstains too, so bars read 0/3 instead of 1/3 (no bad
> entries). Genuine 2/3 consensus forms in **healthy mid-trends** (RSI 30–70 with momentum agreeing),
> which is when trades should fire. "No trades right now" while all epics sit at RSI ~24–25 is the
> guard working as intended, not a stall.

**Docs — Model Routing added to AGENTS.md (✅ 2026-06-08):** new "Model Routing" section codifies
Opus = engine/strategy/risk + live diagnosis, Sonnet = edits/docs/ops, Gemini = Python ML + large-context.

**Fix #6 — deal-size rounding before IG submit (✅ 2026-06-09, `order_manager.rs`):** The FIRST live
M15 trade (EURUSD SELL, 16:15 UTC) reached execution but IG rejected it:
`validation.number.too-many-decimal-places.request.size` — deal size was `3.8866666666666667`
(= 11.66 / 3). `position_sizer` floors to `size_decimals`, but downstream multipliers (1/3
concurrent-position sizing, VOLATILE half-size, regime/alignment) run afterwards and re-introduce
long decimals, sent raw. Fixed by rounding `trade.size` to the instrument's `size_decimals` (fallback 2)
in `OrderManager::execute_trade` — the single execution choke point. **VERIFIED LIVE:** after restart,
3 trades filled ACCEPTED at 16:20–16:30 (EURUSD SELL ×2, USDJPY BUY ×1), logs show
`Rounded deal size … → … (2 dp)`, zero rejections post-fix. **This ends the ~6-week no-trade drought.**

> **Milestone:** full live pipeline now proven end-to-end — M15 2/3 consensus (exhaustion-guard fix) →
> VOLATILE H1-zero bypass (strength ≥8 skips H1 gate) → risk approval → size rounded → IG fill.
> NOTE: the H1-gate loosening decision (Phase 17.E options) is now LOWER priority — strong signals
> already bypass the gate in VOLATILE, so trades fire without H1 agreement.

---

## Phase Summary

| Phase | Name | Status |
|-------|------|--------|
| 1 | Safety Net | ✅ Complete |
| 2 | Testability | ✅ Complete |
| 3 | Architecture Cleanup | ✅ Complete |
| 4 | Production Hardening | ✅ Complete |
| 5 | Advanced Strategy Features | ✅ Complete |
| 6 | Engine Hardening / WS Migration | ✅ Complete |
| 7 | Production Backtesting | ✅ Complete |
| 8 | AI/ML Enhancements | ✅ 8.1–8.5, 8.7 done · 8.6 long-term |
| 9 | High-Availability API | ✅ Complete |
| 10 | Connectivity & Intelligence | ✅ Complete |
| 11 | Advanced Deployment | ⏳ Planned |
| 12 | Tactical Volatility | ✅ Complete |
| 13 | Sticky Trade DNA | ✅ Complete |
| 14 | M15 Bar Trading Scheme | ✅ Fully Live (enabled, trading, H1 gate + tick accumulator) |
| 15 | VOLATILE Profitable Strategy | ✅ Complete |
| 16 | Gold Strong-Trend Fix + Risk Refinements | ✅ Complete (2026-03-20) |

---

## Phase 16 — Gold Strong-Trend Fix + Risk Refinements (✅ 2026-03-20)

> **Motivation:** Mar 19 night — Gold crashed 284 pts (07:00–11:00 UTC). Engine missed the entire move.
> Root cause: H1 warmup failed (403 quota after restart) → `h1_snap.adx = None` → all ADX-dependent
> gates silently skipped → RSI_Reversal + Bollinger fired BUY (RSI=8.76, oversold) → H1 bias leaned
> BUY → H1 gate blocked every M15 SELL signal throughout the crash.

| # | Task | Owner | Status | Notes |
|---|------|-------|--------|-------|
| 16.1 | Gold momentum gate — ADX fallback for Gates 3 & 4 | Claude | ✅ Done | `h1_snap.adx.or(m15_snap.adx)` — M15 ADX used when H1 not yet warmed (403/cold start). Mean-rev suppression and RSI extreme block now fire even when H1 REST warmup failed |
| 16.2 | Dynamic consensus in strong-trend regime | Claude | ✅ Done | `mean_rev_suppressed` flag: when Gate 3 removes mean-rev signals, `min_consensus` drops to 1 for remaining momentum signals. Prevents 2/3 threshold unfairly rejecting 1 valid SELL after mean-rev BUYs stripped |
| 16.3 | Price-slope bypass ADX fallback | Claude | ✅ Done | `h1_snap.adx.or(m15_snap.adx)` in bypass check — slope gate can now override H1 strategy-vote gate even when H1 ADX is None. Logs source as "H1" or "M15↑" |
| 16.4 | Trading hours tightened — London/NY only | Claude | ✅ Done | `config/default.toml`: `start=07:00, end=20:00 UTC` (was 00:00–21:00). Prevents Asia session chop; EUR/USD -SGD 549 loss at 01:16 UTC was the trigger |
| 16.5 | EUR/USD per-instrument rules tightened | Claude | ✅ Done | `min_consensus=2` (was 1), `adx_trend_lock_enabled=true` at ADX≥35, `max_daily_trades=3`, RSI extreme blocks (floor=20, ceiling=80 when ADX≥35) |
| 16.6 | Gold per-instrument rules added | Claude | ✅ Done | `min_consensus=2`, mean-rev suppression weight=0 at ADX≥45, RSI extreme block (floor=15, ceiling=85 when ADX≥40), ATR% ceiling=1.8%, max 2 trades/day |
| 16.7 | Overnight financing tracking | Claude | ✅ Done | Hourly `GET /history/transactions?type=INTEREST` poll in `event_loop/mod.rs`; stored in `DailyStats.financing_pnl`; shown in Telegram daily summary as separate line with Net Total |
| 16.8 | OPU close-level fix (DELETED payload) | Claude | ✅ Done | Confirmed `opu.level` = actual fill price in DELETED events; `close_level` field correctly populated in `streaming_client.rs` |
| 16.9 | CI green — fmt + clippy + security audit | Claude | ✅ Done | `cargo fmt` all files; fix unused `debug` import; fix manual range check in `m15_momentum_burst.rs`; bump `quinn-proto 0.11.14` (RUSTSEC-2026-0037); ignore `RUSTSEC-2023-0071` (rsa, no fix) |
| 16.10 | VOLATILE SL multiplier widened 0.75→1.0, TP 2.0→2.5 | Claude | ✅ Done | `config/default.toml` + Rust defaults in `engine/config.rs`. R:R = 2.5/1.0 = 2.5 — exactly clears `min_risk_reward = 2.5`. Reduces premature SL hits in VOLATILE swings. |

**Trade analysis that drove Phase 16 changes (Mar 18–20):**

| Instrument | W | L | BE | Net SGD | Issue found |
|------------|---|---|----|---------|-------------|
| EUR/USD | 4 | 1 | 0 | +~1400 | 1 loss at 01:16 UTC Asia session — fixed by trading hours |
| Gold | 1 | 4 | 2 | -~900 | H1 mean-rev BUY blocked all SELL during 284pt crash — fixed by 16.1–16.3 |
| Overall | 5 | 5 | 2 | +~55 | Win rate 36% → breaks even at 1:1 R:R |

---

## Phase 17 — Trading Performance Fixes (✅ 2026-04-03)

> **Motivation:** 3-week trade analysis (Mar 9 – Apr 2) revealed: 33% win rate, trades stopped out in 2-10 min,
> breakeven snap too aggressive (0.225 ATR trigger), VOLATILE regime stuck for weeks making tight stops permanent,
> EURUSD BUY stop-loss streak (4 consecutive), USDJPY 0% win rate, SUNGOLD -72K sizing catastrophe.

| # | Task | Owner | Status | Notes |
|---|------|-------|--------|-------|
| 17.1 | Loosen breakeven snap 0.3→0.5 | Claude | ✅ Done | `volatile_breakeven_trigger` in `default.toml`, `config.rs`, `risk/mod.rs`. Trades now need 50% of SL distance in profit before BE snap (was 30%). Combined with Fix 17.2: trigger moves from 0.225 ATR → 0.5 ATR — 2.2× more room |
| 17.2 | Widen VOLATILE SL/TP multipliers | Claude | ✅ Done | SL: 0.75→1.0, TP: 2.0→2.5. R:R = 2.5 (passes strict `<` check vs `min_risk_reward=2.5`). `default.toml` + `config.rs` defaults |
| 17.3 | Raise M15 global consensus 1→2 | Claude | ✅ Done | `default_m15_min_consensus()` returns 2. Added `m15_min_consensus = 2` to `default.toml`. All 3 active instruments already had per-instrument override=2, this makes the global default safe for future instruments |
| 17.4 | Tighten USDJPY min_avg_strength 7.5→8.0 | Claude | ✅ Done | `config.rs` instrument override. Raises signal quality bar — every USDJPY trade in Mar 12–31 lost or broke even |
| 17.5 | Remove SUNGOLD from weekend_epics | Claude | ✅ Done | `weekend_epics = []` in `default.toml`, removed `[strategies.weekend_overrides."IX.D.SUNGOLD.CFI.IP"]` section. Prevents -72K sizing catastrophe (missing InstrumentSpec → wrong pip_value fallback) |
| 17.6 | Regime cooldown system | Claude | ✅ Done | New subsystem: tracks regime persistence in `data/regime_persistence.json`. After `regime_cooldown_days` (default 7) of continuous VOLATILE, relaxes SL→1.25×ATR, TP→3.0×ATR, disables BE snap. Config: `regime_cooldown_*` fields in `StrategiesConfig`. Applied in `analysis.rs` (SL/TP) + `handlers.rs` (BE snap skip) |

**Net effect of 17.1 + 17.2:** BE snap trigger = 0.5 × 1.0 ATR = **0.5 ATR** (was 0.3 × 0.75 ATR = 0.225 ATR). 2.2× more breathing room before stop snaps to entry.

**Net effect of 17.6 after 7 days VOLATILE:** SL relaxes to 1.25×ATR, TP to 3.0×ATR (R:R = 2.4), BE snap disabled entirely. Progressive normalization instead of permanent restriction.

---

## Phase 17.B — Multi-Position Concurrent Trading (✅ 2026-04-09)

> **Motivation:** One-trade-at-a-time with 30-min cooldown means the engine sits idle for most of the day. Market
> signal re-entries and scale-ins are impossible. User requested: spread risk across up to 3 smaller concurrent
> positions per instrument instead of one full-size trade with a long cooldown.

| # | Task | Owner | Status | Notes |
|---|------|-------|--------|-------|
| 17.B | Multi-position mode | Claude | ✅ Done | New `max_positions_per_instrument=3` config. "Already have a position" hard block replaced with per-epic count limit. Position size auto-divided by `max_positions_per_instrument` so total risk per epic stays constant. `post_trade_cooldown_secs` 1800→300 (5 min). `max_open_positions` 5→9. M15 extra 0.5× halving removed (1/3 scaling is sufficient). Files: `config/default.toml`, `risk/mod.rs`, `engine/config.rs` |

---

## Phase 17.A — VOLATILE Cold-Start H1 Gate Bypass (✅ 2026-04-05)

> **Motivation:** After every engine restart, the H1 direction gate blocks ALL M15 trades for up to 1 hour until
> the first H1 bar closes. H1 REST warmup often fails (IG 403 quota) so H1 data stays empty. In VOLATILE regime
> where signals are already rare, losing 1 hour per restart is unacceptable.

| # | Task | Owner | Status | Notes |
|---|------|-------|--------|-------|
| 17.A | VOLATILE cold-start bypass in H1 gate | Claude | ✅ Done | In `analysis.rs` `analyze_market_m15()`: when `h1_bias` is `None` (cold start) AND regime is VOLATILE AND `ensemble_signal.strength >= 8.0`, skip the block and log `VOLATILE cold-start bypass`. Only VOLATILE is bypassed — TRENDING/RANGING still require H1 confirmation. |

---

## Phase 17.C — Exit Management Breathing Room (✅ 2026-04-18)

> **Motivation:** 40-trade analysis showed 15/40 (37.5%) trades closed at entry=exit ($0 P&L) due to aggressive BE snap. Only 3/40 (7.5%) hit full TP. EURUSD net +4,200 but win rate only 33% because winners got snapped to BE before pullbacks that would have recovered. Real leverage for win-rate improvement is EXIT management, not entry bars.

| # | Task | Owner | Status | Notes |
|---|------|-------|--------|-------|
| 17.C.1 | Loosen VOLATILE BE snap 0.5 → 0.7 | Claude | ✅ Done | Trade must now be 70% of SL distance in profit before snapping SL to breakeven (was 50%). Gives winners more room to breathe through normal pullbacks. Files: `config/default.toml`, `src/engine/config.rs`, `src/risk/mod.rs` |
| 17.C.2 | Widen trailing stop min pips 5.0 → 7.5 | Claude | ✅ Done | `trailing_stop_min_pips` 5.0→7.5 — less aggressive ratchet step so trailing SL doesn't tighten on every tiny favorable move. Reduces API spam AND gives price more room to breathe. Files: same 3 files |
| 17.C.3 | Confirm TRENDING-birth BE skip | Claude | ✅ Already done | Verified `handlers.rs:114` — `if birth_regime == "VOLATILE" && !be_snap_cooldown_active` — TRENDING/RANGING births already skip BE snap (only VOLATILE births get it). Let TP hit on trending trades. |

**Expected outcome on historical 40-trade replay:** BE trades 15 → ~7, win rate 25% → ~42%, P&L unchanged signal rate (entries unchanged).

---

## Phase 17.D — IG 403 Backoff Fix (✅ 2026-04-27)

> **Symptom:** Engine ran continuously for 9 days but fired ZERO new signals after 2026-04-18T08:34 UTC. Log filled with 45,000+ `error.public-api.exceeded-account-historical-data-allowance` 403s — the IG weekly historical-data quota was exhausted and the engine kept hammering it.
>
> **Root cause #1 — Backoff gated on warmup state:** The M15 refresh loop (`event_loop/mod.rs:1224`) calls `get_price_history` for each epic every 60s. The existing self-heal backoff only triggered when `needs_warmup == true`. Once indicators warmed up from disk, every 60s tick × 3 epics retried the API with **no backoff** → 180 calls/min during a 403 burst, burning the weekly quota in hours and locking us out for days.
>
> **Root cause #2 — `is_quota` substring match never matched IG errors:** First fix attempted `err.contains("403") || err.contains("exceeded-account-historical-data-allowance")`, but in `api/errors.rs` the 403 status is wrapped into `IGError::RateLimitExceeded("Forbidden / Burst limit hit")`. The `Display` string is `"Rate limit exceeded: Forbidden / Burst limit hit"` — contains neither "403" nor the original errorCode. So backoff never engaged. Verified by zero `backing off` log lines after first deployment despite ongoing 403s.

| # | Task | Owner | Status | Notes |
|---|------|-------|--------|-------|
| 17.D.1 | Make backoff check unconditional | Claude | ✅ Done | Moved `in_backoff` skip outside the `if needs_warmup` block. Steady-state refresh now also skips API when backoff active. Tick accumulator builds bars locally during backoff — no data loss. File: `src/engine/event_loop/mod.rs:1242-1275` |
| 17.D.2 | Widen backoff window 30→60min | Claude | ✅ Done | IG historical-data quota is weekly; 30 min was too aggressive once quota exhausted. File: same |
| 17.D.3 | Fix `is_quota` substring detection | Claude | ✅ Done | Now matches `"Rate limit exceeded"` / `"Forbidden"` / `"403"` / `"exceeded-account-historical-data-allowance"` — covers IG's actual `IGError::RateLimitExceeded` Display output. File: `src/engine/event_loop/mod.rs:1342-1358` |
| 17.D.4 | Verify in production | Claude | ✅ Done | Single launchd-managed instance (PID 87239). Post-restart 403s = 0 after first backoff cycle (3 epics × 1 attempt = 3 unique 403s, then silent). 15+ `backing off 60 min` log lines confirm engagement. Bar analysis firing every 60s. |

**Operational note:** Engine is managed by **launchd** (`~/Library/LaunchAgents/com.igengine.plist`, `KeepAlive=true`). Killing only the PID-file process leaves launchd to respawn a duplicate. **Always use `launchctl unload/load` to restart**, not `kill` + `nohup`. Earlier verification was misled by 3 zombie engines all running simultaneously.

---

## Phase 17.A-Fix — H1-Zero Bypass + Notification Spam (✅ 2026-04-18)

> **Motivation 1:** Phase 17.A cold-start bypass only applied when `h1_bias` was `None`. After ~1 hour, H1 analysis runs but produces 0 signals (buy_count=0, sell_count=0), converting bias to `Some(...)`. From that point forward, the bypass no longer applied and the gate blocked all M15 trades indefinitely.
> 
> **Motivation 2:** Multi-position concurrent trading exposed notification spam: 5+ Telegram alerts per single trade closure. Root cause: OPU stream path and REST close path both sending notifications for the same trade, with Lightstreamer replays doubling the events.

| # | Task | Owner | Status | Notes |
|---|------|-------|--------|-------|
| 17.A.1 | Extend VOLATILE bypass to H1-zero case | Claude | ✅ Done | In `analysis.rs` line 1654–1674, added same bypass logic to `Some(bias) if bias.buy_count==0 && bias.sell_count==0` match arm: if VOLATILE regime AND strength >= 8.0, allow trade instead of blocking. Fixes perpetual gate block after H1 runs but fires no signals. Files: `src/engine/event_loop/analysis.rs` |
| 17.A.2 | Fix 5x notification spam (OPU duplicate path) | Claude | ✅ Done | Removed Telegram notification from OPU stream handler (`streaming_client.rs`). Keep only authoritative REST close notification from `handlers.rs`. OPU events still update state and trigger internal events, just no duplicate Telegram. With 3 concurrent positions and overlapping closes, two notification paths created 5+ alerts per trade. Files: `src/api/streaming_client.rs` |

---

## Phase 15 — VOLATILE Profitable Strategy (Complete)

> **Goal:** Make the engine actually trade in VOLATILE regime by adding more vote sources, signal boosters, and fixing critical bugs.

| # | Task | Owner | Status | Notes |
|---|------|-------|--------|-------|
| 15.1 | StochasticMomentumStrategy (6th vote source) | Claude | ✅ Done | `strategy/stochastic_momentum.rs` — %K/%D crossover; BUY: bullish cross <50; SELL: bearish cross >50; strength 7.0–10.0 with ADX/RSI bonuses |
| 15.2 | Phase B signal boosters | Claude | ✅ Done | `apply_signal_boosters()` in analysis.rs — ATR expansion (+1.0 all); key level proximity (×1.2 breakout direction) |
| 15.3 | VOLATILE regime multipliers updated | Claude | ✅ Done | Stochastic_Momentum: 0.8× (vs 0.5× others); Ranging: 1.2× for oscillators |
| 15.4 | Fix Multi_Timeframe hardcoded strength=9.0 | Claude | ✅ Done | Dynamic `calculate_signal_strength()`: ADX+1.5, MACD expanding+0.5, RSI pullback+0.5, fallback TF-1.0 |
| 15.5 | Fix critical daily reset bug | Claude | ✅ Done | `risk_manager.reset_daily()` was NEVER called — engine stopped after 20 lifetime trades; now detects date rollover and resets both state + risk_manager |
| 15.6 | Audit all strategies for mock/stub logic | Claude | ✅ Done | All 6 strategies confirmed real (MA_Crossover, RSI_Reversal, MACD_Momentum, Bollinger, Multi_Timeframe, Stochastic_Momentum) |
| 15.7 | Fix sentiment_agent.py Python 3.9 crash | Claude | ✅ Done | `dict \| None` → `Optional[dict]` (Python 3.10 syntax on 3.9); fixed at 20:11 SGT 2026-03-16 |

---

## Phase 12 — Tactical Volatility (Complete)

| # | Task | Owner | Status | Notes |
|---|------|-------|--------|-------|
| 12.1 | Regime-Switching Logic | Claude | ✅ Done | regime/mod.rs + apply_regime_multipliers in analysis.rs; TRENDING/RANGING/VOLATILE signal multipliers |
| 12.2 | Sentiment Velocity Guard | Claude | ✅ Done | velocity>0.5 sets macro_pause_until (+2h) in MetricsState; analysis.rs checks it before all trade entries |
| 12.3 | Dynamic Spread Gate | Claude | ✅ Done | avg_spread EMA(0.05) on MarketState; spread>1.5×avg rejects trade in analysis.rs |
| 12.4 | Limit Order Migration | Claude | ✅ Done | VOLATILE regime sets AdjustedTrade.order_type=LIMIT+entry_level; order_manager uses it |

---

## Phase 13 — Sticky Trade DNA (Management)

| # | Task | Owner | Status | Notes |
|---|------|-------|--------|-------|
| 13.1 | Birth Regime Tracking | Claude | ✅ Done | opened_in_regime: Option<String> on Position; set in analysis.rs at trade open |
| 13.2 | Management Personalities | Claude | ✅ Done | handlers.rs: VOLATILE-birth → break-even snap; TRENDING-birth+current VOLATILE → skip ratchet |
| 13.3 | Genetic P&L Logging | Claude | ✅ Done | ClosedTrade.opened_in_regime serialised to trades.jsonl via existing TradeLogger |

---

## Phase 14 — M15 Bar Trading Scheme (✅ Implemented)

> **Opus Architecture Plan completed 2026-03-16. Implementation complete 2026-03-16.**
> Full dual-timeframe architecture: M15 as primary signal, H1 as directional filter.
> All 3 strategies disabled by default — enable in `config/default.toml` when ready.

| # | Task | Owner | Status | Notes |
|---|------|-------|--------|-------|
| 14.A | Infrastructure — `M15CooldownTracker` + M15 config structs | Claude | ✅ Done | `M15CooldownTracker` in `state.rs`; 3 config structs + 4 new fields in `config.rs`; `indicators` map already supported MINUTE_15 as a key — no new field needed |
| 14.B | M15 data pipeline — warmup (250 bars) + 60s refresh interval | Claude | ✅ Done | Disk-first warmup in `event_loop/mod.rs`; `m15_refresh_interval` (60s); `last_m15_candle_ts` dedup; new bars trigger `analyze_market_m15()` |
| 14.C | `M15Strategy` trait + 3 M15 strategies | Claude | ✅ Done | `strategy/traits.rs` extended; `m15_momentum_burst.rs`, `m15_ema_microtrend.rs`, `m15_bollinger_reversion.rs` created |
| 14.D | `check_trade_m15()` in RiskManager + `analyze_market_m15()` | Claude | ✅ Done | 0.5× position size (rejects if < min_deal_size); cooldown max 2 trades/H1 candle; apply_m15_regime_multipliers() in analysis.rs |
| 14.E | H1 Direction Gate + H1 Alignment Bonus | Claude | ✅ Done | Gate blocks M15 signals contradicting H1 bias; ×1.2 bonus for M15 signals agreeing with H1 direction. Config: `h1_direction_gate_enabled`, `h1_alignment_bonus = 1.2`. `H1DirectionBias` struct in `state.rs`, written by `analyze_market()`, read by `analyze_market_m15()` |
| 14.F | M15 disk persistence fix | Claude | ✅ Done | `persist_series("MINUTE_15")` called after API warmup AND after each 60s live tick. Prevents re-fetching 250 bars from IG API on every restart |
| 14.G | M15 self-heal (API rate limit recovery) | Claude | ✅ Done | If M15 indicators not warmed at 60s tick, fetch 250 bars instead of 5 — auto-recovers without restart when IG rate limit resets |
| 14.H | M15 tick accumulator (BarAccumulator) | Claude | ✅ Done | `bar_accumulator_m15: BarAccumulator::new(900)` added to `MarketStateContainer`. Lightstreamer ticks → M15 OHLCV bars → CandleStore + indicator update + persist. M15 candle data now built locally from live ticks, fully independent of IG API |
| 14.I | M15 analysis fallback (tick-warmed) | Claude | ✅ Done | 60s loop runs `analyze_market_m15()` even when IG API fails, provided M15 indicators are warmed from tick-built bars |

**M15 Strategy Summary:**

| Strategy | Regime | Signal Logic | Multiplier |
|---|---|---|---|
| M15_MomentumBurst | Trending/Volatile | RSI 55–75 + MACD expanding + H1 EMA200 confirm | VOLATILE: 1.3× · TRENDING: 1.2× |
| M15_EmaMicrotrend | Trending/Volatile | EMA9>EMA21 + EMA21 slope + H1 EMA21 slope confirm | TRENDING: 1.2× |
| M15_BollingerReversion | Ranging ONLY | %B<0.05 + RSI<35 + H1 RSI>35 | RANGING: 1.2× |

**Risk profile:** 0.5× H1 position size via `check_trade_m15()` · 4.0× ATR TP / 1.5× ATR SL (R:R = 2.67) · max 2 trades/H1 candle · R:R ≥ 2.5 validated before signal

**M15 candle sources (priority order):**
1. `data/candles/*_MINUTE_15.jsonl` — disk cache (tick-built, populated from live ticks going forward)
2. IG REST API — 250-bar warmup on first start (if disk < 210 bars); self-heal on restart
3. Live Lightstreamer ticks → `bar_accumulator_m15` (continuous, always running)

---

## Phase 11 — Advanced Deployment (Planned)

> When ready to move to 24/7 VPS (Hetzner Singapore recommended — ~€8/mo, closest to IG servers).

| # | Task | Status | Notes |
|---|------|--------|-------|
| 11.1 | Docker-Compose + health checks | ⏳ | Dockerfile exists; add compose + liveness probes |
| 11.2 | GitHub Actions CI/CD | ⏳ | cargo test + clippy + rsync deploy to VPS on merge to main |
| 11.3 | Systemd service unit | ⏳ | Auto-restart on reboot; logrotate for engine.log |
| 11.4 | Regime-aware deploy gate | ⏳ | CD delays restart if regime = VOLATILE (avoid mid-trade reload) |
| 11.5 | VPS IP whitelist in IG API dashboard | ⏳ | Required before going live on VPS |

---

## Phase 9 — High-Availability API (Robustness)

| # | Task | Owner | Status | Notes |
|---|------|-------|--------|-------|
| 9.1 | Leaky Bucket Rate Limiter | Claude | ✅ Complete | TokenBucket replaces Semaphore; rate_limit_per_minute wired from config (commit 4591a90) |
| 9.2 | Granular IG Error Enum | Claude | ✅ Complete | errors.rs activated (was orphan); handle_response() uses IGError; UNAUTHORIZED sentinel preserved (commit 4591a90) |

---

## Phase 10 — Connectivity & Intelligence

| # | Task | Owner | Status | Notes |
|---|------|-------|--------|-------|
| 10.1 | Client Sentiment Indicators | Claude | ✅ Done | GlobalSentimentRegistry activated in state.rs; get_client_sentiment() on TraderAPI; 15-min poll timer in event_loop; rest_client + mock_client implemented |
| 10.2 | Related Market Sentiment | Claude | ✅ Done | context_market_ids polled alongside trading epics; all market IDs update the same GlobalSentimentRegistry |
| 10.3 | Recursive API Pagination | Claude | ✅ Done | get_account_activity() follows metadata.paging.next until exhausted; Version 1 |
| 10.4 | Watchlist Syncing (BOT_ACTIVE) | Claude | ✅ Done | 1-hr watchlist_sync_interval; fetches BOT_ACTIVE watchlist and adds new epics to state dynamically |

---

## Phase 5 — Advanced Strategy Features

| # | Task | Status | Owner | Notes |
|---|------|--------|-------|-------|
| 5.1 | Trailing Stop Loss logic | ✅ Done | Claude + Gemini | Ratchet logic + strategy-specific distances from config; configurable `trailing_stop_min_pips` in RiskConfig |
| 5.2 | Session-specific filters (news exclusion) | ✅ Done | Claude | Session filter + news blackout windows (±15min configurable) |
| 5.4 | Shadow Mode (Paper mode strategy validation) | ✅ Done | — | Mapped to Paper mode |

---

## Live Trading Transition (In Progress)

| # | Task | Owner | Status | Notes |
|---|------|-------|--------|-------|
| L.1 | Close position API fix (POST + _method:DELETE) | Gemini | ✅ Done | Was completely broken — DELETE verb not supported by IG |
| L.2 | Currency code hybrid logic (JPY/USD/account-base) | Gemini | ✅ Done | Per-trade currency + Position.currency field |
| L.3 | Verify min deal sizes on demo | Gemini | ✅ Done | EUR/USD=0.5, GBP/USD=0.5, USD/JPY=0.2, Gold=3.0 |
| L.4 | Config-driven guaranteed_stop | Gemini | ✅ Done | Reads from config.risk.limited_risk_account, not hardcoded |
| L.5 | api_lab CLI tool (Rust + Python) | Gemini | ✅ Done | List, close, clear_profit, inject trades |
| L.6 | Complete `config/live.toml` (all sections) | Claude | ✅ Done | Fixed 7 wrong field names, added 9 macro events, instrument specs, ADX overrides |
| L.7 | Create `config/live-ramp.toml` | Claude | ✅ Done | USD/JPY only, 0.25% risk, 1 position, 5 trades/day |
| L.8 | Live startup validation in Rust | Claude | ✅ Done | validate_live_readiness() — macro events, spec completeness, margin feasibility |
| L.9 | Verify live epic codes + min deal sizes | User | ⏳ Waiting | Must check on live IG platform before going live |

---

## Known Bugs / Open Issues

| Priority | Description | File(s) |
|----------|-------------|---------|
| Low | IG 403 quota after multiple restarts — H1 REST warmup fails; tick accumulator builds M15 bars instead; ADX fallback (16.1) mitigates impact | `data/candles/` |
| Low | Python test scripts (`test_ig_trade*.py`) fail in proxied/sandboxed environments — `ProxyError: 403 Forbidden`. Must run locally. | `test_ig_trade*.py` |
| Low | rsa RUSTSEC-2023-0071 (Marvin Attack) — no upstream fix; ignored in audit.toml. Not exploitable in this context. | `Cargo.lock` |

### Recently Fixed (2026-04-05)

| Bug | Root Cause | Fix |
|-----|-----------|-----|
| Ensemble average poisoned by regime-crushed signals | Regime multipliers (e.g. MA_Crossover × 0.3 = 2.4 in TRENDING) left near-zero signals in the vote pool, dragging avg_strength below threshold even when 3+ valid signals agreed | New `ensemble_signal_floor = 5.0` in `StrategiesConfig`; signals below floor excluded from consensus count + avg before vote in `EnsembleVoter::vote_with_overrides`. Logged when signals are filtered. Config field in `default.toml`. |
| VOLATILE M15 trades blocked entirely | `min_consensus=2` (global + instrument overrides) but only 1/3 M15 strategies ever fires per bar in VOLATILE → threshold never met | Runtime relaxation in `analysis.rs`: when `regime_str == "VOLATILE"`, `override_consensus` is decremented by 1 (floor 1). Global default and instrument overrides unchanged; logs `[M15] VOLATILE consensus relaxed: 2 → 1` |

### Recently Fixed (2026-03-20)

| Bug | Root Cause | Fix |
|-----|-----------|-----|
| Gold missed 284pt crash (Mar 19) | H1 ADX=None after 403 quota restart → mean-rev gates silently skipped → BUY bias blocked SELL | ADX fallback: `h1_snap.adx.or(m15_snap.adx)` in Gates 3, 4, and slope bypass (16.1, 16.3) |
| Gold SELL blocked even with 1 valid momentum signal | `min_consensus=2` applied to pool AFTER mean-rev signals removed → 1/1 never reaches 2 | Dynamic consensus: `mean_rev_suppressed=true` → drops threshold to 1 (16.2) |
| EUR/USD -SGD 549 loss at 01:16 UTC | Trading allowed 00:00–21:00 UTC; Asia session + 1/3 consensus + stale H1 | Trading hours → 07:00–20:00; min_consensus=2 for EUR/USD (16.4, 16.5) |
| Overnight financing not tracked | IG financing charges not in OPU stream | Hourly REST poll `GET /history/transactions?type=INTEREST` → `DailyStats.financing_pnl` (16.7) |
| P&L = 0 on Gold closes (apparent bug) | SL moved to entry by VOLATILE BE trigger (correct behaviour) | Confirmed expected: `level = openLevel = stopLevel` when breakeven SL hit |

### Recently Fixed (2026-03-17)

| Bug | Root Cause | Fix |
|-----|-----------|-----|
| Regime file stale (57h) — engine always saw NORMAL regime, "need 3" instead of "need 2" | `ig-engine/data/regime_latest.json` was a standalone file; cron wrote to `ig-trading/data/regime_latest.json` (different path) | Replaced with symlink `../../data/regime_latest.json` — now auto-refreshed every hour by cron |
| M15 candles never persisted — every restart hit IG API rate limit | `persist_series("MINUTE_15")` was never called after API warmup or live tick | Added `persist_series()` after warmup and after each 60s tick; tick accumulator also persists every 15 min |
| M15 R:R 1.0 rejected (min 2.5) | `atr_tp_multiplier = 1.5` = same as SL → R:R = 1.0 | Fixed `atr_tp_multiplier = 4.0` → R:R = 2.67 |
| H1 says Gold BUY, M15 fires Gold SELL → SL hit immediately | No cross-timeframe conflict detection | H1 Direction Gate (14.E): blocks M15 signals contradicting H1 bias; H1 Alignment Bonus ×1.2 for agreeing signals |
| M15 indicators never warm after restart (rate limit exhausted) | Multiple rapid restarts exhausted IG's historical data allowance | Self-heal (14.G): fetch 250 bars when `!is_warmed_up()`; tick accumulator (14.H) builds bars locally forever |

### Recently Fixed (2026-03-16)

| Bug | Root Cause | Fix |
|-----|-----------|-----|
| Engine stops trading after 20 total trades (lifetime, not daily) | `risk_manager.reset_daily()` was never called — daily trade counter hit max and never reset | Detect date rollover in daily_reset_interval branch; call both `state.check_daily_reset()` AND `risk_manager.reset_daily()` |
| VOLATILE regime: no trades ever execute | With `VOLATILE_MUTE=0.5`, strategies couldn't reach ensemble threshold; only 2-3 vote sources available | Added StochasticMomentumStrategy (0.8× mute) + signal boosters + VOLATILE scalp tier (2 strategies, avg≥6.0) |
| Multi_Timeframe always signals strength=9.0 | Hardcoded `let strength = 9.0` — all signals looked identical to adaptive weight system | Dynamic `calculate_signal_strength()` based on ADX, MACD expansion, RSI pullback |
| sentiment_agent.py crashes every 15 min since 18:16 SGT | `dict \| None` union syntax requires Python 3.10+; cron runs Python 3.9.6 | Changed to `Optional[dict]` from typing module |

### Recently Fixed (2026-03-12)

| Bug | Root Cause | Fix | Commit |
|-----|-----------|-----|--------|
| ALL market analysis silently skipped | `MARKET_STATE` comparison was `!= "TRADEABLE"` (uppercase) but IG sends lowercase `"tradeable"` | Changed to `to_ascii_uppercase().starts_with("TRADEABLE")` | 169d22f |
| Indicators never reached warmup (19 of 250 candles used) | `snapshotTime` parse used wrong format — all 250 candles got `Utc::now()` → deduplicated to 1 | Multi-format parse: try RFC3339 → strip `:SSS` → IG format | 706a1dd |

---

## Phase 6 — Engine Hardening (Complete)

| # | Task | Owner | Status | Rationale |
|---|------|-------|--------|-----------|
| 6.1 | Multi-timeframe analysis | Claude | ✅ Done | Evaluates trend, signal, and entry timeframes together |
| 6.2 | WebSocket push fully replace REST polling | Claude | ✅ Done | BarAccumulator drives OHLCV bars from WS ticks |
| 6.4 | Fix remaining `unwrap()` panics in optimizer + backtester | Claude | ✅ Done | Safety for live mode |
| 6.5 | Live mode pre-flight checklist | Gemini | ✅ Done | LIVE_PREFLIGHT_CHECKLIST.md |
| 6.7 | Engine hardening weekend session | Claude | ✅ Done | 7 improvements: MARKET_STATE propagation, state worker, bar-close gating, VecDeque, dedup, log levels, unwrap cleanup |
| 6.8 | Candle persistence layer (survive restarts) | Claude | ✅ Done | JSONL disk cache → instant warmup on restart. Disk-first startup, persist on bar close + shutdown. |

---

## Phase 7 — Production Backtesting (Complete)

| # | Task | Owner | Status | Rationale |
|---|------|-------|--------|-----------|
| 7.1 | Historical candle data fetcher | Claude | ✅ Done | `scripts/fetch_historical_data.py` — yfinance 2yr 1H OHLCV |
| 7.2 | Python backtester — ensemble + trailing stop | Claude | ✅ Done | Portfolio +$2,625 (+26%) at 2.97% max DD |
| 7.3 | Parameter optimizer | Claude | ✅ Done | `scripts/optimize.py` — grid search |
| 7.4 | ADX range filter in Rust engine | Claude | ✅ Done | Strategy override per instrument |
| 7.5 | Backtest HTTP endpoint | Gemini | ✅ Done | `POST /backtest` on port 9090 |

---

## Phase 8 — AI/ML Enhancements

> **Full details:** See `AI_ROADMAP.md`
> **Philosophy:** AI is additive — classical ensemble stays as core, AI layers on top.

| # | Task | Owner | Status | Priority | Rationale |
|---|------|-------|--------|----------|-----------|
| 8.1 | Walk-forward auto re-optimisation | Claude | ✅ Done | 🔴 High | Weekly self-tuning via SIGUSR1 hot-reload |
| 8.2 | Performance-based strategy weighting | Claude | ✅ Done | 🔴 High | Adaptive weights every 10 trades, rolling 50-trade window |
| 8.3 | Gold news sentiment signal | Claude | ✅ Done | 🟠 Medium | RSS → keyword/Ollama/Claude scoring → 5th Signal for Gold |
| 8.4 | ML regime classifier | Claude | ✅ Done | 🟠 Medium | LightGBM per instrument → TRENDING/RANGING/VOLATILE multipliers |
| 8.5 | Macro calendar awareness | Claude | ✅ Done | 🟡 Low | Per-event blackout windows + live ForexFactory calendar (`scripts/fetch_calendar.py` → `data/economic_calendar.json`, 26h stale fallback). London Open blocks removed — only fire on actual event days. |
| 8.6 | RL position sizing | Claude | 🏗️ Long-term | 🔵 | PPO on live trade outcomes. `TradeLogger` recording to `logs/trades.jsonl`. Needs 3+ months data. |
| 8.7 | Code quality pass — zero clippy warnings | Claude | ✅ Done | 🔴 High | `cargo clippy -- -D warnings` exits 0. All 74 tests pass. |

### Data Collection — Active

Every trade logged is future training data for 8.6:

| Data | File | Purpose |
|------|------|---------|
| OHLCV candles | `data/candles/*.jsonl` | Persist across restarts, regime classifier, re-optimise |
| Trade outcomes | `logs/trades.jsonl` | Strategy weighting, RL |
| Strategy signals | `logs/signals.jsonl` | Win rate tracking |
| Sentiment scores | `data/sentiment.db` | Sentiment validation |

---

## How to Update This File

- When starting a task: change status to 🏗️ In Progress
- When completing a task: change status to ✅ Done
- When discovering a new bug: add a row to the Known Bugs table
