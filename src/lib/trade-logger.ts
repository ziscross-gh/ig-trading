/**
 * Trade Logger Service
 * Persistent logging of all trades, signals, and bot activity
 */

export interface TradeLog {
  id: string;
  timestamp: Date;
  type: 'SIGNAL' | 'TRADE_OPEN' | 'TRADE_CLOSE' | 'TRADE_MODIFY' | 'ERROR' | 'INFO';
  epic: string;
  direction?: 'BUY' | 'SELL';
  size?: number;
  price?: number;
  stopLoss?: number;
  takeProfit?: number;
  pnl?: number;
  strategy?: string;
  reason?: string;
  error?: string;
  metadata?: Record<string, unknown>;
}

export interface DailySummary {
  date: string;
  totalTrades: number;
  winningTrades: number;
  losingTrades: number;
  totalPnl: number;
  winRate: number;
  largestWin: number;
  largestLoss: number;
  strategies: Record<string, { wins: number; losses: number; pnl: number }>;
}

// In-memory storage (replace with database in production)
const tradeLogs: TradeLog[] = [];
const MAX_LOGS = 10000; // Keep last 10k logs in memory

/**
 * Trade Logger Class
 */
export class TradeLogger {
  private logs: TradeLog[] = tradeLogs;
  private enabled: boolean = true;

  /**
   * Log a signal
   */
  logSignal(signal: {
    epic: string;
    direction: 'BUY' | 'SELL';
    strategy: string;
    strength: number;
    price: number;
    reason: string;
  }): void {
    this.addLog({
      id: this.generateId(),
      timestamp: new Date(),
      type: 'SIGNAL',
      epic: signal.epic,
      direction: signal.direction,
      price: signal.price,
      strategy: signal.strategy,
      reason: signal.reason,
      metadata: { strength: signal.strength }
    });
  }

  /**
   * Log trade open
   */
  logTradeOpen(trade: {
    dealId: string;
    epic: string;
    direction: 'BUY' | 'SELL';
    size: number;
    price: number;
    stopLoss?: number;
    takeProfit?: number;
    strategy?: string;
    reason?: string;
  }): void {
    this.addLog({
      id: trade.dealId,
      timestamp: new Date(),
      type: 'TRADE_OPEN',
      epic: trade.epic,
      direction: trade.direction,
      size: trade.size,
      price: trade.price,
      stopLoss: trade.stopLoss,
      takeProfit: trade.takeProfit,
      strategy: trade.strategy,
      reason: trade.reason
    });
  }

  /**
   * Log trade close
   */
  logTradeClose(trade: {
    dealId: string;
    epic: string;
    direction: 'BUY' | 'SELL';
    size: number;
    openPrice: number;
    closePrice: number;
    pnl: number;
    reason?: string;
  }): void {
    this.addLog({
      id: trade.dealId,
      timestamp: new Date(),
      type: 'TRADE_CLOSE',
      epic: trade.epic,
      direction: trade.direction,
      size: trade.size,
      price: trade.closePrice,
      pnl: trade.pnl,
      reason: trade.reason,
      metadata: { openPrice: trade.openPrice }
    });
  }

  /**
   * Log error
   */
  logError(error: {
    epic?: string;
    operation: string;
    message: string;
    details?: Record<string, unknown>;
  }): void {
    this.addLog({
      id: this.generateId(),
      timestamp: new Date(),
      type: 'ERROR',
      epic: error.epic || 'SYSTEM',
      error: error.message,
      reason: error.operation,
      metadata: error.details
    });
  }

  /**
   * Log info
   */
  logInfo(info: {
    epic?: string;
    message: string;
    details?: Record<string, unknown>;
  }): void {
    this.addLog({
      id: this.generateId(),
      timestamp: new Date(),
      type: 'INFO',
      epic: info.epic || 'SYSTEM',
      reason: info.message,
      metadata: info.details
    });
  }

