'use client';

import React, { createContext, useContext, useEffect, ReactNode } from 'react';
import { useEngine as useEngineHook } from '@/hooks/useEngine';
import { useMarketData as useMarketDataHook } from '@/hooks/useMarketData';

type EngineContextType = ReturnType<typeof useEngineHook>;
type MarketDataContextType = ReturnType<typeof useMarketDataHook>;

const EngineContext = createContext<EngineContextType | null>(null);
const MarketDataContext = createContext<MarketDataContextType | null>(null);

export function EngineProvider({ children }: { children: ReactNode }) {
    const engine = useEngineHook();
    const marketData = useMarketDataHook();

    // Unified auto-refresh orchestration — run once on mount
    useEffect(() => {
        engine.startAutoRefresh(5000);
        marketData.startAutoRefresh();
        return () => {
            engine.stopAutoRefresh();
            marketData.stopAutoRefresh();
        };
    // eslint-disable-next-line react-hooks/exhaustive-deps
    }, []);

    return (
        <EngineContext.Provider value={engine}>
            <MarketDataContext.Provider value={marketData}>
                {children}
            </MarketDataContext.Provider>
        </EngineContext.Provider>
    );
}

export function useEngine() {
    const context = useContext(EngineContext);
    if (!context) {
        throw new Error('useEngine must be used within an EngineProvider');
    }
    return context;
}

export function useMarketData() {
    const context = useContext(MarketDataContext);
    if (!context) {
        throw new Error('useMarketData must be used within a MarketDataProvider');
    }
    return context;
}
