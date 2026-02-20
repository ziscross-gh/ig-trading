/**
 * Market Scanner
 * Scans multiple markets for trading opportunities based on technical analysis
 */

import { calculateSMA, calculateEMA, calculateRSI, analyzeMACD, calculateBollingerBands, calculateATR } from './technical-indicators';

export interface ScanResult {
  epic: string;
  name: string;
  timestamp: Date;
  price: number;
  change: number;
  changePercent: number;
  
  // Trend Analysis
  trend: 'STRONG_BULLISH' | 'BULLISH' | 'NEUTRAL' | 'BEARISH' | 'STRONG_BEARISH';
  trendStrength: number; // 1-10
  
  // Signal
  signal: 'STRONG_BUY' | 'BUY' | 'HOLD' | 'SELL' | 'STRONG_SELL';
  signalStrength: number; // 1-10
  confidence: number; // 0-100%
  
  // Technical Indicators
  indicators: {
    rsi: { value: number; signal: 'OVERBOUGHT' | 'OVERSOLD' | 'NEUTRAL' };
    macd: { value: number; signal: 'BULLISH' | 'BEARISH' | 'NEUTRAL' };
    ma: { short: number; long: number; signal: 'BULLISH' | 'BEARISH' | 'NEUTRAL' };
    bb: { upper: number; lower: number; position: number; signal: 'OVERBOUGHT' | 'OVERSOLD' | 'NEUTRAL' };
    atr: number;
  };
  
  // Trading Setup
  setup: {
    entry: number;
    stopLoss: number;
    takeProfit1: number;
    takeProfit2: number;
    riskReward: number;
  };
  
  // Reasons
  reasons: string[];
  warnings: string[];
  
  // Score
  score: number; // 0-100
  hotness: number; // 0-10 (how "hot" the setup is)
}

export interface ScanConfig {
  markets: string[];
  timeframes: ('1m' | '5m' | '15m' | '1h' | '4h' | '1d')[];
  minVolume?: number;
  minATR?: number;
  maxSpread?: number;
  signalFilters: {
    minSignalStrength: number;
    minConfidence: number;
    requiredSignals: ('rsi' | 'macd' | 'ma' | 'bb')[];
  };
}

const DEFAULT_CONFIG: ScanConfig = {
  markets: [
    'CS.D.GOLDUSD.CFD',
    'CS.D.EURUSD.CFD',
    'CS.D.GBPUSD.CFD',
    'CS.D.USDJPY.CFD',
    'CS.D.AUDUSD.CFD'
  ],
  timeframes: ['1h', '4h'],
  signalFilters: {
    minSignalStrength: 5,
    minConfidence: 60,
    requiredSignals: []
  }
};

// Market names mapping
const MARKET_NAMES: Record<string, string> = {
  'CS.D.GOLDUSD.CFD': 'Gold (XAU/USD)',
  'CS.D.EURUSD.CFD': 'EUR/USD',
  'CS.D.GBPUSD.CFD': 'GBP/USD',
  'CS.D.USDJPY.CFD': 'USD/JPY',
  'CS.D.AUDUSD.CFD': 'AUD/USD'
};

/**
 * Market Scanner Class
 */
export class MarketScanner {
  private config: ScanConfig;
  private lastScan: Map<string, ScanResult> = new Map();

  constructor(config: Partial<ScanConfig> = {}) {
    this.config = { ...DEFAULT_CONFIG, ...config };
  }

  /**
   * Scan all configured markets
   */
  async scanAll(): Promise<ScanResult[]> {
    const results: ScanResult[] = [];

    for (const epic of this.config.markets) {
      try {
        const result = await this.scanMarket(epic);
        if (result) {
          results.push(result);
          this.lastScan.set(epic, result);
        }
      } catch (error) {
        console.error(`Scan failed for ${epic}:`, error);
      }
    }

    // Sort by score descending
    return results.sort((a, b) => b.score - a.score);
  }

