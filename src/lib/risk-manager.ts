/**
 * Risk Manager
 * Comprehensive risk management for live trading
 */

import { getTradeLogger } from './trade-logger';
import { getNotificationService } from './notification-service';

export interface RiskConfig {
  // Per-trade limits
  maxRiskPerTrade: number; // % of account
  maxPositionSize: number; // Maximum lot size
  minStopLoss: number; // Minimum stop loss in points
  maxStopLoss: number; // Maximum stop loss in points
  
  // Daily limits
  maxDailyLoss: number; // $ amount
  maxDailyTrades: number;
  maxDailyDrawdown: number; // % of account
  
  // Overall limits
  maxOpenPositions: number;
  maxCorrelatedPositions: number;
  maxMarginUsage: number; // % of available margin
  
  // Time restrictions
  tradingStartTime: string; // HH:MM
  tradingEndTime: string; // HH:MM
  noTradeBeforeNews: number; // Minutes before high-impact news
  noTradeAfterNews: number; // Minutes after high-impact news
}

export interface RiskCheckResult {
  allowed: boolean;
  reason?: string;
  adjustedSize?: number;
  suggestedStopLoss?: number;
  suggestedTakeProfit?: number;
  warnings?: string[];
}

export interface AccountInfo {
  balance: number;
  available: number;
  margin: number;
  equity: number;
  openPnl: number;
}

export interface PositionInfo {
  epic: string;
  direction: 'BUY' | 'SELL';
  size: number;
  pnl: number;
}

// Default risk configuration
const DEFAULT_RISK_CONFIG: RiskConfig = {
  maxRiskPerTrade: 1,
  maxPositionSize: 5,
  minStopLoss: 10,
  maxStopLoss: 100,
  maxDailyLoss: 500,
  maxDailyTrades: 10,
  maxDailyDrawdown: 5,
  maxOpenPositions: 3,
  maxCorrelatedPositions: 1,
  maxMarginUsage: 50,
  tradingStartTime: '08:00',
  tradingEndTime: '17:00',
  noTradeBeforeNews: 30,
  noTradeAfterNews: 15
};

/**
 * Risk Manager Class
 */
export class RiskManager {
  private config: RiskConfig;
  private dailyStats = {
    date: new Date().toDateString(),
    trades: 0,
    pnl: 0,
    highWatermark: 0
  };

  constructor(config: Partial<RiskConfig> = {}) {
    this.config = { ...DEFAULT_RISK_CONFIG, ...config };
  }

  /**
   * Update risk configuration
   */
  updateConfig(config: Partial<RiskConfig>): void {
    this.config = { ...this.config, ...config };
  }

  /**
   * Get current configuration
   */
  getConfig(): RiskConfig {
    return { ...this.config };
  }

