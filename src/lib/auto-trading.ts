/**
 * Auto-Trading Engine
 * Automatically executes trades based on strategy signals
 */

import { getRiskManager } from './risk-manager';
import { getTradeLogger } from './trade-logger';
import { getNotificationService } from './notification-service';
import { getAlertManager } from './alert-system';

export interface AutoTradingConfig {
  enabled: boolean;
  maxOpenTrades: number;
  riskPerTrade: number; // % of account
  defaultStopLoss: number; // ATR multiplier
  defaultTakeProfit: number; // ATR multiplier
  trailingStop: boolean;
  trailingStopATR: number;
  breakEvenAfter: number; // Move to break even after X ATR profit
  maxDailyTrades: number;
  maxDailyLoss: number;
  tradingHours: {
    start: string;
    end: string;
  };
  strategies: {
    name: string;
    enabled: boolean;
    weight: number;
    minSignalStrength: number;
  }[];
  markets: {
    epic: string;
    enabled: boolean;
    maxPositionSize: number;
  }[];
}

export interface TradingSignal {
  id: string;
  timestamp: Date;
  epic: string;
  direction: 'BUY' | 'SELL';
  strength: number; // 1-10
  strategy: string;
  reason: string;
  price: number;
  indicators: Record<string, number>;
  executed: boolean;
}

export interface TradeDecision {
  action: 'OPEN' | 'CLOSE' | 'HOLD';
  epic: string;
  direction?: 'BUY' | 'SELL';
  size?: number;
  stopLoss?: number;
  takeProfit?: number;
  reason: string;
  confidence: number;
}

const DEFAULT_CONFIG: AutoTradingConfig = {
  enabled: false,
  maxOpenTrades: 3,
  riskPerTrade: 1,
  defaultStopLoss: 2,
  defaultTakeProfit: 3,
  trailingStop: true,
  trailingStopATR: 1.5,
  breakEvenAfter: 1,
  maxDailyTrades: 5,
  maxDailyLoss: 500,
  tradingHours: {
    start: '08:00',
    end: '17:00'
  },
  strategies: [
    { name: 'MA_CROSSOVER', enabled: true, weight: 1, minSignalStrength: 6 },
    { name: 'RSI_STRATEGY', enabled: true, weight: 0.8, minSignalStrength: 7 },
    { name: 'MACD_SIGNAL', enabled: true, weight: 1, minSignalStrength: 6 },
    { name: 'BOLLINGER_BANDS', enabled: true, weight: 0.7, minSignalStrength: 6 }
  ],
  markets: [
    { epic: 'CS.D.GOLDUSD.CFD', enabled: true, maxPositionSize: 1 },
    { epic: 'CS.D.EURUSD.CFD', enabled: true, maxPositionSize: 2 },
    { epic: 'CS.D.GBPUSD.CFD', enabled: true, maxPositionSize: 2 },
    { epic: 'CS.D.USDJPY.CFD', enabled: true, maxPositionSize: 2 }
  ]
};

/**
 * Auto-Trading Engine Class
 */
export class AutoTradingEngine {
  private config: AutoTradingConfig;
  private signals: TradingSignal[] = [];
  private dailyStats = {
    date: new Date().toDateString(),
    trades: 0,
    pnl: 0,
    wins: 0,
    losses: 0
  };
  private lastAnalysis: Map<string, Date> = new Map();

  constructor(config: Partial<AutoTradingConfig> = {}) {
    this.config = { ...DEFAULT_CONFIG, ...config };
  }

  /**
   * Update configuration
   */
  updateConfig(config: Partial<AutoTradingConfig>): void {
    this.config = { ...this.config, ...config };
  }

  /**
   * Get current configuration
   */
  getConfig(): AutoTradingConfig {
    return { ...this.config };
  }

  /**
   * Enable/disable auto-trading
   */
  setEnabled(enabled: boolean): void {
    this.config.enabled = enabled;
    
    getNotificationService().sendBotStatus({
      action: enabled ? 'STARTED' : 'STOPPED',
      message: enabled ? 'Auto-trading enabled' : 'Auto-trading disabled'
    });
  }

  /**
   * Check if auto-trading is enabled
   */
  isEnabled(): boolean {
    return this.config.enabled;
  }

