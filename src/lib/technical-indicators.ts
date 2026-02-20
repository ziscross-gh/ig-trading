// Technical Indicators Library

import type { Candle, MovingAverageResult, RSIResult, MACDResult, BollingerBandsResult } from '@/types/ig';

// Simple Moving Average (SMA)
export function calculateSMA(prices: number[], period: number): number[] {
  const sma: number[] = [];
  for (let i = period - 1; i < prices.length; i++) {
    const sum = prices.slice(i - period + 1, i + 1).reduce((a, b) => a + b, 0);
    sma.push(sum / period);
  }
  return sma;
}

// Exponential Moving Average (EMA)
export function calculateEMA(prices: number[], period: number): number[] {
  const ema: number[] = [];
  const multiplier = 2 / (period + 1);
  const firstSMA = prices.slice(0, period).reduce((a, b) => a + b, 0) / period;
  ema.push(firstSMA);
  for (let i = period; i < prices.length; i++) {
    const currentEMA = (prices[i] - ema[ema.length - 1]) * multiplier + ema[ema.length - 1];
    ema.push(currentEMA);
  }
  return ema;
}

// Relative Strength Index (RSI)
export function calculateRSI(prices: number[], period: number = 14): number[] {
  const rsi: number[] = [];
  const gains: number[] = [];
  const losses: number[] = [];
  for (let i = 1; i < prices.length; i++) {
    const change = prices[i] - prices[i - 1];
    gains.push(change > 0 ? change : 0);
    losses.push(change < 0 ? Math.abs(change) : 0);
  }
  let avgGain = gains.slice(0, period).reduce((a, b) => a + b, 0) / period;
  let avgLoss = losses.slice(0, period).reduce((a, b) => a + b, 0) / period;
  if (avgLoss === 0) { rsi.push(100); } else { rsi.push(100 - 100 / (1 + avgGain / avgLoss)); }
  for (let i = period; i < gains.length; i++) {
    avgGain = ((avgGain * (period - 1)) + gains[i]) / period;
    avgLoss = ((avgLoss * (period - 1)) + losses[i]) / period;
    if (avgLoss === 0) { rsi.push(100); } else { rsi.push(100 - 100 / (1 + avgGain / avgLoss)); }
  }
  return rsi;
}

// MACD
export function calculateMACD(prices: number[], fastPeriod: number = 12, slowPeriod: number = 26, signalPeriod: number = 9): { macdLine: number[]; signalLine: number[]; histogram: number[] } {
  const fastEMA = calculateEMA(prices, fastPeriod);
  const slowEMA = calculateEMA(prices, slowPeriod);
  const macdLine: number[] = [];
  const offset = slowPeriod - fastPeriod;
  for (let i = 0; i < slowEMA.length; i++) { macdLine.push(fastEMA[i + offset] - slowEMA[i]); }
  const signalLine = calculateEMA(macdLine, signalPeriod);
  const histogram: number[] = [];
  const signalOffset = macdLine.length - signalLine.length;
  for (let i = 0; i < signalLine.length; i++) { histogram.push(macdLine[i + signalOffset] - signalLine[i]); }
  return { macdLine, signalLine, histogram };
}

// Bollinger Bands
export function calculateBollingerBands(prices: number[], period: number = 20, stdDev: number = 2): { upper: number[]; middle: number[]; lower: number[] } {
  const middle = calculateSMA(prices, period);
  const upper: number[] = [];
  const lower: number[] = [];
  for (let i = 0; i < middle.length; i++) {
    const priceSlice = prices.slice(i, i + period);
    const variance = priceSlice.reduce((sum, price) => sum + Math.pow(price - middle[i], 2), 0) / period;
    const std = Math.sqrt(variance);
    upper.push(middle[i] + stdDev * std);
    lower.push(middle[i] - stdDev * std);
  }
  return { upper, middle, lower };
}

// ATR
export function calculateATR(candles: Candle[], period: number = 14): number[] {
  const trueRanges: number[] = [];
  for (let i = 0; i < candles.length; i++) {
    const high = candles[i].high;
    const low = candles[i].low;
    const prevClose = i > 0 ? candles[i - 1].close : candles[i].open;
    const tr = Math.max(high - low, Math.abs(high - prevClose), Math.abs(low - prevClose));
    trueRanges.push(tr);
  }
  const atr: number[] = [];
  let firstATR = trueRanges.slice(0, period).reduce((a, b) => a + b, 0) / period;
  atr.push(firstATR);
  for (let i = period; i < trueRanges.length; i++) {
    firstATR = ((firstATR * (period - 1)) + trueRanges[i]) / period;
    atr.push(firstATR);
  }
  return atr;
}

// Support/Resistance
export function findSupportResistance(candles: Candle[], lookback: number = 20): { support: number[]; resistance: number[] } {
  const support: number[] = [];
  const resistance: number[] = [];
  for (let i = 2; i < candles.length - 2 && i < lookback + 2; i++) {
    const candle = candles[candles.length - 1 - i];
    if (candle.low < candles[candles.length - i].low && candle.low < candles[candles.length - i - 2].low) { support.push(candle.low); }
    if (candle.high > candles[candles.length - i].high && candle.high > candles[candles.length - i - 2].high) { resistance.push(candle.high); }
  }
  support.sort((a, b) => b - a);
  resistance.sort((a, b) => a - b);
  return { support: [...new Set(support)].slice(0, 5), resistance: [...new Set(resistance)].slice(0, 5) };
}

