// ============================================
// Rust Engine Types
// ============================================

export interface EngineStatus {
    mode: 'paper' | 'demo' | 'live';
    status: 'starting' | 'running' | 'paused' | 'stopped' | 'error';
    uptime_secs: number;
    account: {
        balance: number;
        available: number;
        margin_used: number;
        pnl: number;
    } | null;
    open_positions: number;
    daily_stats: {
        trades_today: number;
        winning: number;
        losing: number;
        net_pnl: number;
        max_drawdown_pct: number;
    };
    circuit_breaker: {
        consecutive_losses: number;
        is_paused: boolean;
        size_multiplier: number;
    };
}

export interface EnginePosition {
    deal_id: string;
    epic: string;
    direction: 'buy' | 'sell';
    size: number;
    entry_price: number;
    current_price: number;
    stop_loss: number;
    take_profit: number | null;
    unrealised_pnl: number;
    strategy: string;
    opened_at: string;
}

export interface EngineSignal {
    epic: string;
    direction: 'buy' | 'sell';
    strategy: string;
    strength: number;
    stop_loss: number;
    take_profit: number;
    was_executed: boolean;
    rejection_reason: string | null;
    timestamp: string;
}

export interface IndicatorSnapshot {
    sma_short?: number;
    sma_long?: number;
    ema_short?: number;
    ema_long?: number;
    ema_200?: number;
    rsi?: number;
    macd?: number;
    macd_signal?: number;
    macd_histogram?: number;
    atr?: number;
    bollinger_upper?: number;
    bollinger_middle?: number;
    bollinger_lower?: number;
    adx?: number;
    stochastic_k?: number;
    stochastic_d?: number;
}

export interface EngineTrade {
    deal_id: string;
    epic: string;
    direction: 'buy' | 'sell';
    size: number;
    entry_price: number;
    exit_price: number | null;
    stop_loss: number;
    take_profit: number | null;
    pnl: number | null;
    strategy: string;
    status: 'open' | 'closed' | 'cancelled' | 'rejected';
    opened_at: string;
    closed_at: string | null;
}

export interface BacktestResult {
    total_trades: number;
    winning_trades: number;
    losing_trades: number;
    win_rate: number;
    total_pnl: number;
    total_pnl_pct: number;
    max_drawdown_pct: number;
    profit_factor: number;
    sharpe_ratio: number;
}

export interface BacktestTrade {
    entry_time: number;
    direction: 'BUY' | 'SELL';
    entry_price: number;
    exit_price?: number | null;
    pnl: number;
}

export interface BacktestResultFull extends BacktestResult {
    trades: BacktestTrade[];
}

export interface OptimizationRun {
    parameters: string;
    result: BacktestResult;
}

export interface OptimizationResult {
    best_pnl: number;
    best_parameters: string;
    top_runs: OptimizationRun[];
}

export interface EngineStats {
    daily: {
        trades: number;
        wins: number;
        losses: number;
        win_rate_pct: number;
        net_pnl: number;
        gross_profit: number;
        gross_loss: number;
        avg_win: number;
        avg_loss: number;
        profit_factor: number;
        max_drawdown: number;
        consecutive_losses: number;
    };
    all_time: {
        total_trades: number;
        equity_curve: Array<{
            timestamp: string;
            pnl: number;
            cumulative_pnl: number;
            epic: string;
            strategy: string;
        }>;
    };
    circuit_breaker: {
        active: boolean;
        consecutive_losses: number;
        size_multiplier: number;
    };
    timestamp: string;
}

export interface IndicatorsResponse {
    epic: string;
    available: boolean;
    message?: string;
    indicators?: IndicatorSnapshot;
    timestamp: string;
}

export interface SessionStats {
    win_rate: number;
    profit_factor: number;
}

export interface StrategyLearningEntry {
    name: string;
    win_rate: number;
    profit_factor: number;
    current_multiplier: number;
    effective_weight: number;
    max_consecutive_losses: number;
    trades_in_window: number;
    sessions: Record<string, SessionStats>;
}

export interface WeightAdjustment {
    strategy: string;
    old_weight: number;
    new_weight: number;
    win_rate: number;
    profit_factor: number;
    trade_count: number;
    timestamp: string;
}

export interface EngineLearning {
    total_trades_processed: number;
    strategies: StrategyLearningEntry[];
    recent_adjustments: WeightAdjustment[];
    timestamp: string;
}

export interface EngineConfig {
    mode: string;
    max_risk_per_trade: number;
    max_daily_loss_pct: number;
    max_open_positions: number;
    markets: string[];
    strategies: {
        ma_crossover: boolean;
        rsi_divergence: boolean;
        macd_momentum: boolean;
        bollinger_reversion: boolean;
        min_consensus: number;
        min_avg_strength: number;
    };
}

export interface EngineState {
    connected: boolean;
    status: EngineStatus | null;
    stats: EngineStats | null;
    learning: EngineLearning | null;
    positions: EnginePosition[];
    signals: EngineSignal[];
    trades: EngineTrade[];
    config: EngineConfig | null;
    indicators: Record<string, IndicatorSnapshot>;
    health: {
        uptime_secs: number;
        engine_status: string;
        connected_to_ig: boolean;
        version: string;
    } | null;
    optimizationResult: OptimizationResult | null;
    loading: boolean;
    error: string | null;
    lastUpdate: Date | null;
}
