# Live Mode Pre-flight Checklist

Before switching the IG Trading Engine from `demo` or `paper` mode to `live` (real money), run through this checklist to ensure all systems are correctly configured and risk is bounded.

## 1. Environment & Credentials
- [ ] Ensure `.env` contains the correct **Live** credentials (`IG_API_KEY`, `IG_IDENTIFIER`, `IG_PASSWORD`).
- [ ] Set `IG_ENVIRONMENT=live` in `.env`.
- [ ] Verify you are using a **Live** account ID (if explicitly set in `IG_ACCOUNT_ID`).
- [ ] Test authentication manually: verify the engine can connect to `api.ig.com` (instead of `demo-api.ig.com`) during startup.
- [ ] Double-check that `.env` is **not** committed to version control.

## 2. Risk Management Configuration (`config/default.toml`)
- [ ] Verify `[general] mode = "live"`.
- [ ] Verify `max_risk_per_trade` is strictly adhered to (default 1%). Ensure account balance is sufficient to cover this percentage per trade minimums.
- [ ] Ensure `max_daily_loss_pct` is set (e.g., 3%) and circuit breakers are active.
- [ ] Confirm `max_open_positions` is reasonable (e.g., 3).
- [ ] If using a Limited Risk account, ensure `guaranteed_stops = true` is configured and correctly passed to order execution.
- [ ] Ensure `min_reward_to_risk` requires a positive expected value (e.g. 1.5).

## 3. Market & Data Configurations
- [ ] Check `epics` in `config/default.toml`. Ensure that the instruments you wish to trade use the correct **Live** epics (sometimes `CS.D.*` epics differ slightly between Demo and Live accounts).
- [ ] Confirm multi-timeframe variables are using standard IG API timeframe tokens (e.g., `MINUTE_15`, `HOUR`, `HOUR_4`).
- [ ] Verify historical data limits and rate limits — live rate limits may be stricter than demo environments.

## 4. Stability & Alerts
- [ ] Run a clean build: `cargo build --release`. You should run the production engine via the compiled release binary, not in debug mode.
- [ ] Check for unhandled code panics: ensure all `.unwrap()` references in critical async execution loops have been removed or explicitly validated.
- [ ] Ensure `TELEGRAM_BOT_TOKEN` and `TELEGRAM_CHAT_ID` are configured and tested. You **must** receive push notifications for trade executions and risk breaker events.
- [ ] Verify `RUST_LOG=info` (or `warn` in production) so your disk doesn't fill up with trace logs.

## 5. Deployment / Network
- [ ] If running on a VPS, verify the server time is synchronized via NTP (UTC preferred).
- [ ] Ensure internet connection is stable. A dropped WebSocket connection must correctly trigger the engine's auto-reconnect logic without stranding positions.
- [ ] Verify database (if used for logging) and disk have sufficient storage for continuous logging of ticks/candles.

## 6. Final Dry Run
- [ ] **Data Feed Test:** Run the bot with `[general] mode = "paper"` but against the `IG_ENVIRONMENT=live` to verify price tick parsing against live spreads and liquidity.
- [ ] **Manual Trade Test:** If possible, execute a 0.01 micro-lot manual triggered trade using `/trigger` endpoint on the lowest possible timeframe to verify end-to-end execution latency and Lightstreamer position syncing.
- [ ] **Go Live:** Only switch `mode = "live"` when confident. Monitor the first 3 trades manually.