  /**
   * Process a trading signal
   */
  processSignal(signal: Omit<TradingSignal, 'id' | 'timestamp' | 'executed'>): TradeDecision {
    const fullSignal: TradingSignal = {
      ...signal,
      id: `SIG_${Date.now()}_${Math.random().toString(36).substr(2, 6)}`,
      timestamp: new Date(),
      executed: false
    };

    this.signals.push(fullSignal);

    // Check if trading is enabled
    if (!this.config.enabled) {
      return { action: 'HOLD', epic: signal.epic, reason: 'Auto-trading disabled', confidence: 0 };
    }

    // Check trading hours
    if (!this.isWithinTradingHours()) {
      return { action: 'HOLD', epic: signal.epic, reason: 'Outside trading hours', confidence: 0 };
    }

    // Check daily limits
    this.resetDailyStatsIfNeeded();
    if (this.dailyStats.trades >= this.config.maxDailyTrades) {
      return { action: 'HOLD', epic: signal.epic, reason: 'Max daily trades reached', confidence: 0 };
    }
    if (this.dailyStats.pnl <= -this.config.maxDailyLoss) {
      return { action: 'HOLD', epic: signal.epic, reason: 'Max daily loss reached', confidence: 0 };
    }

    // Check if market is enabled
    const marketConfig = this.config.markets.find(m => m.epic === signal.epic);
    if (!marketConfig?.enabled) {
      return { action: 'HOLD', epic: signal.epic, reason: 'Market not enabled for trading', confidence: 0 };
    }

    // Check strategy configuration
    const strategyConfig = this.config.strategies.find(s => s.name === signal.strategy);
    if (!strategyConfig?.enabled) {
      return { action: 'HOLD', epic: signal.epic, reason: 'Strategy not enabled', confidence: 0 };
    }

    // Check signal strength
    if (signal.strength < strategyConfig.minSignalStrength) {
      return { action: 'HOLD', epic: signal.epic, reason: `Signal strength ${signal.strength} below minimum ${strategyConfig.minSignalStrength}`, confidence: 50 };
    }

    // Calculate confidence
    const confidence = this.calculateConfidence(signal, strategyConfig);

    // Make decision
    const decision: TradeDecision = {
      action: 'OPEN',
      epic: signal.epic,
      direction: signal.direction,
      size: Math.min(marketConfig.maxPositionSize, this.calculatePositionSize(signal)),
      stopLoss: this.calculateStopLoss(signal),
      takeProfit: this.calculateTakeProfit(signal),
      reason: signal.reason,
      confidence
    };

    return decision;
  }

  /**
   * Execute a trade decision
   */
  async executeDecision(decision: TradeDecision): Promise<{
    success: boolean;
    tradeId?: string;
    error?: string;
  }> {
    if (decision.action !== 'OPEN') {
      return { success: false, error: 'No trade to execute' };
    }

    try {
      // Log the trade
      const tradeId = `TRADE_${Date.now()}`;
      
      getTradeLogger().logTradeOpen({
        dealId: tradeId,
        epic: decision.epic,
        direction: decision.direction!,
        size: decision.size!,
        price: decision.price || 0,
        stopLoss: decision.stopLoss,
        takeProfit: decision.takeProfit,
        reason: decision.reason
      });

      // Update daily stats
      this.dailyStats.trades++;

      // Send notification
      await getNotificationService().sendTradeAlert({
        action: 'OPEN',
        epic: decision.epic,
        direction: decision.direction!,
        size: decision.size!,
        price: decision.price || 0,
        reason: decision.reason
      });

      // Create alert
      getAlertManager().createTradeAlert({
        type: 'ENTRY',
        epic: decision.epic,
        message: `${decision.direction} ${decision.size} @ ${decision.price}`,
        severity: 'INFO'
      });

      return { success: true, tradeId };

    } catch (error) {
      return {
        success: false,
        error: error instanceof Error ? error.message : 'Trade execution failed'
      };
    }
  }

  /**
   * Process multiple signals and find best opportunity
   */
  findBestOpportunity(signals: TradingSignal[]): TradingSignal | null {
    if (signals.length === 0) return null;

    // Filter by enabled strategies and markets
    const validSignals = signals.filter(signal => {
      const strategy = this.config.strategies.find(s => s.name === signal.strategy);
      const market = this.config.markets.find(m => m.epic === signal.epic);
      return strategy?.enabled && market?.enabled && signal.strength >= strategy.minSignalStrength;
    });

    if (validSignals.length === 0) return null;

    // Sort by weighted strength
    validSignals.sort((a, b) => {
      const aWeight = this.config.strategies.find(s => s.name === a.strategy)?.weight || 1;
      const bWeight = this.config.strategies.find(s => s.name === b.strategy)?.weight || 1;
      return (b.strength * bWeight) - (a.strength * aWeight);
    });

    return validSignals[0];
  }