  /**
   * Scan a single market
   */
  async scanMarket(epic: string): Promise<ScanResult | null> {
    // Generate mock candle data (in production, fetch from IG API)
    const candles = this.generateMockCandles(epic, 200);
    const currentPrice = candles[candles.length - 1].close;
    const prevPrice = candles[candles.length - 2].close;
    const change = currentPrice - prevPrice;
    const changePercent = (change / prevPrice) * 100;

    // Calculate indicators
    const closes = candles.map(c => c.close);
    const rsiValue = calculateRSI(closes, 14);
    const currentRSI = rsiValue[rsiValue.length - 1];
    
    const macdResult = analyzeMACD(closes, 12, 26, 9);
    
    const shortMA = calculateEMA(closes, 9);
    const longMA = calculateEMA(closes, 21);
    
    const bb = calculateBollingerBands(closes, 20, 2);
    const atr = calculateATR(candles.map(c => ({ high: c.high, low: c.low, close: c.close })), 14);

    // Analyze signals
    const signals: { bullish: number; bearish: number } = { bullish: 0, bearish: 0 };
    const reasons: string[] = [];
    const warnings: string[] = [];

    // RSI Analysis
    let rsiSignal: 'OVERBOUGHT' | 'OVERSOLD' | 'NEUTRAL' = 'NEUTRAL';
    if (currentRSI <= 30) {
      rsiSignal = 'OVERSOLD';
      signals.bullish += 2;
      reasons.push(`RSI oversold at ${currentRSI.toFixed(1)}`);
    } else if (currentRSI >= 70) {
      rsiSignal = 'OVERBOUGHT';
      signals.bearish += 2;
      reasons.push(`RSI overbought at ${currentRSI.toFixed(1)}`);
    }

    // MACD Analysis
    let macdSignal: 'BULLISH' | 'BEARISH' | 'NEUTRAL' = 'NEUTRAL';
    if (macdResult.crossover === 'BULLISH') {
      macdSignal = 'BULLISH';
      signals.bullish += 3;
      reasons.push('MACD bullish crossover');
    } else if (macdResult.crossover === 'BEARISH') {
      macdSignal = 'BEARISH';
      signals.bearish += 3;
      reasons.push('MACD bearish crossover');
    } else if (macdResult.histogram > 0) {
      macdSignal = 'BULLISH';
      signals.bullish += 1;
    } else if (macdResult.histogram < 0) {
      macdSignal = 'BEARISH';
      signals.bearish += 1;
    }

    // MA Analysis
    const currentShortMA = shortMA[shortMA.length - 1];
    const currentLongMA = longMA[longMA.length - 1];
    let maSignal: 'BULLISH' | 'BEARISH' | 'NEUTRAL' = 'NEUTRAL';
    
    if (currentShortMA > currentLongMA) {
      maSignal = 'BULLISH';
      signals.bullish += 2;
      if (shortMA[shortMA.length - 2] <= longMA[longMA.length - 2]) {
        signals.bullish += 2;
        reasons.push('MA bullish crossover');
      }
    } else if (currentShortMA < currentLongMA) {
      maSignal = 'BEARISH';
      signals.bearish += 2;
      if (shortMA[shortMA.length - 2] >= longMA[longMA.length - 2]) {
        signals.bearish += 2;
        reasons.push('MA bearish crossover');
      }
    }

    // Bollinger Bands Analysis
    const currentBB = { upper: bb.upper[bb.upper.length - 1], lower: bb.lower[bb.lower.length - 1] };
    const bbPosition = (currentPrice - currentBB.lower) / (currentBB.upper - currentBB.lower);
    let bbSignal: 'OVERBOUGHT' | 'OVERSOLD' | 'NEUTRAL' = 'NEUTRAL';
    
    if (bbPosition >= 0.95) {
      bbSignal = 'OVERBOUGHT';
      signals.bearish += 2;
      warnings.push('Price at upper Bollinger Band');
    } else if (bbPosition <= 0.05) {
      bbSignal = 'OVERSOLD';
      signals.bullish += 2;
      warnings.push('Price at lower Bollinger Band');
    }

    // Determine overall trend
    const trend: ScanResult['trend'] = this.determineTrend(closes, shortMA, longMA);
    const trendStrength = Math.min(10, Math.abs(signals.bullish - signals.bearish) + 3);

    // Determine signal
    const signalDiff = signals.bullish - signals.bearish;
    let signal: ScanResult['signal'] = 'HOLD';
    let signalStrength = 5;

    if (signalDiff >= 5) {
      signal = 'STRONG_BUY';
      signalStrength = Math.min(10, 7 + signalDiff);
    } else if (signalDiff >= 2) {
      signal = 'BUY';
      signalStrength = 6;
    } else if (signalDiff <= -5) {
      signal = 'STRONG_SELL';
      signalStrength = Math.min(10, 7 + Math.abs(signalDiff));
    } else if (signalDiff <= -2) {
      signal = 'SELL';
      signalStrength = 6;
    }

    // Calculate confidence
    const totalSignals = signals.bullish + signals.bearish;
    const confidence = totalSignals > 0 ? Math.min(100, (Math.abs(signalDiff) / totalSignals) * 100 + 30) : 50;

    // Calculate setup
    const atrValue = atr[atr.length - 1];
    const setup = this.calculateSetup(currentPrice, signal, atrValue);

    // Calculate score (0-100)
    const score = this.calculateScore(signal, signalStrength, confidence, trend, trendStrength);
    
    // Calculate hotness (0-10)
    const hotness = this.calculateHotness(changePercent, signalStrength, confidence);

    return {
      epic,
      name: MARKET_NAMES[epic] || epic,
      timestamp: new Date(),
      price: currentPrice,
      change,
      changePercent,
      trend,
      trendStrength,
      signal,
      signalStrength,
      confidence,
      indicators: {
        rsi: { value: currentRSI, signal: rsiSignal },
        macd: { value: macdResult.histogram, signal: macdSignal },
        ma: { short: currentShortMA, long: currentLongMA, signal: maSignal },
        bb: { upper: currentBB.upper, lower: currentBB.lower, position: bbPosition, signal: bbSignal },
        atr: atrValue
      },
      setup,
      reasons,
      warnings,
      score,
      hotness
    };
  }

