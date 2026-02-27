-- IG Trading Engine — Initial Schema
-- Run: psql -d ig_trading -f migrations/001_initial.sql

-- ============================================
-- TRADES
-- ============================================
CREATE TABLE IF NOT EXISTS trades (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    deal_id         TEXT UNIQUE,
    deal_reference  TEXT,
    epic            TEXT NOT NULL,
    direction       TEXT NOT NULL CHECK (direction IN ('buy', 'sell')),
    size            DOUBLE PRECISION NOT NULL,
    entry_price     DOUBLE PRECISION NOT NULL,
    exit_price      DOUBLE PRECISION,
    stop_loss       DOUBLE PRECISION NOT NULL,
    take_profit     DOUBLE PRECISION,
    status          TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'closed', 'cancelled', 'rejected')),
    pnl             DOUBLE PRECISION,
    pnl_pct         DOUBLE PRECISION,
    strategy        TEXT NOT NULL,
    signal_strength DOUBLE PRECISION,
    risk_reward     DOUBLE PRECISION,
    opened_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    closed_at       TIMESTAMPTZ,
    duration_secs   INTEGER,
    notes           TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_trades_epic ON trades(epic);
CREATE INDEX idx_trades_status ON trades(status);
CREATE INDEX idx_trades_opened_at ON trades(opened_at);
CREATE INDEX idx_trades_strategy ON trades(strategy);

-- ============================================
-- SIGNALS
-- ============================================
CREATE TABLE IF NOT EXISTS signals (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    epic            TEXT NOT NULL,
    strategy        TEXT NOT NULL,
    direction       TEXT NOT NULL CHECK (direction IN ('buy', 'sell')),
    strength        DOUBLE PRECISION NOT NULL,
    stop_loss       DOUBLE PRECISION,
    take_profit     DOUBLE PRECISION,
    was_executed    BOOLEAN NOT NULL DEFAULT FALSE,
    rejection_reason TEXT,
    indicator_snapshot JSONB,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_signals_epic ON signals(epic);
CREATE INDEX idx_signals_created_at ON signals(created_at);
CREATE INDEX idx_signals_strategy ON signals(strategy);

-- ============================================
-- DAILY STATS
-- ============================================
CREATE TABLE IF NOT EXISTS daily_stats (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    date            DATE NOT NULL UNIQUE,
    starting_balance DOUBLE PRECISION NOT NULL,
    ending_balance  DOUBLE PRECISION,
    total_trades    INTEGER NOT NULL DEFAULT 0,
    winning_trades  INTEGER NOT NULL DEFAULT 0,
    losing_trades   INTEGER NOT NULL DEFAULT 0,
    gross_pnl       DOUBLE PRECISION NOT NULL DEFAULT 0,
    commissions     DOUBLE PRECISION NOT NULL DEFAULT 0,
    net_pnl         DOUBLE PRECISION NOT NULL DEFAULT 0,
    max_drawdown_pct DOUBLE PRECISION NOT NULL DEFAULT 0,
    max_open_positions INTEGER NOT NULL DEFAULT 0,
    circuit_breaker_hits INTEGER NOT NULL DEFAULT 0,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_daily_stats_date ON daily_stats(date);

-- ============================================
-- CANDLES (price history cache)
-- ============================================
CREATE TABLE IF NOT EXISTS candles (
    id              BIGSERIAL PRIMARY KEY,
    epic            TEXT NOT NULL,
    resolution      TEXT NOT NULL,
    open_price      DOUBLE PRECISION NOT NULL,
    high_price      DOUBLE PRECISION NOT NULL,
    low_price       DOUBLE PRECISION NOT NULL,
    close_price     DOUBLE PRECISION NOT NULL,
    volume          BIGINT NOT NULL DEFAULT 0,
    timestamp       TIMESTAMPTZ NOT NULL,
    UNIQUE(epic, resolution, timestamp)
);

CREATE INDEX idx_candles_epic_res_ts ON candles(epic, resolution, timestamp DESC);

-- ============================================
-- ENGINE EVENTS (audit log)
-- ============================================
CREATE TABLE IF NOT EXISTS engine_events (
    id              BIGSERIAL PRIMARY KEY,
    event_type      TEXT NOT NULL,
    severity        TEXT NOT NULL DEFAULT 'info' CHECK (severity IN ('debug', 'info', 'warning', 'error', 'critical')),
    message         TEXT NOT NULL,
    data            JSONB,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_engine_events_type ON engine_events(event_type);
CREATE INDEX idx_engine_events_created_at ON engine_events(created_at);
CREATE INDEX idx_engine_events_severity ON engine_events(severity);

-- ============================================
-- CONFIG SNAPSHOTS (track config changes)
-- ============================================
CREATE TABLE IF NOT EXISTS config_snapshots (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    config_toml     TEXT NOT NULL,
    changed_by      TEXT NOT NULL DEFAULT 'system',
    change_reason   TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ============================================
-- CLEANUP POLICY (auto-purge old data)
-- ============================================
-- Run periodically via cron or engine scheduler:
-- DELETE FROM candles WHERE timestamp < NOW() - INTERVAL '90 days';
-- DELETE FROM engine_events WHERE created_at < NOW() - INTERVAL '30 days';
-- DELETE FROM signals WHERE created_at < NOW() - INTERVAL '90 days';
