import { useCallback } from 'react';
import type {
    EngineState, EngineStatus, EnginePosition, EngineSignal, EngineTrade,
    EngineConfig, EngineStats, IndicatorsResponse, EngineLearning
} from './types';

export const ENGINE_BASE = process.env.NEXT_PUBLIC_ENGINE_URL || 'http://localhost:9090';

export async function engineFetch<T>(path: string, options?: RequestInit): Promise<T> {
    const url = `${ENGINE_BASE}${path}`;
    const res = await fetch(url, {
        ...options,
        headers: {
            'Content-Type': 'application/json',
            ...options?.headers,
        },
    });
    if (!res.ok) {
        const body = await res.text().catch(() => 'Unknown error');
        throw new Error(`Engine error ${res.status}: ${body}`);
    }
    return res.json();
}

export function useEngineAPI(setState: React.Dispatch<React.SetStateAction<EngineState>>) {
    const fetchStatus = useCallback(async () => {
        try {
            const status = await engineFetch<EngineStatus>('/api/status');
            setState((prev) => ({
                ...prev,
                connected: true,
                status,
                lastUpdate: new Date(),
                error: null,
            }));
            return status;
        } catch (error) {
            setState((prev) => ({
                ...prev,
                connected: false,
                error: error instanceof Error ? error.message : 'Engine unreachable',
            }));
            return null;
        }
    }, [setState]);

    const fetchHealth = useCallback(async () => {
        try {
            const health = await engineFetch<NonNullable<EngineState['health']>>('/api/health');
            setState((prev) => ({ ...prev, health }));
            return health;
        } catch (error) {
            console.error('Failed to fetch health:', error);
            return null;
        }
    }, [setState]);

    const fetchPositions = useCallback(async () => {
        try {
            const data = await engineFetch<{ positions: EnginePosition[] }>('/api/positions');
            setState((prev) => ({ ...prev, positions: data.positions }));
            return data.positions;
        } catch (error) {
            console.error('Failed to fetch positions:', error);
            return [];
        }
    }, [setState]);

    const fetchSignals = useCallback(async (limit = 50) => {
        try {
            const data = await engineFetch<{ signals: EngineSignal[] }>(`/api/signals?limit=${limit}`);
            setState((prev) => ({ ...prev, signals: data.signals }));
            return data.signals;
        } catch (error) {
            console.error('Failed to fetch signals:', error);
            return [];
        }
    }, [setState]);

    const fetchSignalsHistory = useCallback(async (limit = 200) => {
        try {
            const data = await engineFetch<{ signals: EngineSignal[] }>(`/api/signals-history?limit=${limit}`);
            setState((prev) => ({ ...prev, signals: data.signals }));
            return data.signals;
        } catch (error) {
            console.error('Failed to fetch signals history:', error);
            return [];
        }
    }, [setState]);

    const fetchTrades = useCallback(async (limit = 100) => {
        try {
            const data = await engineFetch<{ trades: EngineTrade[] }>(`/api/trades?limit=${limit}`);
            setState((prev) => ({ ...prev, trades: data.trades }));
            return data.trades;
        } catch (error) {
            console.error('Failed to fetch trades:', error);
            return [];
        }
    }, [setState]);

    const fetchConfig = useCallback(async () => {
        try {
            const config = await engineFetch<EngineConfig>('/api/config');
            setState((prev) => ({ ...prev, config }));
            return config;
        } catch (error) {
            console.error('Failed to fetch config:', error);
            return null;
        }
    }, [setState]);

    const fetchStats = useCallback(async () => {
        try {
            const stats = await engineFetch<EngineStats>('/api/stats');
            setState((prev) => ({ ...prev, stats }));
            return stats;
        } catch (error) {
            console.error('Failed to fetch stats:', error);
            return null;
        }
    }, [setState]);

    const fetchIndicatorsForEpic = useCallback(async (epic: string) => {
        try {
            const data = await engineFetch<IndicatorsResponse>(`/api/indicators?epic=${encodeURIComponent(epic)}`);
            if (data.available && data.indicators) {
                setState((prev) => ({
                    ...prev,
                    indicators: {
                        ...prev.indicators,
                        [epic]: data.indicators!,
                    },
                }));
            }
            return data;
        } catch (error) {
            console.error('Failed to fetch indicators for epic:', epic, error);
            return null;
        }
    }, [setState]);

    const fetchLearning = useCallback(async () => {
        try {
            const learning = await engineFetch<EngineLearning>('/api/learning');
            setState((prev) => ({ ...prev, learning }));
            return learning;
        } catch (error) {
            console.error('Failed to fetch learning data:', error);
            return null;
        }
    }, [setState]);

    const setMode = useCallback(async (mode: 'paper' | 'live') => {
        try {
            const data = await engineFetch<{ success: boolean; message: string; old_mode: string; new_mode: string }>(
                '/api/config/mode',
                {
                    method: 'POST',
                    body: JSON.stringify({ mode }),
                }
            );
            if (data.success) {
                await Promise.all([fetchConfig(), fetchStatus()]);
            }
            return data;
        } catch (error) {
            const msg = error instanceof Error ? error.message : 'Failed to switch mode';
            return { success: false, message: msg, old_mode: mode, new_mode: mode };
        }
    }, [fetchConfig, fetchStatus]);

    const refreshAll = useCallback(async () => {
        await Promise.all([
            fetchStatus(),
            fetchPositions(),
            fetchSignals(),
            fetchTrades(),
            fetchConfig(),
            fetchStats(),
            fetchLearning()
        ]);
    }, [fetchStatus, fetchPositions, fetchSignals, fetchTrades, fetchConfig, fetchStats, fetchLearning]);

    const fetchAllIndicators = useCallback(async (epics: string[]) => {
        await Promise.all(epics.map((epic) => fetchIndicatorsForEpic(epic)));
    }, [fetchIndicatorsForEpic]);

    return {
        fetchStatus,
        fetchHealth,
        fetchPositions,
        fetchSignals,
        fetchSignalsHistory,
        fetchTrades,
        fetchConfig,
        fetchStats,
        fetchIndicatorsForEpic,
        fetchLearning,
        setMode,
        refreshAll,
        fetchAllIndicators
    };
}
