// Trading Strategies Library

import type { Candle, StrategyConfig } from '@/types/ig';
import { calculateSMA, calculateEMA, calculateRSI, calculateATR, calculateBollingerBands, findSupportResistance, analyzeMACD } from './technical-indicators';

export interface StrategyResult {
  signal: 'BUY' | 'SELL' | 'NONE';
  strength: number;
  reason: string;
  indicators: Record<string, number | string>;
}

// MA Crossover Strategy
export function maCrossoverStrategy(candles: Candle[], config: StrategyConfig): StrategyResult {
  const shortPeriod = Number(config.parameters.shortPeriod) || 9;
  const longPeriod = Number(config.parameters.longPeriod) || 21;
  const useEMA = config.parameters.useEMA !== false;
  if (candles.length < longPeriod + 5) return { signal: 'NONE', strength: 0, reason: 'Insufficient data', indicators: {} };
  const closes = candles.map((c) => c.close);
  const shortMA = useEMA ? calculateEMA(closes, shortPeriod) : calculateSMA(closes, shortPeriod);
  const longMA = useEMA ? calculateEMA(closes, longPeriod) : calculateSMA(closes, longPeriod);
  const offset = longPeriod - shortPeriod;
  const currentShortMA = shortMA[shortMA.length - 1 - offset];
  const currentLongMA = longMA[longMA.length - 1];
  const prevShortMA = shortMA[shortMA.length - 2 - offset];
  const prevLongMA = longMA[longMA.length - 2];
  const indicators: Record<string, number | string> = { shortMA: currentShortMA.toFixed(4), longMA: currentLongMA.toFixed(4), crossover: 'NONE' };
  if (prevShortMA <= prevLongMA && currentShortMA > currentLongMA) {
    return { signal: 'BUY', strength: 8, reason: `Bullish crossover: EMA(${shortPeriod}) crossed above EMA(${longPeriod})`, indicators };
  } else if (prevShortMA >= prevLongMA && currentShortMA < currentLongMA) {
    return { signal: 'SELL', strength: 8, reason: `Bearish crossover: EMA(${shortPeriod}) crossed below EMA(${longPeriod})`, indicators };
  }
  const trendStrength = Math.abs(currentShortMA - currentLongMA) / currentLongMA * 1000;
  if (currentShortMA > currentLongMA && trendStrength > 0.5) {
    return { signal: 'BUY', strength: Math.min(6, Math.floor(trendStrength)), reason: `Bullish trend: EMA(${shortPeriod}) above EMA(${longPeriod})`, indicators };
  } else if (currentShortMA < currentLongMA && trendStrength > 0.5) {
    return { signal: 'SELL', strength: Math.min(6, Math.floor(trendStrength)), reason: `Bearish trend: EMA(${shortPeriod}) below EMA(${longPeriod})`, indicators };
  }
  return { signal: 'NONE', strength: 0, reason: 'No significant MA signal', indicators };
}

// RSI Strategy
export function rsiStrategy(candles: Candle[], config: StrategyConfig): StrategyResult {
  const period = Number(config.parameters.period) || 14;
  const overboughtLevel = Number(config.parameters.overboughtLevel) || 70;
  const oversoldLevel = Number(config.parameters.oversoldLevel) || 30;
  if (candles.length < period + 5) return { signal: 'NONE', strength: 0, reason: 'Insufficient data', indicators: {} };
  const closes = candles.map((c) => c.close);
  const rsi = calculateRSI(closes, period);
  const currentRSI = rsi[rsi.length - 1];
  const indicators: Record<string, number | string> = { rsi: currentRSI.toFixed(2), signal: 'NEUTRAL' };
  if (currentRSI <= oversoldLevel) {
    const strength = Math.min(10, Math.floor((oversoldLevel - currentRSI) / 5) + 5);
    return { signal: 'BUY', strength, reason: `RSI oversold at ${currentRSI.toFixed(2)} (below ${oversoldLevel})`, indicators };
  } else if (currentRSI >= overboughtLevel) {
    const strength = Math.min(10, Math.floor((currentRSI - overboughtLevel) / 5) + 5);
    return { signal: 'SELL', strength, reason: `RSI overbought at ${currentRSI.toFixed(2)} (above ${overboughtLevel})`, indicators };
  }
  return { signal: 'NONE', strength: 0, reason: `RSI neutral at ${currentRSI.toFixed(2)}`, indicators };
}

// MACD Strategy
export function macdStrategy(candles: Candle[], config: StrategyConfig): StrategyResult {
  const fastPeriod = Number(config.parameters.fastPeriod) || 12;
  const slowPeriod = Number(config.parameters.slowPeriod) || 26;
  const signalPeriod = Number(config.parameters.signalPeriod) || 9;
  if (candles.length < slowPeriod + signalPeriod + 5) return { signal: 'NONE', strength: 0, reason: 'Insufficient data', indicators: {} };
  const closes = candles.map((c) => c.close);
  const result = analyzeMACD(closes, fastPeriod, slowPeriod, signalPeriod);
  const indicators: Record<string, number | string> = { macd: result.macd.toFixed(6), signal: result.signal.toFixed(6), histogram: result.histogram.toFixed(6), crossover: result.crossover };
  if (result.crossover === 'BULLISH') return { signal: 'BUY', strength: 7, reason: 'MACD bullish crossover: MACD crossed above signal line', indicators };
  if (result.crossover === 'BEARISH') return { signal: 'SELL', strength: 7, reason: 'MACD bearish crossover: MACD crossed below signal line', indicators };
  const histogramStrength = Math.abs(result.histogram);
  if (result.histogram > 0 && histogramStrength > 0.0001) return { signal: 'BUY', strength: Math.min(5, Math.floor(histogramStrength * 10000)), reason: 'MACD bullish momentum', indicators };
  if (result.histogram < 0 && histogramStrength > 0.0001) return { signal: 'SELL', strength: Math.min(5, Math.floor(histogramStrength * 10000)), reason: 'MACD bearish momentum', indicators };
  return { signal: 'NONE', strength: 0, reason: 'No significant MACD signal', indicators };
}

