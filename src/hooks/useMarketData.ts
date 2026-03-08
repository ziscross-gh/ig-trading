'use client';

import { useState, useCallback, useEffect, useRef } from 'react';
import type { Market, Candle } from '@/types/ig';
import { DEFAULT_MARKETS, MARKET_NAMES } from '@/types/ig';

const ENGINE_BASE = process.env.NEXT_PUBLIC_ENGINE_URL || 'http://localhost:9090';
const WS_URL = ENGINE_BASE.replace(/^http/, 'ws') + '/api/ws';

// Slow polling fallback interval (ms) — only kicks in when WebSocket is down
const FALLBACK_POLL_MS = 30_000;

interface MarketDataState {
  markets: Market[];
  selectedEpic: string;
  historicalData: Candle[];
  loading: boolean;
  error: string | null;
  lastUpdate: Date | null;
  connected: boolean;
}

// Shape of the MarketUpdate payload from the engine WebSocket
interface WsMarketUpdate {
  epic: string;
  bid: number;
  ask: number;
  spread: number;
  high: number;
  low: number;
  change_pct: number;
  last_update: string;
}

// Fallback mock prices when engine is unreachable
function generateFallbackMarkets(): Market[] {
  const basePrices: Record<string, number> = {
    'CS.D.CFIGOLD.CFI.IP': 2350.50,
    'CS.D.EURUSD.CFD': 1.0850,
    'CS.D.GBPUSD.CFD': 1.2700,
    'CS.D.USDJPY.CFD': 149.50,
  };
  return Object.values(DEFAULT_MARKETS).map((epic) => {
    const basePrice = basePrices[epic] || 100;
    const change = (Math.random() - 0.5) * basePrice * 0.01;
    return {
      epic,
      name: MARKET_NAMES[epic] || epic,
      bid: basePrice + change,
      offer: basePrice + change + basePrice * 0.0002,
      high: basePrice + Math.random() * basePrice * 0.005,
      low: basePrice - Math.random() * basePrice * 0.005,
      change,
      changePercent: (change / basePrice) * 100,
      delayTime: 0,
      expiry: '',
      InstrumentType: 'CURRENCIES',
      marketStatus: 'TRADEABLE' as const,
    };
  });
}

function wsMarketToMarket(ws: WsMarketUpdate): Market {
  const mid = (ws.bid + ws.ask) / 2;
  const netChange = mid * ws.change_pct / 100;
  return {
    epic: ws.epic,
    name: MARKET_NAMES[ws.epic] || ws.epic,
    bid: ws.bid,
    offer: ws.ask,
    high: ws.high,
    low: ws.low,
    change: netChange,
    changePercent: ws.change_pct,
    delayTime: 0,
    expiry: '',
    InstrumentType: 'CURRENCIES',
    marketStatus: 'TRADEABLE' as const,
  };
}

