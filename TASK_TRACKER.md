# TASK_TRACKER.md — IG Trading Engine

**Last updated:** 2026-06-24 (Incident: Lightstreamer feed died at weekend close, engine blind ~4 days — added stale-data watchdog + restart recovery. ⛔ PARAMETER FREEZE still in effect until 2026-07-03)
**Current phase:** Production-ready + Active trading. VOLATILE regime live + cooldown system. Concurrent multi-position mode live. H1 gate dual bypass (cold-start + zero-signal).
**Current focus:** 🤖 Engine live & trading | 📊 Multi-position mode: max 3/instrument at 1/3 size | 🔄 Regime cooldown active (7-day VOLATILE → relaxed SL/TP) | 🕐 Trading hours: 07:00–20:00 UTC only | 🚪 H1 gate: VOLATILE bypass for 0-signal & cold-start

> 📦 Dashboard (`src/`) is **archived** — not maintained. All dashboard tasks removed.

For the full history of completed work and debt items, see `TECH_DEBT_AUDIT.md`.

---

## Incident — Lightstreamer Feed Death / Engine Blind ~4 Days (2026-06-24)

> **Symptom (operator-reported):** "no signals for 3 days." **Root cause:** the Lightstreamer
> tick feed died at the Friday weekend close (last bar 06-19 22:05 UTC) and the auto-reconnect
> never recovered when the market reopened Sunday — a silent half-open socket (`connect()` blocked
> forever, never returned, so the reconnect loop never re-iterated). Auth/tokens stayed healthy
> (refreshing every 50 min), masking it. With the REST fallback also blocked by the 403 weekly
> quota, the engine ran "alive" but blind: no bars → no analysis → no signals → no trades for ~4
> days. 4 positions sat "open" in the engine's view (actually closed at IG via guaranteed stops;
> reconciled +1,227 on restart). No money lost — the outage froze the engine's *view*, not the account.

**Recovery:** restart re-established the feed instantly (bars + 2/3 GOLD consensus within seconds),
re-synced positions from IG. Balance 218,931.