  /**
   * Check if trade is allowed
   */
  checkTrade(params: {
    account: AccountInfo;
    positions: PositionInfo[];
    epic: string;
    direction: 'BUY' | 'SELL';
    size: number;
    price: number;
    stopLoss?: number;
    takeProfit?: number;
  }): RiskCheckResult {
    const warnings: string[] = [];

    // Reset daily stats if new day
    this.resetDailyStatsIfNeeded();

    // 1. Check trading hours
    const timeCheck = this.checkTradingHours();
    if (!timeCheck.allowed) {
      return timeCheck;
    }

    // 2. Check daily trade limit
    if (this.dailyStats.trades >= this.config.maxDailyTrades) {
      return {
        allowed: false,
        reason: `Daily trade limit reached (${this.config.maxDailyTrades} trades)`
      };
    }

    // 3. Check daily loss limit
    if (this.dailyStats.pnl < -this.config.maxDailyLoss) {
      return {
        allowed: false,
        reason: `Daily loss limit reached ($${this.config.maxDailyLoss})`
      };
    }

    // 4. Check daily drawdown
    const drawdownPercent = (this.dailyStats.highWatermark - params.account.equity) / params.account.equity * 100;
    if (drawdownPercent > this.config.maxDailyDrawdown) {
      return {
        allowed: false,
        reason: `Max daily drawdown exceeded (${this.config.maxDailyDrawdown}%)`
      };
    }

    // 5. Check max open positions
    const openPositions = params.positions.length;
    if (openPositions >= this.config.maxOpenPositions) {
      return {
        allowed: false,
        reason: `Maximum open positions reached (${this.config.maxOpenPositions})`
      };
    }

    // 6. Check correlated positions
    const correlatedCheck = this.checkCorrelatedPositions(params.epic, params.positions);
    if (!correlatedCheck.allowed) {
      return correlatedCheck;
    }

    // 7. Check margin usage
    const marginCheck = this.checkMarginUsage(params.account, params.size, params.price);
    if (!marginCheck.allowed) {
      return marginCheck;
    }

    // 8. Calculate and validate position size
    const sizeResult = this.calculatePositionSize(params);
    if (sizeResult.adjustedSize && sizeResult.adjustedSize !== params.size) {
      warnings.push(`Position size adjusted from ${params.size} to ${sizeResult.adjustedSize}`);
    }

    // 9. Validate stop loss
    const stopLossResult = this.validateStopLoss(params.price, params.stopLoss, params.direction);
    if (!stopLossResult.valid) {
      return {
        allowed: false,
        reason: stopLossResult.reason
      };
    }

    // Log the check
    getTradeLogger().logInfo({
      message: `Risk check passed for ${params.epic} ${params.direction}`,
      details: {
        size: sizeResult.adjustedSize || params.size,
        warnings
      }
    });

    return {
      allowed: true,
      adjustedSize: sizeResult.adjustedSize,
      suggestedStopLoss: stopLossResult.suggested,
      suggestedTakeProfit: this.calculateTakeProfit(params.price, params.stopLoss, params.direction),
      warnings: warnings.length > 0 ? warnings : undefined
    };
  }

  /**
   * Calculate appropriate position size
   */
  calculatePositionSize(params: {
    account: AccountInfo;
    price: number;
    stopLoss?: number;
    direction: 'BUY' | 'SELL';
  }): { size: number; adjustedSize?: number } {
    // Risk amount based on account balance
    const riskAmount = params.account.balance * (this.config.maxRiskPerTrade / 100);

    // Calculate size based on stop loss
    let calculatedSize = this.config.maxPositionSize;

    if (params.stopLoss) {
      const stopDistance = Math.abs(params.price - params.stopLoss);
      if (stopDistance > 0) {
        // Size = Risk Amount / (Stop Distance * Point Value)
        const pointValue = 100; // Approximate for forex/gold
        calculatedSize = Math.min(
          riskAmount / (stopDistance * pointValue),
          this.config.maxPositionSize
        );
      }
    }

    // Round to appropriate size
    calculatedSize = Math.floor(calculatedSize * 10) / 10;

    return {
      size: params.account.available > 0 ? calculatedSize : 0,
      adjustedSize: calculatedSize !== this.config.maxPositionSize ? calculatedSize : undefined
    };
  }

  /**
   * Validate stop loss
   */
  private validateStopLoss(
    price: number,
    stopLoss?: number,
    direction?: 'BUY' | 'SELL'
  ): { valid: boolean; reason?: string; suggested?: number } {
    if (!stopLoss) {
      // Suggest a stop loss
      const suggestedDistance = this.config.minStopLoss;
      const suggested = direction === 'BUY'
        ? price - suggestedDistance
        : price + suggestedDistance;
      
      return {
        valid: true,
        suggested
      };
    }

    const stopDistance = Math.abs(price - stopLoss);

    if (stopDistance < this.config.minStopLoss) {
      return {
        valid: false,
        reason: `Stop loss too close (min: ${this.config.minStopLoss} points)`
      };
    }

    if (stopDistance > this.config.maxStopLoss) {
      return {
        valid: false,
        reason: `Stop loss too far (max: ${this.config.maxStopLoss} points)`
      };
    }

    return { valid: true };
  }

  /**
   * Calculate take profit based on risk:reward
   */
  private calculateTakeProfit(
    price: number,
    stopLoss?: number,
    direction?: 'BUY' | 'SELL'
  ): number | undefined {
    if (!stopLoss || !direction) return undefined;

    const stopDistance = Math.abs(price - stopLoss);
    const riskReward = 2; // 2:1 risk:reward ratio

    return direction === 'BUY'
      ? price + (stopDistance * riskReward)
      : price - (stopDistance * riskReward);
  }