export function useMarketData() {
  const [state, setState] = useState<MarketDataState>({
    markets: [],
    selectedEpic: DEFAULT_MARKETS.GOLD,
    historicalData: [],
    loading: false,
    error: null,
    lastUpdate: null,
    connected: false,
  });

  const wsRef = useRef<WebSocket | null>(null);
  const fallbackTimerRef = useRef<NodeJS.Timeout | null>(null);
  const wsConnectedRef = useRef(false);

  // ── REST: full market snapshot (used on mount + fallback polling) ──────────
  const fetchPrices = useCallback(async () => {
    try {
      const response = await fetch(`${ENGINE_BASE}/api/markets`);
      if (!response.ok) throw new Error('Engine not available');
      const data = await response.json();
      const markets: Market[] = (data.markets || []).map((m: Record<string, unknown>) => ({
        epic: m.epic as string,
        name: (m.name as string) || MARKET_NAMES[m.epic as string] || (m.epic as string),
        bid: m.bid as number,
        offer: m.offer as number,
        high: m.high as number,
        low: m.low as number,
        change: m.change as number,
        changePercent: m.changePercent as number,
        delayTime: 0,
        expiry: '',
        InstrumentType: 'CURRENCIES',
        marketStatus: 'TRADEABLE' as const,
      }));
      setState((prev) => ({ ...prev, markets, connected: true, lastUpdate: new Date(), error: null }));
    } catch {
      setState((prev) => ({ ...prev, markets: generateFallbackMarkets(), connected: false, lastUpdate: new Date() }));
    }
  }, []);

  // ── REST: price history (candles) ─────────────────────────────────────────
  const fetchHistory = useCallback(async (epic?: string) => {
    const targetEpic = epic ?? state.selectedEpic;
    try {
      setState((prev) => ({ ...prev, loading: true }));
      const response = await fetch(`${ENGINE_BASE}/api/prices?epic=${targetEpic}&resolution=HOUR&max=100`);
      if (!response.ok) throw new Error('Engine not available');
      const data = await response.json();
      const candles: Candle[] = (data.prices || data.candles || []).map((c: Record<string, unknown>) => ({
        open: c.open as number,
        high: c.high as number,
        low: c.low as number,
        close: c.close as number,
        volume: (c.volume as number) || 0,
        timestamp: (c.timestamp as string) || new Date().toISOString(),
      }));
      setState((prev) => ({ ...prev, historicalData: candles, selectedEpic: targetEpic, loading: false, error: null }));
      return candles;
    } catch {
      setState((prev) => ({ ...prev, historicalData: [], selectedEpic: targetEpic, loading: false, error: 'Price history unavailable' }));
      return [];
    }
  }, [state.selectedEpic]);

  // ── WebSocket: real-time market tick updates ───────────────────────────────
  const connectWS = useCallback(function connect() {
    if (wsRef.current?.readyState === WebSocket.OPEN) return;

    const socket = new WebSocket(WS_URL);

    socket.onopen = () => {
      wsConnectedRef.current = true;
      setState((prev) => ({ ...prev, connected: true }));
      // Cancel fallback polling — WebSocket is live
      if (fallbackTimerRef.current) {
        clearInterval(fallbackTimerRef.current);
        fallbackTimerRef.current = null;
      }
    };

    socket.onmessage = (event) => {
      try {
        const { type, data } = JSON.parse(event.data) as { type: string; data: unknown };
        if (type === 'MarketUpdate') {
          const ms = data as WsMarketUpdate;
          const updated = wsMarketToMarket(ms);
          setState((prev) => {
            const exists = prev.markets.some((m) => m.epic === ms.epic);
            const markets = exists
              ? prev.markets.map((m) => (m.epic === ms.epic ? updated : m))
              : [...prev.markets, updated];
            return { ...prev, markets, lastUpdate: new Date() };
          });
        }
      } catch {
        // malformed frame — ignore
      }
    };

    socket.onclose = () => {
      wsConnectedRef.current = false;
      setState((prev) => ({ ...prev, connected: false }));
      // Start fallback polling while WebSocket is down
      if (!fallbackTimerRef.current) {
        fallbackTimerRef.current = setInterval(fetchPrices, FALLBACK_POLL_MS);
      }
      // Auto-reconnect after 5 s
      setTimeout(connect, 5_000);
    };

    socket.onerror = () => {
      socket.close();
    };

    wsRef.current = socket;
  }, [fetchPrices]);

  const selectMarket = useCallback((epic: string) => {
    setState((prev) => ({ ...prev, selectedEpic: epic }));
    fetchHistory(epic);
  }, [fetchHistory]);

  // ── Lifecycle ─────────────────────────────────────────────────────────────
  useEffect(() => {
    // Seed with REST snapshot, then establish WebSocket
    fetchPrices();
    fetchHistory();
    connectWS();

    return () => {
      if (wsRef.current) {
        wsRef.current.onclose = null; // suppress auto-reconnect on unmount
        wsRef.current.close();
        wsRef.current = null;
      }
      if (fallbackTimerRef.current) {
        clearInterval(fallbackTimerRef.current);
        fallbackTimerRef.current = null;
      }
    };
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []); // run once on mount

  // Keep startAutoRefresh/stopAutoRefresh as no-ops for API compatibility
  // (page.tsx calls them but WebSocket now handles real-time updates)
  const startAutoRefresh = useCallback(() => {}, []);
  const stopAutoRefresh = useCallback(() => {}, []);

  return {
    ...state,
    fetchPrices,
    fetchHistory,
    selectMarket,
    startAutoRefresh,
    stopAutoRefresh,
    marketName: state.markets.find(m => m.epic === state.selectedEpic)?.name || MARKET_NAMES[state.selectedEpic] || state.selectedEpic,
  };
}