**Fixes shipped:** `engine_status.sh` gained a `DATA:` watchdog — flags `⚠️ STALE` if no `Bar closed`
in >20 min during market hours (weekday, FX open). `docs/MONITORING.md` + AGENTS.md: a confirmed
stale feed during market hours = auto-restart (the one restart exception that doesn't need approval).

**Queued follow-up (not done — risky):** in-engine auto-reconnect-on-staleness watchdog (notify the
streaming shutdown when no tick for N min → force a fresh reconnect). Depends on the external
`lightstreamer-client` 0.1.x crate honoring the shutdown notify mid-blocked-read; needs weekend
validation and conservative thresholds to avoid false-positive reconnect storms. The monitoring
auto-restart is the reliable backstop until then.

---

## Phase 17.H — CRITICAL: Circuit Breaker + Daily-Loss Limit Were Dead (✅ 2026-06-16, freeze-exempt)

> **Discovered during a live drawdown.** A correlated cluster stopped out the one-directional book on
> 06-16, running the day to **−3,264 with 6 consecutive losses** — and **neither** the loss-streak
> circuit breaker **nor** the 2% daily-loss limit engaged. Investigation: `metrics.circuit_breaker_active`
> gates `can_trade()` (both H1 + M15 paths) but was **never set true anywhere**; the RiskManager's own
> `consecutive_losses`/`daily_pnl`/`is_paused` were only updated by `record_trade_result()`, which is
> called **only in a unit test**. So both safety nets had been non-functional the entire time — the
> documented "reduce after N / pause after M / stop at 2%" protections did not exist in the running bot.
> (Only became visible because the same-day close-accounting fix made the losses show up in stats.)

> **Live action:** engine manually **paused** via `/api/control` (user-approved) to stop the bleed, fix
> built + tested under pause, then deployed + resumed.

**Fix** (`state.rs`, `default.toml`): `EngineState::update_circuit_breaker()` recomputes
`metrics.circuit_breaker_active` from live daily stats on every close — trips when
`consecutive_losses >= consecutive_losses_pause` OR daily P&L breaches `max_daily_loss_pct`; clears on a
winning close or the 00:00 UTC daily reset (which now also clears the flag). Thresholds made explicit in
`[risk.circuit_breaker]`: **pause at 5** (the ~32% win rate makes 3-streaks common, so the dead-default 3
would halt constantly; the 06-16 blow-up was a 5–6 cluster). Daily limit stays **2%**. Tests: 4 new
`circuit_breaker_tests` + `config_load` asserts the threshold. PR #8 merged, engine redeployed.

> **Risk-system audit follow-up (queued):** other `#[allow(dead_code)]` risk methods exist — worth a
> sweep to confirm nothing else documented-but-dead (e.g. weekly drawdown limit, correlated-position cap).

---

## Bug Fix — Guaranteed-Stop Close Accounting (✅ 2026-06-16, freeze-exempt)

> **Found during freeze monitoring** (operator noticed balance dropping with no logged closes).
> On a limited-risk account, IG closes the position **server-side** the instant the guaranteed
> stop fires. The engine independently detects the same hit and fires a redundant REST close,
> which **404s `position.notional.details.null`** (position already gone). Guaranteed-stop closes
> emit **no OPU close event** (only `status=OPEN`), so the old `Err` branch — which merely logged —
> **silently dropped the trade** from daily stats, the scorecard, AND the circuit breaker.

**Evidence (reconciled from balance, this engine):** since the 06-12 restart the engine
under-recorded realized P&L by **≈ −1,435 SGD**; 9 stop-losses incl. a **−1,122 GOLD** were invisible,
and **06-14 21:19–21:27 was a 4-loss streak (−1,506) the circuit breaker never counted.** Confirmed
the OPU stream never sends a close for these (only `status=OPEN … ignoring`) → fully *missed*, not
double-counted.

**Fix** (`handlers.rs`): `is_already_closed_error()` detects 404/`notional.null`/`POSITION_NOT_FOUND`;
the `Err` branch then records the close with the engine's locally-computed P&L, **dedup-gated through
`recently_closed_deal_ids`** so any later OPU event can't double-book. Genuine failures still error.
Unit tests pin the error strings. `engine_status.sh` gained a RECONCILED line. PR #5 (merged), engine
redeployed PID 15950.

> ⚠️ **Scorecard/freeze-data caveat:** the freeze-period P&L collected before this fix is unreliable
> (the 06-14 evening session was logged as "weekend flat" but was a real −2.6k losing session). The
> clean freeze-evaluation window effectively **restarts 2026-06-16**; ground-truth P&L is the account
> balance, not the pre-fix stats. The adaptive scorecard was also training on incomplete history.

---

## Phase 17.G — USDJPY SL Override + Entry Spacing + ⛔ Parameter Freeze (✅ 2026-06-12)

> **Motivation:** BE-snap 0.9 unmasked USDJPY's real problem on 06-11 — both longs stopped ~4.7
> pips from entry (1.5× of a tiny M15 ATR): the same whipsaw disease 17.F fixed for EURUSD.
> Separately, every multi-loss day featured same-instrument entries stacked 1–15 min apart that
> won or died together (06-10 EURUSD ×2, 06-11 USDJPY ×2) — doubled risk, zero diversification.
> User approved: USDJPY override extension + suggestion #4 (entry spacing) + parameter freeze.

**Fix #1 — USDJPY M15 SL/TP override** (`default.toml`): same treatment as EURUSD —
`2.5× ATR SL / 6.5× ATR TP` under `[strategies.instrument_overrides."CS.D.USDJPY.CSD.IP"]`.
`tests/config_load.rs` now loops over both overridden epics asserting R:R ≥ min_risk_reward.

**Fix #2 — Minimum same-instrument entry spacing** (`state.rs`, `config.rs`, `analysis.rs`):
- `M15CooldownTracker` gains `last_entry_ts` per epic + `secs_since_last_entry()`.
- New config `m15_min_entry_spacing_secs = 2700` (45 min; 0 disables; serde default 2700).
- Checked in the M15 path next to the per-H1-candle cap; logs
  `[M15] {epic} — entry spacing: last entry {n}s ago < {min}s minimum`.
- M15 path only — all observed stacking came through the 60s M15 refresh.
- Unit tests: `m15_cooldown_tests` in `state.rs` (spacing per-epic + H1 counter unchanged).

**⛔ PARAMETER FREEZE (2026-06-12 → 2026-07-03):** no strategy/risk/gate tuning for ~3 weeks of
clean data. Evaluation criteria fixed up-front: profit factor > 1.3 overall; EURUSD and USDJPY
each individually ≥ 0; performance through at least one regime change. Monitoring continues
(observe + propose only). Exceptions: genuine bugs (crashes, API errors, wrong math) — not P&L.

**Pending (frozen, revisit 2026-07-03):** #1 consecutive-loss cooldown; H1-zero bypass threshold
(8.0 vs observed 7.80 GOLD block); Asia-session investigation (recurring blocked-consensus
overnight, inconclusive 12-trade backtest).

### Freeze-end backlog — exit-management investigation (logged 2026-06-15)

> **Operator observation:** "sometimes already in profit but TP is too high" — trades reach real
> profit then reverse to a break-even scratch. **Confirmed in log:** of 44 closes (06-09→06-15),
> **16 scratched at exactly 0.00** after arming the BE-snap (≥2.25× ATR in profit); only 4 hit the
> full TP, 5 trailed to a partial win. 31 BE-snap arming events total. Whole edge = 4 big TP wins.

> **Backtest verdict (do NOT implement — both "obvious" fixes fail; `/tmp/trailing_backtest.py`,
> EURUSD+USDJPY M15 04-27→06-15, EmaMicrotrend-approx entries, identical across policies):**
> - **Tighter trailing (1.0× ATR once in profit):** recovers only 4/26 scratches, +50 pips (~7%). Marginal.
> - **Nearer TP (pull 6.5×→3.0× ATR):** net gets *worse* (−717→−813 pips). More green trades but it
>   caps the rare big winner that carries a low-win-rate system. The far TP is doing its job.
> - **Real lever is entry quality, not exits:** 132/195 backtest trades were full SL losses that never
>   reached profit — no exit tweak touches those. Win rate ~32% sits right on the 33% break-even line
>   for a 2.0× payoff. (Backtest PF ~0.4 is pessimistic vs live ~0.93 because it omits the 2/3
>   consensus + H1 gate — which is itself evidence that selectivity is the lever.)

> **Post-freeze direction (not a decision — evidence for 07-03 review):** (1) raise entry selectivity
> to lift win rate above ~33%; (2) concentrate on GOLD (only instrument whose trends run far enough
> to reach the TPs). Exit-management tinkering is a dead end per the above.

---

## Phase 17.F — Live-Week Tuning: EURUSD Whipsaw SL + BE-Snap Relax (✅ 2026-06-11)

> **Motivation — first live-week P&L (06-08 → 06-11, ~36 closes):** run net ≈ **+1,373 SGD**, but
> 100% GOLD-concentrated. GOLD ≈ +2,945; EURUSD ≈ −1,570 (every loss a ~5–6 pip whipsaw stop-out,
> short AND long — the 06-10 direction-flip BUYs died the same way as the 11 losing SELLs);
> USDJPY ≈ 0.00 with a **100% BE-scratch rate** (the 70% BE-snap sterilized every trade).
> User approved suggestions #2 (wider EURUSD SL) and #3 (relax BE-snap).

**Fix #1 — Per-instrument M15 SL/TP override** (`config.rs`, `analysis.rs`, `default.toml`):
- New `InstrumentStrategyOverride` fields: `m15_atr_sl_multiplier` / `m15_atr_tp_multiplier`.
- Applied in the M15 path of `analysis.rs` right after the ensemble signal forms (before the risk
  gate): recomputes SL/TP from **M15 ATR** with the per-epic multipliers.
- EURUSD shipped at **2.5× SL / 6.5× TP** (≈10-pip SL, R:R 2.6 — clears `min_risk_reward = 2.5`).
- Trailing distance + BE-snap trigger derive from SL distance (`risk/mod.rs::check_trade`), so both
  widen automatically.
- ⚠️ Lesson encoded in `tests/config_load.rs`: widening SL without scaling TP would have made the
  RR gate silently reject every EURUSD trade. The test parses the real `default.toml` and asserts
  `tp_mult / sl_mult ≥ min_risk_reward` (also the first CI guard that the shipped TOML parses at all).

**Fix #2 — BE-snap trigger 0.7 → 0.9** (`default.toml` `volatile_breakeven_trigger`):
- USDJPY went 7/7 breakeven-scratch at 0.7 — SL snapped to entry at 70% of trail distance, then
  every minor pullback tagged it. At 0.9 the snap only fires when TP is nearly reached.

**Also:** removed the dead `trading_hours_utc = [0, 21]` from `[risk]` (the engine overwrites it
from `[trading_hours]` 07:00–20:00 at startup — one source of truth now, comment points there);
refreshed PROJECT_ARCHITECTURE.md (m15 consensus values, Phase 17.F, trading-hours note) and the
stale `0.75× ATR` headers in the two M15 strategy files.

**Pending (user decision):** suggestion #1 (per-instrument consecutive-loss cooldown) and
#4 (≥45-min same-instrument entry spacing — 06-10 showed stacked entries 15 min apart dying
together). **Investigation queue:** Asia-session GOLD backtest (on 06-11 every signal fell in
00–06 UTC and was rejected on hours); H1-zero bypass threshold (GOLD 2/3-consensus SELL at
strength 7.80 blocked, bypass needs ≥8.0).

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

## Phase History

Phases 1–17.D (Phase Summary table + per-phase details) moved to `docs/PHASE_HISTORY.md` to keep this file lean — load it only when researching old phases.

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