// Bollinger Bands Strategy
export function bollingerBandsStrategy(candles: Candle[], config: StrategyConfig): StrategyResult {
  const period = Number(config.parameters.period) || 20;
  const stdDev = Number(config.parameters.stdDev) || 2;
  if (candles.length < period + 5) return { signal: 'NONE', strength: 0, reason: 'Insufficient data', indicators: {} };
  const closes = candles.map((c) => c.close);
  const { upper, middle, lower } = calculateBollingerBands(closes, period, stdDev);
  const currentPrice = closes[closes.length - 1];
  const currentUpper = upper[upper.length - 1];
  const currentLower = lower[lower.length - 1];
  const indicators: Record<string, number | string> = { upper: currentUpper.toFixed(4), lower: currentLower.toFixed(4), currentPrice: currentPrice.toFixed(4) };
  if (currentPrice <= currentLower * 1.01) return { signal: 'BUY', strength: 6, reason: `Price near lower Bollinger Band at ${currentPrice.toFixed(4)}`, indicators };
  if (currentPrice >= currentUpper * 0.99) return { signal: 'SELL', strength: 6, reason: `Price near upper Bollinger Band at ${currentPrice.toFixed(4)}`, indicators };
  return { signal: 'NONE', strength: 0, reason: 'Price within Bollinger Bands', indicators };
}

// Strategy Metadata
export const STRATEGY_METADATA = {
  MA_CROSSOVER: { displayName: 'Moving Average Crossover', description: 'EMA 9/21 crossover strategy', category: 'Trend Following', parameters: [{ name: 'shortPeriod', type: 'number', default: 9, label: 'Short Period' }, { name: 'longPeriod', type: 'number', default: 21, label: 'Long Period' }, { name: 'useEMA', type: 'boolean', default: true, label: 'Use EMA' }] },
  RSI_OVERBOUGHT_OVERSOLD: { displayName: 'RSI Overbought/Oversold', description: 'Trade based on RSI extremes', category: 'Mean Reversion', parameters: [{ name: 'period', type: 'number', default: 14, label: 'RSI Period' }, { name: 'overboughtLevel', type: 'number', default: 70, label: 'Overbought Level' }, { name: 'oversoldLevel', type: 'number', default: 30, label: 'Oversold Level' }] },
  MACD_SIGNAL: { displayName: 'MACD Signal Cross', description: 'MACD signal line crossover', category: 'Trend Following', parameters: [{ name: 'fastPeriod', type: 'number', default: 12, label: 'Fast Period' }, { name: 'slowPeriod', type: 'number', default: 26, label: 'Slow Period' }, { name: 'signalPeriod', type: 'number', default: 9, label: 'Signal Period' }] },
  BOLLINGER_BREAKOUT: { displayName: 'Bollinger Bands Breakout', description: 'Trade bounces and breakouts', category: 'Mean Reversion', parameters: [{ name: 'period', type: 'number', default: 20, label: 'Period' }, { name: 'stdDev', type: 'number', default: 2, label: 'Standard Deviation' }] },
};

// Analyze all strategies
export function analyzeStrategies(candles: Candle[], strategies: StrategyConfig[]): any {
  let buySignals = 0;
  let sellSignals = 0;
  const reasons: string[] = [];
  for (const strategy of strategies) {
    if (!strategy.enabled) continue;
    let result: StrategyResult = { signal: 'NONE', strength: 0, reason: '', indicators: {} };
    switch (strategy.name) {
      case 'MA_CROSSOVER': result = maCrossoverStrategy(candles, strategy); break;
      case 'RSI_OVERBOUGHT_OVERSOLD': result = rsiStrategy(candles, strategy); break;
      case 'MACD_SIGNAL': result = macdStrategy(candles, strategy); break;
      case 'BOLLINGER_BREAKOUT': result = bollingerBandsStrategy(candles, strategy); break;
    }
    if (result.signal === 'BUY') { buySignals++; reasons.push(result.reason); }
    else if (result.signal === 'SELL') { sellSignals++; reasons.push(result.reason); }
  }
  if (buySignals > sellSignals && buySignals >= 2) {
    const currentPrice = candles[candles.length - 1].close;
    return { id: `signal-${Date.now()}`, epic: '', direction: 'BUY', strength: Math.min(10, buySignals * 2), strategy: 'COMBINED', timestamp: new Date(), entry: currentPrice, stopLoss: currentPrice * 0.985, takeProfit: currentPrice * 1.03, reason: reasons.join(' | ') };
  } else if (sellSignals > buySignals && sellSignals >= 2) {
    const currentPrice = candles[candles.length - 1].close;
    return { id: `signal-${Date.now()}`, epic: '', direction: 'SELL', strength: Math.min(10, sellSignals * 2), strategy: 'COMBINED', timestamp: new Date(), entry: currentPrice, stopLoss: currentPrice * 1.015, takeProfit: currentPrice * 0.97, reason: reasons.join(' | ') };
  }
  return null;
}
