'use client';

import { useState, useCallback } from 'react';
import type { EngineConfig } from './types';

export function useEngineConfig() {
  const [config, setConfig] = useState<EngineConfig | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Fetch config from /api/engine/config
  const fetchConfig = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await fetch('/api/engine/config');
      if (!response.ok) {
        throw new Error(`Failed to fetch engine config: ${response.statusText}`);
      }
      const data = await response.json();
      setConfig(data);
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : 'Unknown error';
      setError(errorMessage);
    } finally {
      setLoading(false);
    }
  }, []);

  // Update config via PATCH/POST to /api/engine/config
  const updateConfig = useCallback(async (patch: Partial<EngineConfig>) => {
    setLoading(true);
    setError(null);
    try {
      const response = await fetch('/api/engine/config', {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(patch),
      });
      if (!response.ok) {
        throw new Error(`Failed to update engine config: ${response.statusText}`);
      }
      const data = await response.json();
      setConfig(data);
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : 'Unknown error';
      setError(errorMessage);
    } finally {
      setLoading(false);
    }
  }, []);

  return {
    config,
    loading,
    error,
    fetchConfig,
    updateConfig,
  };
}