  /**
   * Determine trend from price action
   */
  private determineTrend(
    closes: number[],
    shortMA: number[],
    longMA: number[]
  ): ScanResult['trend'] {
    const currentPrice = closes[closes.length - 1];
    const currentShortMA = shortMA[shortMA.length - 1];
    const currentLongMA = longMA[longMA.length - 1];
    
    const priceVsShortMA = (currentPrice - currentShortMA) / currentShortMA * 100;
    const priceVsLongMA = (currentPrice - currentLongMA) / currentLongMA * 100;
    
    const maDiff = (currentShortMA - currentLongMA) / currentLongMA * 100;

    if (maDiff > 0.5 && priceVsShortMA > 0.2) {
      return 'STRONG_BULLISH';
    } else if (maDiff > 0.1 && priceVsShortMA > 0) {
      return 'BULLISH';
    } else if (maDiff < -0.5 && priceVsShortMA < -0.2) {
      return 'STRONG_BEARISH';
    } else if (maDiff < -0.1 && priceVsShortMA < 0) {
      return 'BEARISH';
    }
    
    return 'NEUTRAL';
  }

  /**
   * Calculate entry, stop loss, and take profit levels
   */
  private calculateSetup(
    price: number,
    signal: ScanResult['signal'],
    atr: number
  ): ScanResult['setup'] {
    const isBuy = signal.includes('BUY');
    const atrMultiplier = 2;
    
    const entry = price;
    const stopLoss = isBuy 
      ? price - (atr * atrMultiplier)
      : price + (atr * atrMultiplier);
    
    const takeProfit1 = isBuy
      ? price + (atr * atrMultiplier * 1.5)
      : price - (atr * atrMultiplier * 1.5);
    
    const takeProfit2 = isBuy
      ? price + (atr * atrMultiplier * 2.5)
      : price - (atr * atrMultiplier * 2.5);

    const risk = Math.abs(entry - stopLoss);
    const reward = Math.abs(takeProfit1 - entry);
    const riskReward = reward / risk;

    return {
      entry,
      stopLoss,
      takeProfit1,
      takeProfit2,
      riskReward
    };
  }

