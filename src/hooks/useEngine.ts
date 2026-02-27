'use client';

import { useState, useCallback, useEffect, useRef } from 'react';

// Export types from the unified types file to avoid breaking existing imports
export * from './engine/types';

import type { EngineState } from './engine/types';
import { useEngineAPI } from './engine/useEngineAPI';
import { useEngineWebSocket } from './engine/useEngineWebSocket';
import { useEngineControl } from './engine/useEngineControl';

export function useEngine() {
  const [state, setState] = useState<EngineState>({
    connected: false,
    status: null,
    stats: null,
    learning: null,
    positions: [],
    signals: [],
    trades: [],
    config: null,
    indicators: {},
    health: null,
    optimizationResult: null,
    loading: false,
    error: null,
    lastUpdate: null,
  });

  const intervalRef = useRef<NodeJS.Timeout | null>(null);

  // Compose all the engine hooks
  const api = useEngineAPI(setState);

  const websocket = useEngineWebSocket(setState, {
    fetchStatus: api.fetchStatus,
    fetchPositions: api.fetchPositions,
    fetchTrades: api.fetchTrades,
    fetchConfig: api.fetchConfig,
    fetchStats: api.fetchStats,
    fetchLearning: api.fetchLearning,
  });

  const control = useEngineControl(setState, {
    fetchStatus: api.fetchStatus,
    fetchConfig: api.fetchConfig,
  });

  // ── Auto-refresh ──
  const startAutoRefresh = useCallback((interval = 5000) => {
    if (intervalRef.current) clearInterval(intervalRef.current);
    intervalRef.current = setInterval(() => {
      api.fetchStatus();
      api.fetchPositions();
    }, interval);
  }, [api]);

  const stopAutoRefresh = useCallback(() => {
    if (intervalRef.current) {
      clearInterval(intervalRef.current);
      intervalRef.current = null;
    }
  }, []);

  // ── Lifecycle ──
  useEffect(() => {
    api.fetchStatus();
    api.fetchPositions();
    api.fetchConfig();
    api.fetchSignals();
    api.fetchTrades();
    api.fetchStats();
    api.fetchLearning();
    websocket.connectWS();
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    return () => {
      stopAutoRefresh();
      websocket.disconnectWS();
    };
  }, [stopAutoRefresh, websocket]);

  // ── Computed values ──
  const isRunning = state.status?.status === 'running';
  const isPaused = state.status?.status === 'paused';
  const isStopped = !state.status || state.status.status === 'stopped';
  const winRate = state.status?.daily_stats
    ? state.status.daily_stats.trades_today > 0
      ? (state.status.daily_stats.winning / state.status.daily_stats.trades_today) * 100
      : 0
    : 0;

  return {
    ...state,
    isRunning,
    isPaused,
    isStopped,
    winRate,

    // API
    ...api,

    // Control
    ...control,

    // Local / Misc
    startAutoRefresh,
    stopAutoRefresh,
  };
}
