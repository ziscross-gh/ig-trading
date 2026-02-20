/**
 * Alert System
 * Price alerts, indicator alerts, and custom notifications
 */

export interface PriceAlert {
  id: string;
  epic: string;
  name: string;
  type: 'PRICE_ABOVE' | 'PRICE_BELOW';
  targetPrice: number;
  currentPrice: number;
  status: 'ACTIVE' | 'TRIGGERED' | 'CANCELLED';
  createdAt: Date;
  triggeredAt?: Date;
  message?: string;
  repeat: boolean;
}

export interface IndicatorAlert {
  id: string;
  epic: string;
  name: string;
  indicator: 'RSI' | 'MACD' | 'MA_CROSS' | 'BB_TOUCH' | 'ATR_BREAK';
  condition: 'CROSSES_ABOVE' | 'CROSSES_BELOW' | 'ENTERS_ZONE' | 'EXITS_ZONE';
  value: number;
  currentValue: number;
  status: 'ACTIVE' | 'TRIGGERED' | 'CANCELLED';
  createdAt: Date;
  triggeredAt?: Date;
  message?: string;
}

export interface TradeAlert {
  id: string;
  type: 'ENTRY' | 'EXIT' | 'STOP_LOSS' | 'TAKE_PROFIT' | 'DAILY_LOSS' | 'MARGIN';
  epic?: string;
  message: string;
  severity: 'INFO' | 'WARNING' | 'CRITICAL';
  timestamp: Date;
  acknowledged: boolean;
}

export type AnyAlert = PriceAlert | IndicatorAlert | TradeAlert;

/**
 * Alert Manager Class
 */
export class AlertManager {
  private priceAlerts: Map<string, PriceAlert> = new Map();
  private indicatorAlerts: Map<string, IndicatorAlert> = new Map();
  private tradeAlerts: TradeAlert[] = [];
  private maxAlerts = 100;
  private onAlertTriggered?: (alert: AnyAlert) => void;

  /**
   * Set callback for when alerts are triggered
   */
  setAlertCallback(callback: (alert: AnyAlert) => void): void {
    this.onAlertTriggered = callback;
  }

  // ==================== PRICE ALERTS ====================

  /**
   * Create a price alert
   */
  createPriceAlert(params: {
    epic: string;
    name: string;
    type: 'PRICE_ABOVE' | 'PRICE_BELOW';
    targetPrice: number;
    currentPrice: number;
    repeat?: boolean;
    message?: string;
  }): PriceAlert {
    const alert: PriceAlert = {
      id: `PA_${Date.now()}_${Math.random().toString(36).substr(2, 6)}`,
      epic: params.epic,
      name: params.name,
      type: params.type,
      targetPrice: params.targetPrice,
      currentPrice: params.currentPrice,
      status: 'ACTIVE',
      createdAt: new Date(),
      message: params.message,
      repeat: params.repeat || false
    };

    this.priceAlerts.set(alert.id, alert);
    return alert;
  }

  /**
   * Check price alerts against current price
   */
  checkPriceAlerts(epic: string, currentPrice: number): PriceAlert[] {
    const triggered: PriceAlert[] = [];

    this.priceAlerts.forEach((alert, id) => {
      if (alert.epic !== epic || alert.status !== 'ACTIVE') return;

      let shouldTrigger = false;
      if (alert.type === 'PRICE_ABOVE' && currentPrice >= alert.targetPrice) {
        shouldTrigger = true;
      } else if (alert.type === 'PRICE_BELOW' && currentPrice <= alert.targetPrice) {
        shouldTrigger = true;
      }

      if (shouldTrigger) {
        alert.triggeredAt = new Date();
        alert.currentPrice = currentPrice;
        
        if (alert.repeat) {
          // Create a new alert for next trigger
          const newAlert = { ...alert };
          newAlert.id = `PA_${Date.now()}_${Math.random().toString(36).substr(2, 6)}`;
          newAlert.status = 'ACTIVE';
          newAlert.triggeredAt = undefined;
          newAlert.createdAt = new Date();
          this.priceAlerts.set(newAlert.id, newAlert);
        }
        
        alert.status = 'TRIGGERED';
        triggered.push(alert);
        
        // Callback
        if (this.onAlertTriggered) {
          this.onAlertTriggered(alert);
        }
      }
    });

    return triggered;
  }