// MA Crossover Analysis
export function analyzeMACrossover(prices: number[], shortPeriod: number, longPeriod: number, useEMA: boolean = true): MovingAverageResult {
  if (prices.length < longPeriod + 1) return { shortMA: 0, longMA: 0, crossover: 'NONE' };
  const shortMA = useEMA ? calculateEMA(prices, shortPeriod) : calculateSMA(prices, shortPeriod);
  const longMA = useEMA ? calculateEMA(prices, longPeriod) : calculateSMA(prices, longPeriod);
  const offset = longPeriod - shortPeriod;
  const currentShortMA = shortMA[shortMA.length - 1 - offset];
  const currentLongMA = longMA[longMA.length - 1];
  const prevShortMA = shortMA[shortMA.length - 2 - offset];
  const prevLongMA = longMA[longMA.length - 2];
  if (prevShortMA <= prevLongMA && currentShortMA > currentLongMA) return { shortMA: currentShortMA, longMA: currentLongMA, crossover: 'BULLISH' };
  if (prevShortMA >= prevLongMA && currentShortMA < currentLongMA) return { shortMA: currentShortMA, longMA: currentLongMA, crossover: 'BEARISH' };
  return { shortMA: currentShortMA, longMA: currentLongMA, crossover: 'NONE' };
}

// RSI Analysis
export function analyzeRSI(prices: number[], period: number = 14, overbought: number = 70, oversold: number = 30): RSIResult {
  const rsi = calculateRSI(prices, period);
  if (rsi.length === 0) return { value: 50, signal: 'NEUTRAL' };
  const currentRSI = rsi[rsi.length - 1];
  if (currentRSI >= overbought) return { value: currentRSI, signal: 'OVERBOUGHT' };
  if (currentRSI <= oversold) return { value: currentRSI, signal: 'OVERSOLD' };
  return { value: currentRSI, signal: 'NEUTRAL' };
}

// MACD Analysis
export function analyzeMACD(prices: number[], fastPeriod: number = 12, slowPeriod: number = 26, signalPeriod: number = 9): MACDResult {
  const { macdLine, signalLine, histogram } = calculateMACD(prices, fastPeriod, slowPeriod, signalPeriod);
  if (histogram.length < 2) return { macd: 0, signal: 0, histogram: 0, crossover: 'NONE' };
  const currentMACD = macdLine[macdLine.length - 1];
  const currentSignal = signalLine[signalLine.length - 1];
  const currentHistogram = histogram[histogram.length - 1];
  const prevHistogram = histogram[histogram.length - 2];
  if (prevHistogram <= 0 && currentHistogram > 0) return { macd: currentMACD, signal: currentSignal, histogram: currentHistogram, crossover: 'BULLISH' };
  if (prevHistogram >= 0 && currentHistogram < 0) return { macd: currentMACD, signal: currentSignal, histogram: currentHistogram, crossover: 'BEARISH' };
  return { macd: currentMACD, signal: currentSignal, histogram: currentHistogram, crossover: 'NONE' };
}

// Bollinger Bands Analysis
export function analyzeBollingerBands(prices: number[], period: number = 20, stdDev: number = 2): BollingerBandsResult {
  const { upper, middle, lower } = calculateBollingerBands(prices, period, stdDev);
  if (upper.length === 0) return { upper: 0, middle: 0, lower: 0, bandwidth: 0, signal: 'NEUTRAL' };
  const currentPrice = prices[prices.length - 1];
  const currentUpper = upper[upper.length - 1];
  const currentMiddle = middle[middle.length - 1];
  const currentLower = lower[lower.length - 1];
  const bandwidth = ((currentUpper - currentLower) / currentMiddle) * 100;
  const upperThreshold = currentUpper - (currentUpper - currentMiddle) * 0.2;
  const lowerThreshold = currentLower + (currentMiddle - currentLower) * 0.2;
  if (currentPrice >= upperThreshold) return { upper: currentUpper, middle: currentMiddle, lower: currentLower, bandwidth, signal: 'OVERBOUGHT' };
  if (currentPrice <= lowerThreshold) return { upper: currentUpper, middle: currentMiddle, lower: currentLower, bandwidth, signal: 'OVERSOLD' };
  return { upper: currentUpper, middle: currentMiddle, lower: currentLower, bandwidth, signal: 'NEUTRAL' };
}

// All indicators analysis
export function analyzeAllIndicators(candles: Candle[]): { ma: MovingAverageResult; rsi: RSIResult; macd: MACDResult; bollinger: BollingerBandsResult; trend: 'BULLISH' | 'BEARISH' | 'NEUTRAL' } {
  const closes = candles.map((c) => c.close);
  const ma = analyzeMACrossover(closes, 9, 21, true);
  const rsi = analyzeRSI(closes, 14);
  const macd = analyzeMACD(closes);
  const bollinger = analyzeBollingerBands(closes);
  let signals = 0;
  if (ma.crossover === 'BULLISH') signals += 2;
  else if (ma.crossover === 'BEARISH') signals -= 2;
  if (rsi.signal === 'OVERSOLD') signals += 1;
  else if (rsi.signal === 'OVERBOUGHT') signals -= 1;
  if (macd.crossover === 'BULLISH') signals += 1;
  else if (macd.crossover === 'BEARISH') signals -= 1;
  let trend: 'BULLISH' | 'BEARISH' | 'NEUTRAL' = 'NEUTRAL';
  if (signals >= 3) trend = 'BULLISH';
  else if (signals <= -3) trend = 'BEARISH';
  return { ma, rsi, macd, bollinger, trend };
}
