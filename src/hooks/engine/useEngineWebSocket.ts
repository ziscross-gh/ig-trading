import { useCallback, useRef } from 'react';
import type { EngineState } from './types';
import { ENGINE_BASE } from './useEngineAPI';

export function useEngineWebSocket(
    setState: React.Dispatch<React.SetStateAction<EngineState>>,
    actions: {
        fetchStatus: () => void;
        fetchPositions: () => void;
        fetchTrades: () => void;
        fetchConfig: () => void;
        fetchStats: () => void;
        fetchLearning: () => void;
    }
) {
    const wsRef = useRef<WebSocket | null>(null);

    const connectWS = useCallback(function connectInner() {
        if (wsRef.current?.readyState === WebSocket.OPEN) return;

        const wsUrl = ENGINE_BASE.replace(/^http/, 'ws') + '/api/ws';
        const socket = new WebSocket(wsUrl);

        socket.onopen = () => {
            console.log('Engine WebSocket connected');
            setState(prev => ({ ...prev, connected: true }));
        };

        socket.onmessage = (event) => {
            try {
                const payload = JSON.parse(event.data);
                const { type, data } = payload;

                switch (type) {
                    case 'StatusChange':
                        actions.fetchStatus();
                        break;
                    case 'MarketUpdate':
                        break;
                    case 'IndicatorUpdate':
                        setState(prev => ({
                            ...prev,
                            indicators: {
                                ...prev.indicators,
                                [data.epic]: data.indicators
                            }
                        }));
                        break;
                    case 'Signal':
                        setState(prev => ({
                            ...prev,
                            signals: [data, ...prev.signals].slice(0, 100)
                        }));
                        break;
                    case 'TradeExecuted':
                    case 'PositionClosed':
                        actions.fetchPositions();
                        actions.fetchTrades();
                        actions.fetchStatus();
                        actions.fetchStats();
                        actions.fetchLearning();
                        break;
                    case 'Heartbeat':
                        setState(prev => ({
                            ...prev,
                            status: prev.status ? {
                                ...prev.status,
                                uptime_secs: data.uptime_secs,
                                open_positions: data.open_positions
                            } : null
                        }));
                        break;
                    case 'ConfigChanged':
                        actions.fetchConfig();
                        break;
                }
            } catch (e) {
                console.error('Failed to parse engine event:', e);
            }
        };

        socket.onclose = () => {
            console.log('Engine WebSocket disconnected');
            setState(prev => ({ ...prev, connected: false }));
            setTimeout(connectInner, 5000);
        };

        socket.onerror = (err) => {
            console.error('Engine WebSocket error:', err);
            socket.close();
        };

        wsRef.current = socket;
    }, [setState, actions]);

    const disconnectWS = useCallback(() => {
        if (wsRef.current) {
            // Prevent the auto-reconnect logic from firing on purposeful disconnect
            wsRef.current.onclose = null;
            wsRef.current.close();
            wsRef.current = null;
            setState(prev => ({ ...prev, connected: false }));
        }
    }, [setState]);

    return { connectWS, disconnectWS, wsRef };
}
