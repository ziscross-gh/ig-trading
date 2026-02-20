/**
 * Paper Trading Engine
 * Simulates trades without real execution for testing purposes
 */

export interface PaperPosition {
  id: string;
  epic: string;
  marketName: string;
  direction: 'BUY' | 'SELL';
  size: number;
  openPrice: number;
  currentPrice: number;
  stopLevel?: number;
  limitLevel?: number;
  pnl: number;
  pnlPercent: number;
  openTime: Date;
  unrealizedPnl: number;
}

export interface PaperTrade {
  id: string;
  epic: string;
  marketName: string;
  direction: 'BUY' | 'SELL';
  size: number;
  price: number;
  stopLevel?: number;
  limitLevel?: number;
  pnl: number;
  timestamp: Date;
  reason: string;
}

export interface PaperTradingAccount {
  balance: number;
  available: number;
  margin: number;
  equity: number;
  openPositions: PaperPosition[];
  tradeHistory: PaperTrade[];
  dailyPnl: number;
  weeklyPnl: number;
  totalTrades: number;
  winRate: number;
}

export class PaperTradingEngine {
  private positions: Map<string, PaperPosition> = new Map();
  private tradeHistory: PaperTrade[] = [];
  private balance: number;
  private initialBalance: number;
  private dailyStartBalance: number;
  private weeklyStartBalance: number;
  private marginUsed: number = 0;
  private marginRate: number = 0.05; // 5% margin requirement

  constructor(initialBalance: number = 10000) {
    this.balance = initialBalance;
    this.initialBalance = initialBalance;
    this.dailyStartBalance = initialBalance;
    this.weeklyStartBalance = initialBalance;
  }

  /**
   * Open a new paper position
   */
  openPosition(
    epic: string,
    marketName: string,
    direction: 'BUY' | 'SELL',
    size: number,
    price: number,
    stopLevel?: number,
    limitLevel?: number
  ): { success: boolean; position?: PaperPosition; error?: string } {
    // Check if position already exists for this epic
    if (this.positions.has(epic)) {
      return { success: false, error: 'Position already exists for this market' };
    }

    // Calculate margin requirement
    const marginRequired = size * price * this.marginRate;
    
    if (marginRequired > this.balance - this.marginUsed) {
      return { success: false, error: 'Insufficient margin' };
    }

    const position: PaperPosition = {
      id: `paper_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`,
      epic,
      marketName,
      direction,
      size,
      openPrice: price,
      currentPrice: price,
      stopLevel,
      limitLevel,
      pnl: 0,
      pnlPercent: 0,
      openTime: new Date(),
      unrealizedPnl: 0
    };

    this.positions.set(epic, position);
    this.marginUsed += marginRequired;

    return { success: true, position };
  }

  /**
   * Close a paper position
   */
  closePosition(
    epic: string,
    closePrice: number,
    reason: string = 'Manual close'
  ): { success: boolean; trade?: PaperTrade; error?: string } {
    const position = this.positions.get(epic);
    
    if (!position) {
      return { success: false, error: 'Position not found' };
    }

    // Calculate final P&L
    let pnl: number;
    if (position.direction === 'BUY') {
      pnl = (closePrice - position.openPrice) * position.size * 100; // Points to $
    } else {
      pnl = (position.openPrice - closePrice) * position.size * 100;
    }

    const trade: PaperTrade = {
      id: `trade_${Date.now()}`,
      epic: position.epic,
      marketName: position.marketName,
      direction: position.direction,
      size: position.size,
      price: closePrice,
      stopLevel: position.stopLevel,
      limitLevel: position.limitLevel,
      pnl,
      timestamp: new Date(),
      reason
    };

    // Update balance
    this.balance += pnl;
    this.tradeHistory.push(trade);
    this.positions.delete(epic);
    
    // Recalculate margin
    const marginReleased = position.size * position.openPrice * this.marginRate;
    this.marginUsed -= marginReleased;

    return { success: true, trade };
  }

  /**
   * Update position prices and check stop/limit
   */
  updatePrices(prices: Map<string, number>): PaperTrade[] {
    const closedTrades: PaperTrade[] = [];

    this.positions.forEach((position, epic) => {
      const currentPrice = prices.get(epic);
      if (!currentPrice) return;

      position.currentPrice = currentPrice;

      // Calculate unrealized P&L
      if (position.direction === 'BUY') {
        position.unrealizedPnl = (currentPrice - position.openPrice) * position.size * 100;
      } else {
        position.unrealizedPnl = (position.openPrice - currentPrice) * position.size * 100;
      }

      position.pnl = position.unrealizedPnl;
      position.pnlPercent = (position.pnl / (position.openPrice * position.size * 100)) * 100;

      // Check stop loss
      if (position.stopLevel) {
        if (position.direction === 'BUY' && currentPrice <= position.stopLevel) {
          const result = this.closePosition(epic, position.stopLevel, 'Stop Loss Hit');
          if (result.trade) closedTrades.push(result.trade);
        } else if (position.direction === 'SELL' && currentPrice >= position.stopLevel) {
          const result = this.closePosition(epic, position.stopLevel, 'Stop Loss Hit');
          if (result.trade) closedTrades.push(result.trade);
        }
      }

      // Check take profit
      if (position.limitLevel) {
        if (position.direction === 'BUY' && currentPrice >= position.limitLevel) {
          const result = this.closePosition(epic, position.limitLevel, 'Take Profit Hit');
          if (result.trade) closedTrades.push(result.trade);
        } else if (position.direction === 'SELL' && currentPrice <= position.limitLevel) {
          const result = this.closePosition(epic, position.limitLevel, 'Take Profit Hit');
          if (result.trade) closedTrades.push(result.trade);
        }
      }
    });

    return closedTrades;
  }

