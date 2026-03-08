// IG API Types and Interfaces

// ============================================
// CORE TYPES
// ============================================

export interface IGCredentials {
  apiKey: string;
  username: string;
  password: string;
  isDemo: boolean;
}

export interface IGSession {
  cst: string;
  xSecurityToken: string;
  accountId: string;
  lightstreamerEndpoint: string;
  isDemo: boolean;
  expiresIn: number;
  createdAt: Date;
}

// ============================================
// MARKET DATA TYPES
// ============================================

export interface Market {
  epic: string;
  name: string;
  bid: number;
  offer: number;
  high: number;
  low: number;
  change: number;
  changePercent: number;
  delayTime: number;
  expiry: string;
  InstrumentType: string;
  marketStatus: 'TRADEABLE' | 'CLOSED' | 'EDIT' | 'ON_AUCTION' | 'AUCTION_OVER';
}

export interface Candle {
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number;
  timestamp: string;
}

// ============================================
// TRADING TYPES
// ============================================

export interface Position {
  dealId: string;
  dealReference: string;
  epic: string;
  direction: 'BUY' | 'SELL';
  size: number;
  level: number;
  stopLevel?: number;
  limitLevel?: number;
  pnl: number;
  currency: string;
  createdDate: string;
  marketName?: string;
}

export interface Order {
  dealId: string;
  dealReference: string;
  epic: string;
  direction: 'BUY' | 'SELL';
  size: number;
  level: number;
  stopLevel?: number;
  limitLevel?: number;
  orderType: 'LIMIT' | 'STOP';
  currency: string;
  createdDate: string;
  goodTillDate?: string;
}

export interface TradeRequest {
  epic: string;
  direction: 'BUY' | 'SELL';
  size: number;
  orderType: 'MARKET' | 'LIMIT' | 'STOP';
  level?: number;
  stopLevel?: number;
  limitLevel?: number;
  guaranteedStop?: boolean;
}

export interface TradeResponse {
  dealReference: string;
  dealId?: string;
  status: 'SUCCESS' | 'REJECTED';
  reason?: string;
}

// ============================================
// STRATEGY TYPES
// ============================================

export interface StrategyConfig {
  name: string;
  enabled: boolean;
  parameters: Record<string, number | boolean | string>;
}

export interface TradeSignal {
  id: string;
  epic: string;
  direction: 'BUY' | 'SELL';
  strength: number;
  strategy: string;
  timestamp: Date;
  entry: number;
  stopLoss?: number;
  takeProfit?: number;
  reason?: string;
}

// ============================================
// RISK MANAGEMENT TYPES
// ============================================

export interface RiskConfig {
  maxPositionSize: number;
  maxDailyTrades: number;
  maxDailyLoss: number;
  defaultStopLossPercent: number;
  defaultTakeProfitPercent: number;
  riskPerTrade: number;
  maxDrawdown: number;
}

export interface BotConfig {
  selectedMarkets: string[];
  strategies: StrategyConfig[];
  riskConfig: RiskConfig;
  tradingHours: {
    start: string;
    end: string;
  };
  autoStart: boolean;
}

export interface AccountDetails {
  accountId: string;
  accountName: string;
  accountType: string;
  balance: number;
  deposit: number;
  profitLoss: number;
  available: number;
  currency: string;
}

// ============================================
// ACTIVITY TYPES
// ============================================

export interface ActivityLog {
  id: string;
  timestamp: Date;
  type: 'INFO' | 'TRADE' | 'SUCCESS' | 'WARNING' | 'ERROR';
  message: string;
  details?: Record<string, unknown>;
}

export interface PerformanceMetrics {
  totalTrades: number;
  winningTrades: number;
  losingTrades: number;
  winRate: number;
  totalPnl: number;
  avgWin: number;
  avgLoss: number;
  profitFactor: number;
  sharpeRatio: number;
  maxDrawdown: number;
  dailyPnl: Array<{ date: string; pnl: number }>;
}

// ============================================
// DEFAULT VALUES
// ============================================

export const DEFAULT_MARKETS = {
  GOLD: 'CS.D.CFIGOLD.CFI.IP',
  EUR_USD: 'CS.D.EURUSD.CSD.IP',
  GBP_USD: 'CS.D.GBPUSD.CSD.IP',
  USD_JPY: 'CS.D.USDJPY.CSD.IP',
  AUD_USD: 'CS.D.AUDUSD.CSD.IP',
} as const;

export const MARKET_NAMES: Record<string, string> = {
  // Gold variants
  'CS.D.CFIGOLD.CFI.IP': 'Spot Gold (SGD1)',
  'CS.D.CFDGOLD.CMG.IP': 'Spot Gold ($1)',
  'CS.D.GOLDUSD.CFD': 'Gold (XAU/USD)',
  'CS.D.XAUUSD.CFD': 'Gold (XAU/USD)',
  // Forex — demo (*.CSD.IP) and live (*.CFD) variants
  'CS.D.EURUSD.CSD.IP': 'EUR/USD',
  'CS.D.EURUSD.CFD': 'EUR/USD',
  'CS.D.GBPUSD.CSD.IP': 'GBP/USD',
  'CS.D.GBPUSD.CFD': 'GBP/USD',
  'CS.D.USDJPY.CSD.IP': 'USD/JPY',
  'CS.D.USDJPY.CFD': 'USD/JPY',
  'CS.D.AUDUSD.CSD.IP': 'AUD/USD',
  'CS.D.AUDUSD.CFD': 'AUD/USD',
};

export const DEFAULT_RISK_CONFIG: RiskConfig = {
  maxPositionSize: 1,
  maxDailyTrades: 10,
  maxDailyLoss: 500,
  defaultStopLossPercent: 1.5,
  defaultTakeProfitPercent: 3,
  riskPerTrade: 1,
  maxDrawdown: 10,
};

export const DEFAULT_STRATEGIES: StrategyConfig[] = [
  { name: 'MA_CROSSOVER', enabled: true, parameters: { shortPeriod: 9, longPeriod: 21, useEMA: true } },
  { name: 'RSI_OVERBOUGHT_OVERSOLD', enabled: true, parameters: { period: 14, overboughtLevel: 70, oversoldLevel: 30 } },
  { name: 'MACD_SIGNAL', enabled: true, parameters: { fastPeriod: 12, slowPeriod: 26, signalPeriod: 9 } },
  { name: 'BOLLINGER_BREAKOUT', enabled: true, parameters: { period: 20, stdDev: 2 } },
];

export const DEFAULT_BOT_CONFIG: BotConfig = {
  selectedMarkets: Object.values(DEFAULT_MARKETS),
  strategies: DEFAULT_STRATEGIES,
  riskConfig: DEFAULT_RISK_CONFIG,
  tradingHours: { start: '08:00', end: '17:00' },
  autoStart: false,
};

// ============================================
// INDICATOR RESULT TYPES
// ============================================

export interface MovingAverageResult {
  shortMA: number;
  longMA: number;
  crossover: 'BULLISH' | 'BEARISH' | 'NONE';
}

export interface RSIResult {
  value: number;
  signal: 'OVERBOUGHT' | 'OVERSOLD' | 'NEUTRAL';
}

export interface MACDResult {
  macd: number;
  signal: number;
  histogram: number;
  crossover: 'BULLISH' | 'BEARISH' | 'NONE';
}

export interface BollingerBandsResult {
  upper: number;
  middle: number;
  lower: number;
  bandwidth: number;
  signal: 'OVERBOUGHT' | 'OVERSOLD' | 'NEUTRAL';
}