  /**
   * Get logs
   */
  getLogs(filter?: {
    type?: TradeLog['type'];
    epic?: string;
    startDate?: Date;
    endDate?: Date;
    limit?: number;
  }): TradeLog[] {
    let filtered = [...this.logs];

    if (filter) {
      if (filter.type) {
        filtered = filtered.filter(l => l.type === filter.type);
      }
      if (filter.epic) {
        filtered = filtered.filter(l => l.epic === filter.epic);
      }
      if (filter.startDate) {
        filtered = filtered.filter(l => l.timestamp >= filter.startDate!);
      }
      if (filter.endDate) {
        filtered = filtered.filter(l => l.timestamp <= filter.endDate!);
      }
    }

    // Sort by timestamp descending
    filtered.sort((a, b) => b.timestamp.getTime() - a.timestamp.getTime());

    if (filter?.limit) {
      filtered = filtered.slice(0, filter.limit);
    }

    return filtered;
  }

  /**
   * Get daily summary
   */
  getDailySummary(date?: Date): DailySummary {
    const targetDate = date || new Date();
    const dateStr = targetDate.toISOString().split('T')[0];
    
    const dayLogs = this.logs.filter(l => 
      l.timestamp.toISOString().split('T')[0] === dateStr
    );

    const trades = dayLogs.filter(l => l.type === 'TRADE_CLOSE');
    const winningTrades = trades.filter(l => (l.pnl || 0) > 0);
    const losingTrades = trades.filter(l => (l.pnl || 0) < 0);

    const totalPnl = trades.reduce((sum, t) => sum + (t.pnl || 0), 0);
    const winRate = trades.length > 0 ? (winningTrades.length / trades.length) * 100 : 0;

    const pnls = trades.map(t => t.pnl || 0);
    const largestWin = Math.max(0, ...pnls);
    const largestLoss = Math.min(0, ...pnls);

    // Strategy breakdown
    const strategies: Record<string, { wins: number; losses: number; pnl: number }> = {};
    trades.forEach(t => {
      const strategy = t.strategy || 'Unknown';
      if (!strategies[strategy]) {
        strategies[strategy] = { wins: 0, losses: 0, pnl: 0 };
      }
      strategies[strategy].pnl += t.pnl || 0;
      if ((t.pnl || 0) > 0) {
        strategies[strategy].wins++;
      } else if ((t.pnl || 0) < 0) {
        strategies[strategy].losses++;
      }
    });

    return {
      date: dateStr,
      totalTrades: trades.length,
      winningTrades: winningTrades.length,
      losingTrades: losingTrades.length,
      totalPnl,
      winRate,
      largestWin,
      largestLoss,
      strategies
    };
  }

  /**
   * Get trade history
   */
  getTradeHistory(limit: number = 100): TradeLog[] {
    return this.getLogs({ type: 'TRADE_CLOSE', limit });
  }

  /**
   * Get signal history
   */
  getSignalHistory(limit: number = 100): TradeLog[] {
    return this.getLogs({ type: 'SIGNAL', limit });
  }

  /**
   * Export logs to JSON
   */
  exportLogs(): string {
    return JSON.stringify(this.logs, null, 2);
  }

  /**
   * Import logs from JSON
   */
  importLogs(json: string): number {
    try {
      const imported = JSON.parse(json) as TradeLog[];
      imported.forEach(log => {
        log.timestamp = new Date(log.timestamp);
      });
      this.logs.push(...imported);
      this.trimLogs();
      return imported.length;
    } catch {
      return 0;
    }
  }

  /**
   * Clear all logs
   */
  clearLogs(): void {
    this.logs.length = 0;
  }

  /**
   * Enable/disable logging
   */
  setEnabled(enabled: boolean): void {
    this.enabled = enabled;
  }

  /**
   * Add log entry
   */
  private addLog(log: TradeLog): void {
    if (!this.enabled) return;
    
    this.logs.push(log);
    this.trimLogs();
    
    // Log to console in development
    if (process.env.NODE_ENV === 'development') {
      console.log(`[TradeLogger] ${log.type}:`, log);
    }
  }

  /**
   * Trim logs to max size
   */
  private trimLogs(): void {
    if (this.logs.length > MAX_LOGS) {
      this.logs.splice(0, this.logs.length - MAX_LOGS);
    }
  }

  /**
   * Generate unique ID
   */
  private generateId(): string {
    return `${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
  }
}

// Singleton instance
let tradeLogger: TradeLogger | null = null;

export function getTradeLogger(): TradeLogger {
  if (!tradeLogger) {
    tradeLogger = new TradeLogger();
  }
  return tradeLogger;
}