  /**
   * Get account summary
   */
  getAccount(): PaperTradingAccount {
    let totalUnrealizedPnl = 0;
    const positionsArray: PaperPosition[] = [];

    this.positions.forEach(position => {
      totalUnrealizedPnl += position.unrealizedPnl;
      positionsArray.push(position);
    });

    const equity = this.balance + totalUnrealizedPnl;
    
    // Calculate win rate
    const closedTrades = this.tradeHistory.filter(t => t.pnl !== undefined);
    const winningTrades = closedTrades.filter(t => t.pnl > 0);
    const winRate = closedTrades.length > 0 
      ? (winningTrades.length / closedTrades.length) * 100 
      : 0;

    return {
      balance: this.balance,
      available: this.balance - this.marginUsed,
      margin: this.marginUsed,
      equity,
      openPositions: positionsArray,
      tradeHistory: this.tradeHistory,
      dailyPnl: this.balance - this.dailyStartBalance,
      weeklyPnl: this.balance - this.weeklyStartBalance,
      totalTrades: this.tradeHistory.length,
      winRate
    };
  }

  /**
   * Reset daily/weekly tracking
   */
  resetDailyTracking(): void {
    this.dailyStartBalance = this.balance;
  }

  resetWeeklyTracking(): void {
    this.weeklyStartBalance = this.balance;
  }

  /**
   * Reset entire account
   */
  resetAccount(initialBalance?: number): void {
    const balance = initialBalance ?? this.initialBalance;
    this.balance = balance;
    this.initialBalance = balance;
    this.dailyStartBalance = balance;
    this.weeklyStartBalance = balance;
    this.marginUsed = 0;
    this.positions.clear();
    this.tradeHistory = [];
  }

  /**
   * Get performance metrics
   */
  getPerformanceMetrics(): {
    totalReturn: number;
    totalReturnPercent: number;
    maxDrawdown: number;
    sharpeRatio: number;
    profitFactor: number;
    averageWin: number;
    averageLoss: number;
    largestWin: number;
    largestLoss: number;
  } {
    const closedTrades = this.tradeHistory.filter(t => t.pnl !== undefined);
    
    if (closedTrades.length === 0) {
      return {
        totalReturn: 0,
        totalReturnPercent: 0,
        maxDrawdown: 0,
        sharpeRatio: 0,
        profitFactor: 0,
        averageWin: 0,
        averageLoss: 0,
        largestWin: 0,
        largestLoss: 0
      };
    }

    const totalReturn = this.balance - this.initialBalance;
    const totalReturnPercent = (totalReturn / this.initialBalance) * 100;

    // Calculate drawdown
    let peak = this.initialBalance;
    let maxDrawdown = 0;
    let runningBalance = this.initialBalance;
    
    closedTrades.forEach(trade => {
      runningBalance += trade.pnl;
      if (runningBalance > peak) peak = runningBalance;
      const drawdown = (peak - runningBalance) / peak * 100;
      if (drawdown > maxDrawdown) maxDrawdown = drawdown;
    });

    // Calculate metrics
    const wins = closedTrades.filter(t => t.pnl > 0);
    const losses = closedTrades.filter(t => t.pnl < 0);
    
    const totalWins = wins.reduce((sum, t) => sum + t.pnl, 0);
    const totalLosses = Math.abs(losses.reduce((sum, t) => sum + t.pnl, 0));

    const profitFactor = totalLosses > 0 ? totalWins / totalLosses : totalWins > 0 ? Infinity : 0;
    const averageWin = wins.length > 0 ? totalWins / wins.length : 0;
    const averageLoss = losses.length > 0 ? totalLosses / losses.length : 0;
    const largestWin = wins.length > 0 ? Math.max(...wins.map(t => t.pnl)) : 0;
    const largestLoss = losses.length > 0 ? Math.min(...losses.map(t => t.pnl)) : 0;

    // Simple Sharpe ratio (using trade returns)
    const returns = closedTrades.map(t => t.pnl);
    const avgReturn = returns.reduce((a, b) => a + b, 0) / returns.length;
    const variance = returns.reduce((sum, r) => sum + Math.pow(r - avgReturn, 2), 0) / returns.length;
    const stdDev = Math.sqrt(variance);
    const sharpeRatio = stdDev > 0 ? avgReturn / stdDev : 0;

    return {
      totalReturn,
      totalReturnPercent,
      maxDrawdown,
      sharpeRatio,
      profitFactor,
      averageWin,
      averageLoss,
      largestWin,
      largestLoss
    };
  }
}

// Singleton instance for paper trading
let paperTradingInstance: PaperTradingEngine | null = null;

export function getPaperTradingEngine(initialBalance?: number): PaperTradingEngine {
  if (!paperTradingInstance) {
    paperTradingInstance = new PaperTradingEngine(initialBalance);
  }
  return paperTradingInstance;
}

export function resetPaperTradingEngine(initialBalance?: number): PaperTradingEngine {
  paperTradingInstance = new PaperTradingEngine(initialBalance);
  return paperTradingInstance;
}