  // ==================== INDICATOR ALERTS ====================

  /**
   * Create an indicator alert
   */
  createIndicatorAlert(params: {
    epic: string;
    name: string;
    indicator: 'RSI' | 'MACD' | 'MA_CROSS' | 'BB_TOUCH' | 'ATR_BREAK';
    condition: 'CROSSES_ABOVE' | 'CROSSES_BELOW' | 'ENTERS_ZONE' | 'EXITS_ZONE';
    value: number;
    currentValue: number;
    message?: string;
  }): IndicatorAlert {
    const alert: IndicatorAlert = {
      id: `IA_${Date.now()}_${Math.random().toString(36).substr(2, 6)}`,
      epic: params.epic,
      name: params.name,
      indicator: params.indicator,
      condition: params.condition,
      value: params.value,
      currentValue: params.currentValue,
      status: 'ACTIVE',
      createdAt: new Date(),
      message: params.message
    };

    this.indicatorAlerts.set(alert.id, alert);
    return alert;
  }

  /**
   * Check RSI alerts
   */
  checkRSIAlerts(epic: string, prevRSI: number, currentRSI: number): IndicatorAlert[] {
    const triggered: IndicatorAlert[] = [];

    this.indicatorAlerts.forEach((alert) => {
      if (alert.epic !== epic || alert.indicator !== 'RSI' || alert.status !== 'ACTIVE') return;

      let shouldTrigger = false;

      if (alert.condition === 'CROSSES_ABOVE' && prevRSI < alert.value && currentRSI >= alert.value) {
        shouldTrigger = true;
      } else if (alert.condition === 'CROSSES_BELOW' && prevRSI > alert.value && currentRSI <= alert.value) {
        shouldTrigger = true;
      } else if (alert.condition === 'ENTERS_ZONE') {
        if (alert.value === 70 && prevRSI < 70 && currentRSI >= 70) shouldTrigger = true;
        if (alert.value === 30 && prevRSI > 30 && currentRSI <= 30) shouldTrigger = true;
      }

      if (shouldTrigger) {
        alert.currentValue = currentRSI;
        alert.triggeredAt = new Date();
        alert.status = 'TRIGGERED';
        triggered.push(alert);
        
        if (this.onAlertTriggered) {
          this.onAlertTriggered(alert);
        }
      }
    });

    return triggered;
  }

  /**
   * Check MACD alerts
   */
  checkMACDAlerts(epic: string, histogram: number, crossover: 'BULLISH' | 'BEARISH' | 'NONE'): IndicatorAlert[] {
    const triggered: IndicatorAlert[] = [];

    this.indicatorAlerts.forEach((alert) => {
      if (alert.epic !== epic || alert.indicator !== 'MACD' || alert.status !== 'ACTIVE') return;

      let shouldTrigger = false;

      if (alert.condition === 'CROSSES_ABOVE' && crossover === 'BULLISH') {
        shouldTrigger = true;
      } else if (alert.condition === 'CROSSES_BELOW' && crossover === 'BEARISH') {
        shouldTrigger = true;
      }

      if (shouldTrigger) {
        alert.currentValue = histogram;
        alert.triggeredAt = new Date();
        alert.status = 'TRIGGERED';
        triggered.push(alert);
        
        if (this.onAlertTriggered) {
          this.onAlertTriggered(alert);
        }
      }
    });

    return triggered;
  }

  // ==================== TRADE ALERTS ====================

  /**
   * Create a trade alert
   */
  createTradeAlert(params: {
    type: 'ENTRY' | 'EXIT' | 'STOP_LOSS' | 'TAKE_PROFIT' | 'DAILY_LOSS' | 'MARGIN';
    epic?: string;
    message: string;
    severity: 'INFO' | 'WARNING' | 'CRITICAL';
  }): TradeAlert {
    const alert: TradeAlert = {
      id: `TA_${Date.now()}_${Math.random().toString(36).substr(2, 6)}`,
      ...params,
      timestamp: new Date(),
      acknowledged: false
    };

    this.tradeAlerts.unshift(alert);
    
    // Keep only recent alerts
    if (this.tradeAlerts.length > this.maxAlerts) {
      this.tradeAlerts = this.tradeAlerts.slice(0, this.maxAlerts);
    }

    if (this.onAlertTriggered) {
      this.onAlertTriggered(alert);
    }

    return alert;
  }