  /**
   * Calculate overall score
   */
  private calculateScore(
    signal: ScanResult['signal'],
    signalStrength: number,
    confidence: number,
    trend: ScanResult['trend'],
    trendStrength: number
  ): number {
    let score = 50;

    // Signal contribution
    if (signal === 'STRONG_BUY' || signal === 'STRONG_SELL') {
      score += 20;
    } else if (signal === 'BUY' || signal === 'SELL') {
      score += 10;
    }

    // Signal strength
    score += signalStrength * 2;

    // Confidence
    score = score * (confidence / 100);

    // Trend alignment
    if ((signal.includes('BUY') && trend.includes('BULLISH')) ||
        (signal.includes('SELL') && trend.includes('BEARISH'))) {
      score += 10;
    }

    return Math.min(100, Math.max(0, Math.round(score)));
  }

  /**
   * Calculate hotness score
   */
  private calculateHotness(
    changePercent: number,
    signalStrength: number,
    confidence: number
  ): number {
    const volatility = Math.abs(changePercent) * 5;
    const strength = signalStrength * 0.5;
    const conf = confidence / 20;
    
    return Math.min(10, Math.round(volatility + strength + conf));
  }

  /**
   * Generate mock candle data
   */
  private generateMockCandles(epic: string, count: number): Array<{
    open: number;
    high: number;
    low: number;
    close: number;
    volume: number;
  }> {
    const basePrice = epic.includes('GOLD') ? 2000 :
                      epic.includes('EURUSD') ? 1.08 :
                      epic.includes('GBPUSD') ? 1.26 :
                      epic.includes('USDJPY') ? 149 : 0.65;
    
    const candles = [];
    let price = basePrice;

    for (let i = 0; i < count; i++) {
      const change = (Math.random() - 0.5) * 0.02 * price;
      const open = price;
      const close = price + change;
      const high = Math.max(open, close) + Math.random() * 0.005 * price;
      const low = Math.min(open, close) - Math.random() * 0.005 * price;
      
      candles.push({
        open,
        high,
        low,
        close,
        volume: Math.floor(Math.random() * 10000) + 1000
      });
      
      price = close;
    }

    return candles;
  }

  /**
   * Get last scan results
   */
  getLastScan(): Map<string, ScanResult> {
    return this.lastScan;
  }

  /**
   * Get top opportunities
   */
  getTopOpportunities(limit: number = 5): ScanResult[] {
    return Array.from(this.lastScan.values())
      .filter(r => r.signal !== 'HOLD')
      .sort((a, b) => b.score - a.score)
      .slice(0, limit);
  }

  /**
   * Filter results by criteria
   */
  filterResults(
    results: ScanResult[],
    filter: {
      signal?: ('STRONG_BUY' | 'BUY' | 'SELL' | 'STRONG_SELL')[];
      minScore?: number;
      minConfidence?: number;
      trend?: ('BULLISH' | 'BEARISH')[];
    }
  ): ScanResult[] {
    return results.filter(r => {
      if (filter.signal && !filter.signal.includes(r.signal as any)) return false;
      if (filter.minScore && r.score < filter.minScore) return false;
      if (filter.minConfidence && r.confidence < filter.minConfidence) return false;
      if (filter.trend && !filter.trend.some(t => r.trend.includes(t))) return false;
      return true;
    });
  }
}

// Singleton instance
let scannerInstance: MarketScanner | null = null;

export function getMarketScanner(config?: Partial<ScanConfig>): MarketScanner {
  if (!scannerInstance) {
    scannerInstance = new MarketScanner(config);
  }
  return scannerInstance;
}
