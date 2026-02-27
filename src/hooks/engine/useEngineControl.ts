import { useCallback } from 'react';
import type { EngineState, EngineStatus, EngineConfig, OptimizationResult, BacktestResultFull } from './types';
import { engineFetch } from './useEngineAPI';

export function useEngineControl(
    setState: React.Dispatch<React.SetStateAction<EngineState>>,
    actions: {
        fetchStatus: () => Promise<EngineStatus | null>;
        fetchConfig: () => Promise<EngineConfig | null>;
    }
) {
    const startEngine = useCallback(async () => {
        try {
            setState((prev) => ({ ...prev, loading: true }));
            await engineFetch('/api/control', {
                method: 'POST',
                body: JSON.stringify({ action: 'start' }),
            });
            setState((prev) => ({ ...prev, loading: false }));
            await actions.fetchStatus();
            return { success: true };
        } catch (error) {
            const msg = error instanceof Error ? error.message : 'Failed to start engine';
            setState((prev) => ({ ...prev, loading: false, error: msg }));
            return { success: false, error: msg };
        }
    }, [setState, actions]);

    const stopEngine = useCallback(async () => {
        try {
            setState((prev) => ({ ...prev, loading: true }));
            await engineFetch('/api/control', {
                method: 'POST',
                body: JSON.stringify({ action: 'stop' }),
            });
            setState((prev) => ({ ...prev, loading: false }));
            await actions.fetchStatus();
            return { success: true };
        } catch (error) {
            const msg = error instanceof Error ? error.message : 'Failed to stop engine';
            setState((prev) => ({ ...prev, loading: false, error: msg }));
            return { success: false, error: msg };
        }
    }, [setState, actions]);

    const pauseEngine = useCallback(async () => {
        try {
            await engineFetch('/api/control', {
                method: 'POST',
                body: JSON.stringify({ action: 'pause' }),
            });
            await actions.fetchStatus();
            return { success: true };
        } catch (error) {
            const msg = error instanceof Error ? error.message : 'Failed to pause engine';
            return { success: false, error: msg };
        }
    }, [actions]);

    const startOptimization = useCallback(async (params: {
        epic: string;
        short_range: [number, number];
        long_range: [number, number];
    }) => {
        try {
            setState(prev => ({ ...prev, loading: true }));
            const data = await engineFetch<{ success: boolean; message: string }>('/api/optimize', {
                method: 'POST',
                body: JSON.stringify(params),
            });
            setState(prev => ({ ...prev, loading: false }));
            return data;
        } catch {
            setState(prev => ({ ...prev, loading: false, error: 'Failed to start optimization' }));
            return { success: false, error: 'Failed to start optimization' };
        }
    }, [setState]);

    const fetchOptimizationResults = useCallback(async () => {
        try {
            const data = await engineFetch<{ success: boolean; result?: OptimizationResult }>('/api/optimizer/results');
            if (data.success && data.result) {
                setState(prev => ({ ...prev, optimizationResult: data.result! }));
            }
            return data;
        } catch (error) {
            console.error('Failed to fetch optimization results:', error);
            return { success: false };
        }
    }, [setState]);

    // Actually a config action, but control fits
    const updateRisk = useCallback(async (updates: {
        max_risk_per_trade?: number;
        max_daily_loss_pct?: number;
        max_open_positions?: number;
    }) => {
        try {
            await engineFetch('/api/config', {
                method: 'PUT',
                body: JSON.stringify(updates),
            });
            await actions.fetchConfig();
            return { success: true };
        } catch {
            return { success: false, error: 'Failed to update risk config' };
        }
    }, [actions]);

    const toggleStrategy = useCallback(async (name: string, enabled: boolean) => {
        try {
            await engineFetch('/api/config', {
                method: 'PUT',
                body: JSON.stringify({ updates: { strategies: { [name]: { enabled } } } }),
            });
            await actions.fetchConfig();
            return { success: true };
        } catch {
            return { success: false, error: 'Failed to toggle strategy' };
        }
    }, [actions]);

    const updateStrategy = useCallback(async (name: string, params: Record<string, unknown>) => {
        try {
            await engineFetch('/api/config', {
                method: 'PUT',
                body: JSON.stringify({ updates: { strategies: { [name]: params } } }),
            });
            await actions.fetchConfig();
            return { success: true };
        } catch {
            return { success: false, error: 'Failed to update strategy parameters' };
        }
    }, [actions]);

    const runBacktest = useCallback(async (params: {
        epic: string;
        strategy_name: string;
        initial_balance: number;
        risk_pct: number;
    }) => {
        try {
            return await engineFetch<{ success: boolean; result: BacktestResultFull }>('/api/backtest', {
                method: 'POST',
                body: JSON.stringify(params),
            });
        } catch {
            return { success: false, error: 'Failed to run backtest' };
        }
    }, []);

    return {
        startEngine,
        stopEngine,
        pauseEngine,
        startOptimization,
        fetchOptimizationResults,
        updateRisk,
        toggleStrategy,
        updateStrategy,
        runBacktest,
    };
}
