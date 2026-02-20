'use client';

import { useState, useCallback, useEffect, useRef } from 'react';
import type { BotConfig, ActivityLog, TradeSignal, StrategyConfig, RiskConfig } from '@/types/ig';
import { DEFAULT_BOT_CONFIG } from '@/types/ig';

interface BotState {
  status: 'STOPPED' | 'STARTING' | 'RUNNING' | 'STOPPING' | 'ERROR';
  config: BotConfig | null;
  logs: ActivityLog[];
  recentSignals: TradeSignal[];
  stats: { signalsGenerated: number; tradesExecuted: number; uptime: number };
  loading: boolean;
  error: string | null;
}

export function useBot() {
  const [state, setState] = useState<BotState>({
    status: 'STOPPED',
    config: DEFAULT_BOT_CONFIG,
    logs: [],
    recentSignals: [],
    stats: { signalsGenerated: 0, tradesExecuted: 0, uptime: 0 },
    loading: false,
    error: null,
  });
  const intervalRef = useRef<NodeJS.Timeout | null>(null);

  const fetchStatus = useCallback(async () => {
    try {
      const response = await fetch('/api/bot/control?action=status');
      const data = await response.json();
      if (data.success) {
        setState((prev) => ({ ...prev, status: data.status, config: data.config, stats: data.stats }));
      }
    } catch (error) {
      console.error('Failed to fetch bot status:', error);
    }
  }, []);

  const fetchLogs = useCallback(async () => {
    try {
      const response = await fetch('/api/bot/control?action=logs&limit=50');
      const data = await response.json();
      if (data.success) {
        setState((prev) => ({ ...prev, logs: data.logs }));
      }
    } catch (error) {
      console.error('Failed to fetch logs:', error);
    }
  }, []);

  const start = useCallback(async () => {
    try {
      setState((prev) => ({ ...prev, loading: true, error: null }));
      const response = await fetch('/api/bot/control', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ action: 'start' }) });
      const data = await response.json();
      if (data.success) {
        setState((prev) => ({ ...prev, status: 'STARTING', loading: false }));
        setTimeout(fetchStatus, 1500);
        return { success: true };
      } else {
        setState((prev) => ({ ...prev, loading: false, error: data.error || 'Failed to start bot' }));
        return { success: false, error: data.error };
      }
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : 'Failed to start bot';
      setState((prev) => ({ ...prev, loading: false, error: errorMessage }));
      return { success: false, error: errorMessage };
    }
  }, [fetchStatus]);

  const stop = useCallback(async () => {
    try {
      setState((prev) => ({ ...prev, loading: true }));
      const response = await fetch('/api/bot/control', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ action: 'stop' }) });
      const data = await response.json();
      if (data.success) {
        setState((prev) => ({ ...prev, status: 'STOPPED', loading: false }));
        return { success: true };
      } else {
        setState((prev) => ({ ...prev, loading: false, error: data.error || 'Failed to stop bot' }));
        return { success: false, error: data.error };
      }
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : 'Failed to stop bot';
      setState((prev) => ({ ...prev, loading: false, error: errorMessage }));
      return { success: false, error: errorMessage };
    }
  }, []);

  const updateConfig = useCallback(async (config: Partial<BotConfig>) => {
    try {
      const response = await fetch('/api/bot/control', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ action: 'updateConfig', config }) });
      const data = await response.json();
      if (data.success) {
        setState((prev) => ({ ...prev, config: data.config }));
        return { success: true };
      }
      return { success: false, error: data.error };
    } catch (error) {
      return { success: false, error: 'Failed to update config' };
    }
  }, []);

  const updateRiskConfig = useCallback(async (riskConfig: Partial<RiskConfig>) => {
    try {
      const response = await fetch('/api/bot/control', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ action: 'updateRiskConfig', riskConfig }) });
      const data = await response.json();
      if (data.success) {
        setState((prev) => ({ ...prev, config: data.config }));
        return { success: true };
      }
      return { success: false, error: data.error };
    } catch (error) {
      return { success: false, error: 'Failed to update risk config' };
    }
  }, []);

  const updateStrategies = useCallback(async (strategies: StrategyConfig[]) => {
    try {
      const response = await fetch('/api/bot/strategy', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ action: 'update', strategies }) });
      const data = await response.json();
      if (data.success) return { success: true, strategies: data.strategies };
      return { success: false, error: data.error };
    } catch (error) {
      return { success: false, error: 'Failed to update strategies' };
    }
  }, []);

  const toggleStrategy = useCallback(async (name: string, enabled: boolean) => {
    try {
      const response = await fetch('/api/bot/strategy', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ action: 'toggle', name, enabled }) });
      const data = await response.json();
      if (data.success) return { success: true, strategy: data.strategy };
      return { success: false, error: data.error };
    } catch (error) {
      return { success: false, error: 'Failed to toggle strategy' };
    }
  }, []);

  const clearLogs = useCallback(async () => {
    try {
      const response = await fetch('/api/bot/control', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ action: 'clearLogs' }) });
      const data = await response.json();
      if (data.success) {
        setState((prev) => ({ ...prev, logs: [] }));
        return { success: true };
      }
      return { success: false };
    } catch {
      return { success: false };
    }
  }, []);

  const startAutoRefresh = useCallback((interval: number = 5000) => {
    if (intervalRef.current) clearInterval(intervalRef.current);
    intervalRef.current = setInterval(() => { fetchStatus(); fetchLogs(); }, interval);
  }, [fetchStatus, fetchLogs]);

  const stopAutoRefresh = useCallback(() => {
    if (intervalRef.current) { clearInterval(intervalRef.current); intervalRef.current = null; }
  }, []);

  useEffect(() => {
    const init = async () => {
      try {
        const response = await fetch('/api/bot/control?action=status');
        const data = await response.json();
        if (data.success) setState((prev) => ({ ...prev, status: data.status, config: data.config, stats: data.stats }));
      } catch {}
    };
    init();
  }, []);

  useEffect(() => {
    const init = async () => {
      try {
        const response = await fetch('/api/bot/control?action=logs&limit=50');
        const data = await response.json();
        if (data.success) setState((prev) => ({ ...prev, logs: data.logs }));
      } catch {}
    };
    init();
  }, []);

  useEffect(() => { return () => stopAutoRefresh(); }, [stopAutoRefresh]);

  return { ...state, start, stop, updateConfig, updateRiskConfig, updateStrategies, toggleStrategy, clearLogs, fetchStatus, fetchLogs, startAutoRefresh, stopAutoRefresh, isRunning: state.status === 'RUNNING', isStopped: state.status === 'STOPPED' };
}