  /**
   * Acknowledge a trade alert
   */
  acknowledgeTradeAlert(id: string): boolean {
    const alert = this.tradeAlerts.find(a => a.id === id);
    if (alert) {
      alert.acknowledged = true;
      return true;
    }
    return false;
  }

  // ==================== MANAGEMENT ====================

  /**
   * Cancel an alert
   */
  cancelAlert(id: string): boolean {
    if (this.priceAlerts.has(id)) {
      const alert = this.priceAlerts.get(id)!;
      alert.status = 'CANCELLED';
      return true;
    }
    if (this.indicatorAlerts.has(id)) {
      const alert = this.indicatorAlerts.get(id)!;
      alert.status = 'CANCELLED';
      return true;
    }
    return false;
  }

  /**
   * Delete an alert
   */
  deleteAlert(id: string): boolean {
    return this.priceAlerts.delete(id) || this.indicatorAlerts.delete(id);
  }

  /**
   * Get all active price alerts
   */
  getActivePriceAlerts(): PriceAlert[] {
    return Array.from(this.priceAlerts.values()).filter(a => a.status === 'ACTIVE');
  }

  /**
   * Get all active indicator alerts
   */
  getActiveIndicatorAlerts(): IndicatorAlert[] {
    return Array.from(this.indicatorAlerts.values()).filter(a => a.status === 'ACTIVE');
  }

  /**
   * Get recent trade alerts
   */
  getRecentTradeAlerts(limit: number = 20): TradeAlert[] {
    return this.tradeAlerts.slice(0, limit);
  }

  /**
   * Get all alerts for an epic
   */
  getAlertsForEpic(epic: string): {
    price: PriceAlert[];
    indicator: IndicatorAlert[];
  } {
    return {
      price: Array.from(this.priceAlerts.values()).filter(a => a.epic === epic),
      indicator: Array.from(this.indicatorAlerts.values()).filter(a => a.epic === epic)
    };
  }

  /**
   * Get unacknowledged trade alerts count
   */
  getUnacknowledgedCount(): number {
    return this.tradeAlerts.filter(a => !a.acknowledged).length;
  }

  /**
   * Clear all triggered alerts
   */
  clearTriggeredAlerts(): number {
    let count = 0;
    
    this.priceAlerts.forEach((alert, id) => {
      if (alert.status === 'TRIGGERED') {
        this.priceAlerts.delete(id);
        count++;
      }
    });
    
    this.indicatorAlerts.forEach((alert, id) => {
      if (alert.status === 'TRIGGERED') {
        this.indicatorAlerts.delete(id);
        count++;
      }
    });

    return count;
  }

  /**
   * Get alert statistics
   */
  getStats(): {
    totalActivePriceAlerts: number;
    totalActiveIndicatorAlerts: number;
    totalTradeAlerts: number;
    unacknowledgedAlerts: number;
    triggeredToday: number;
  } {
    const today = new Date().toDateString();
    
    return {
      totalActivePriceAlerts: this.getActivePriceAlerts().length,
      totalActiveIndicatorAlerts: this.getActiveIndicatorAlerts().length,
      totalTradeAlerts: this.tradeAlerts.length,
      unacknowledgedAlerts: this.getUnacknowledgedCount(),
      triggeredToday: this.tradeAlerts.filter(a => a.timestamp.toDateString() === today).length
    };
  }
}

// Singleton instance
let alertManagerInstance: AlertManager | null = null;

export function getAlertManager(): AlertManager {
  if (!alertManagerInstance) {
    alertManagerInstance = new AlertManager();
  }
  return alertManagerInstance;
}

export function initAlertManager(): AlertManager {
  alertManagerInstance = new AlertManager();
  return alertManagerInstance;
}