  /**
   * Check if we should close a position
   */
  shouldClosePosition(params: {
    epic: string;
    direction: 'BUY' | 'SELL';
    entryPrice: number;
    currentPrice: number;
    stopLoss: number;
    takeProfit: number;
    trailingStop?: number;
    atr: number;
  }): { close: boolean; reason: string; newStopLoss?: number } {
    const { epic, direction, entryPrice, currentPrice, stopLoss, takeProfit, trailingStop, atr } = params;

    // Calculate P&L
    let pnl: number;
    if (direction === 'BUY') {
      pnl = currentPrice - entryPrice;
    } else {
      pnl = entryPrice - currentPrice;
    }

    // Check take profit
    if (direction === 'BUY' && currentPrice >= takeProfit) {
      return { close: true, reason: 'Take profit reached' };
    }
    if (direction === 'SELL' && currentPrice <= takeProfit) {
      return { close: true, reason: 'Take profit reached' };
    }

    // Check stop loss
    if (direction === 'BUY' && currentPrice <= stopLoss) {
      return { close: true, reason: 'Stop loss hit' };
    }
    if (direction === 'SELL' && currentPrice >= stopLoss) {
      return { close: true, reason: 'Stop loss hit' };
    }

    // Trailing stop logic
    if (this.config.trailingStop && pnl > 0) {
      const trailingDistance = this.config.trailingStopATR * atr;
      let newStopLoss = stopLoss;

      if (direction === 'BUY') {
        newStopLoss = Math.max(stopLoss, currentPrice - trailingDistance);
      } else {
        newStopLoss = Math.min(stopLoss, currentPrice + trailingDistance);
      }

      // Break even logic
      if (this.config.breakEvenAfter > 0 && pnl > this.config.breakEvenAfter * atr) {
        if (direction === 'BUY') {
          newStopLoss = Math.max(newStopLoss, entryPrice);
        } else {
          newStopLoss = Math.min(newStopLoss, entryPrice);
        }
      }

      if (newStopLoss !== stopLoss) {
        return { close: false, reason: 'Update trailing stop', newStopLoss };
      }
    }

    return { close: false, reason: '' };
  }

  /**
   * Get recent signals
   */
  getRecentSignals(limit: number = 50): TradingSignal[] {
    return this.signals.slice(-limit);
  }

  /**
   * Get daily statistics
   */
  getDailyStats(): typeof this.dailyStats {
    this.resetDailyStatsIfNeeded();
    return { ...this.dailyStats };
  }

  // ==================== PRIVATE METHODS ====================

  /**
   * Calculate confidence score
   */
  private calculateConfidence(signal: TradingSignal, strategyConfig: { weight: number; minSignalStrength: number }): number {
    // Base confidence from signal strength
    let confidence = (signal.strength / 10) * 60;

    // Add strategy weight contribution
    confidence += strategyConfig.weight * 10;

    // Check for confluence (other signals on same epic)
    const sameEpicSignals = this.signals.filter(s => 
      s.epic === signal.epic && 
      s.direction === signal.direction &&
      s.timestamp.getTime() > Date.now() - 3600000 // Last hour
    );
    
    if (sameEpicSignals.length > 1) {
      confidence += 10;
    }

    return Math.min(100, Math.round(confidence));
  }

  /**
   * Calculate position size based on risk
   */
  private calculatePositionSize(signal: TradingSignal): number {
    // Simple calculation based on signal strength
    return Math.min(this.config.riskPerTrade, signal.strength / 10);
  }

  /**
   * Calculate stop loss level
   */
  private calculateStopLoss(signal: TradingSignal): number {
    const atr = signal.indicators.ATR || (signal.price * 0.01);
    const distance = atr * this.config.defaultStopLoss;
    
    return signal.direction === 'BUY'
      ? signal.price - distance
      : signal.price + distance;
  }

  /**
   * Calculate take profit level
   */
  private calculateTakeProfit(signal: TradingSignal): number {
    const atr = signal.indicators.ATR || (signal.price * 0.01);
    const distance = atr * this.config.defaultTakeProfit;
    
    return signal.direction === 'BUY'
      ? signal.price + distance
      : signal.price - distance;
  }

  /**
   * Check if within trading hours
   */
  private isWithinTradingHours(): boolean {
    const now = new Date();
    const currentTime = now.toTimeString().slice(0, 5);
    return currentTime >= this.config.tradingHours.start && 
           currentTime <= this.config.tradingHours.end;
  }

  /**
   * Reset daily stats if new day
   */
  private resetDailyStatsIfNeeded(): void {
    const today = new Date().toDateString();
    if (this.dailyStats.date !== today) {
      this.dailyStats = {
        date: today,
        trades: 0,
        pnl: 0,
        wins: 0,
        losses: 0
      };
    }
  }
}

// Singleton instance
let autoTradingInstance: AutoTradingEngine | null = null;

export function getAutoTradingEngine(config?: Partial<AutoTradingConfig>): AutoTradingEngine {
  if (!autoTradingInstance) {
    autoTradingInstance = new AutoTradingEngine(config);
  }
  return autoTradingInstance;
}

export function initAutoTradingEngine(config: Partial<AutoTradingConfig>): AutoTradingEngine {
  autoTradingInstance = new AutoTradingEngine(config);
  return autoTradingInstance;
}