  /**
   * Check trading hours
   */
  private checkTradingHours(): RiskCheckResult {
    const now = new Date();
    const currentTime = now.toTimeString().slice(0, 5);

    if (currentTime < this.config.tradingStartTime) {
      return {
        allowed: false,
        reason: `Trading not started (starts at ${this.config.tradingStartTime})`
      };
    }

    if (currentTime > this.config.tradingEndTime) {
      return {
        allowed: false,
        reason: `Trading ended (ends at ${this.config.tradingEndTime})`
      };
    }

    return { allowed: true };
  }

  /**
   * Check correlated positions
   */
  private checkCorrelatedPositions(
    epic: string,
    positions: PositionInfo[]
  ): RiskCheckResult {
    // Define correlated pairs
    const correlations: Record<string, string[]> = {
      'EURUSD': ['EURGBP', 'EURJPY', 'EURAUD'],
      'GBPUSD': ['EURGBP', 'GBPJPY', 'GBPAUD'],
      'USDJPY': ['EURJPY', 'GBPJPY', 'AUDJPY'],
      'AUDUSD': ['AUDJPY', 'EURAUD', 'GBPAUD'],
      'GOLD': ['XAUUSD', 'SILVER']
    };

    const correlated = correlations[epic] || [];
    const correlatedPositions = positions.filter(p =>
      correlated.includes(p.epic) || p.epic === epic
    );

    if (correlatedPositions.length >= this.config.maxCorrelatedPositions) {
      return {
        allowed: false,
        reason: `Too many correlated positions in ${epic} group`
      };
    }

    return { allowed: true };
  }

  /**
   * Check margin usage
   */
  private checkMarginUsage(
    account: AccountInfo,
    size: number,
    price: number
  ): RiskCheckResult {
    const marginRate = 0.05; // 5% margin
    const requiredMargin = size * price * marginRate;
    const potentialMarginUsage = ((account.margin + requiredMargin) / account.balance) * 100;

    if (potentialMarginUsage > this.config.maxMarginUsage) {
      return {
        allowed: false,
        reason: `Margin usage would exceed ${this.config.maxMarginUsage}%`
      };
    }

    return { allowed: true };
  }

  /**
   * Record a trade for daily tracking
   */
  recordTrade(pnl: number): void {
    this.resetDailyStatsIfNeeded();
    this.dailyStats.trades++;
    this.dailyStats.pnl += pnl;

    // Update high watermark
    if (this.dailyStats.pnl > this.dailyStats.highWatermark) {
      this.dailyStats.highWatermark = this.dailyStats.pnl;
    }

    // Check if daily loss limit approaching
    if (this.dailyStats.pnl < -this.config.maxDailyLoss * 0.8) {
      getNotificationService().sendRiskAlert({
        type: 'DAILY_LOSS_LIMIT',
        message: 'Approaching daily loss limit',
        current: Math.abs(this.dailyStats.pnl),
        limit: this.config.maxDailyLoss
      });
    }
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
        highWatermark: 0
      };
    }
  }

  /**
   * Get daily statistics
   */
  getDailyStats(): { date: string; trades: number; pnl: number; remainingTrades: number; remainingLoss: number } {
    this.resetDailyStatsIfNeeded();
    return {
      date: this.dailyStats.date,
      trades: this.dailyStats.trades,
      pnl: this.dailyStats.pnl,
      remainingTrades: Math.max(0, this.config.maxDailyTrades - this.dailyStats.trades),
      remainingLoss: Math.max(0, this.config.maxDailyLoss + this.dailyStats.pnl)
    };
  }
}

// Singleton instance
let riskManager: RiskManager | null = null;

export function getRiskManager(): RiskManager {
  if (!riskManager) {
    riskManager = new RiskManager();
  }
  return riskManager;
}

export function initRiskManager(config: Partial<RiskConfig>): RiskManager {
  riskManager = new RiskManager(config);
  return riskManager;
}
