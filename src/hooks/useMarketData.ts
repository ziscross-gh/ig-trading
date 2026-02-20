'use client';

import { useState, useCallback, useEffect, useRef } from 'react';
import type { Market, Candle, Position, AccountDetails } from '@/types/ig';
import { DEFAULT_MARKETS, MARKET_NAMES } from '@/types/ig';

interface MarketDataState {
  markets: Market[];
  selectedEpic: string;
  historicalData: Candle[];
  positions: Position[];
  account: AccountDetails | null;
  loading: boolean;
  error: string | null;
  lastUpdate: Date | null;
}

const REFRESH_INTERVAL = 10000;

// Generate mock data for demo
function generateMockMarkets(): Market[] {
  const basePrices: Record<string, number> = {
    'CS.D.GOLDUSD.CFD': 2350.50,
    'CS.D.EURUSD.CFD': 1.0850,
    'CS.D.GBPUSD.CFD': 1.2700,
    'CS.D.USDJPY.CFD': 149.50,
    'CS.D.AUDUSD.CFD': 0.6650,
  };
  return Object.entries(DEFAULT_MARKETS).map(([, epic]) => {
    const basePrice = basePrices[epic] || 100;
    const change = (Math.random() - 0.5) * basePrice * 0.02;
    return {
      epic,
      name: MARKET_NAMES[epic] || epic,
      bid: basePrice + change,
      offer: basePrice + change + basePrice * 0.0002,
      high: basePrice + Math.random() * basePrice * 0.01,
      low: basePrice - Math.random() * basePrice * 0.01,
      change,
      changePercent: (change / basePrice) * 100,
      delayTime: 0,
      expiry: '',
      InstrumentType: 'CURRENCIES',
      marketStatus: 'TRADEABLE' as const,
    };
  });
}

function generateMockCandles(basePrice: number, count: number): Candle[] {
  const candles: Candle[] = [];
  let price = basePrice;
  const now = Date.now();
  for (let i = 0; i < count; i++) {
    const change = (Math.random() - 0.48) * price * 0.005;
    const open = price;
    const close = price + change;
    const high = Math.max(open, close) + Math.random() * price * 0.002;
    const low = Math.min(open, close) - Math.random() * price * 0.002;
    candles.push({
      open,
      high,
      low,
      close,
      volume: Math.floor(Math.random() * 1000),
      timestamp: new Date(now - (count - i) * 3600000).toISOString(),
    });
    price = close;
  }
  return candles;
}

export function useMarketData(mockMode: boolean = true) {
  const [state, setState] = useState<MarketDataState>({
    markets: [],
    selectedEpic: DEFAULT_MARKETS.GOLD,
    historicalData: [],
    positions: [],
    account: null,
    loading: false,
    error: null,
    lastUpdate: null,
  });
  const intervalRef = useRef<NodeJS.Timeout | null>(null);

  const fetchPrices = useCallback(async () => {
    try {
      if (mockMode) {
        const markets = generateMockMarkets();
        setState((prev) => ({ ...prev, markets, lastUpdate: new Date(), error: null }));
      }
    } catch (error) {
      console.error('Failed to fetch prices:', error);
    }
  }, [mockMode]);

  const fetchHistory = useCallback(async (epic: string = state.selectedEpic) => {
    try {
      setState((prev) => ({ ...prev, loading: true }));
      const basePrices: Record<string, number> = {
        'CS.D.GOLDUSD.CFD': 2350.50,
        'CS.D.EURUSD.CFD': 1.0850,
        'CS.D.GBPUSD.CFD': 1.2700,
        'CS.D.USDJPY.CFD': 149.50,
        'CS.D.AUDUSD.CFD': 0.6650,
      };
      const basePrice = basePrices[epic] || 100;
      const candles = generateMockCandles(basePrice, 100);
      setState((prev) => ({ ...prev, historicalData: candles, selectedEpic: epic, loading: false, error: null }));
      return candles;
    } catch (error) {
      setState((prev) => ({ ...prev, loading: false, error: 'Failed to fetch historical data' }));
      return [];
    }
  }, [state.selectedEpic]);

  const fetchPositions = useCallback(async () => {
    try {
      if (mockMode) {
        setState((prev) => ({ ...prev, positions: [] }));
      }
    } catch (error) {
      console.error('Failed to fetch positions:', error);
    }
  }, [mockMode]);

  const selectMarket = useCallback((epic: string) => {
    setState((prev) => ({ ...prev, selectedEpic: epic }));
    fetchHistory(epic);
  }, [fetchHistory]);

  const startAutoRefresh = useCallback((interval: number = REFRESH_INTERVAL) => {
    if (intervalRef.current) clearInterval(intervalRef.current);
    intervalRef.current = setInterval(() => { fetchPrices(); fetchPositions(); }, interval);
  }, [fetchPrices, fetchPositions]);

  const stopAutoRefresh = useCallback(() => {
    if (intervalRef.current) { clearInterval(intervalRef.current); intervalRef.current = null; }
  }, []);

  useEffect(() => {
    fetchPrices();
    fetchHistory();
    fetchPositions();
  }, [fetchPrices, fetchHistory, fetchPositions]);

  useEffect(() => {
    return () => stopAutoRefresh();
  }, [stopAutoRefresh]);

  return { ...state, fetchPrices, fetchHistory, fetchPositions, selectMarket, startAutoRefresh, stopAutoRefresh, marketName: MARKET_NAMES[state.selectedEpic] || state.selectedEpic };
}
