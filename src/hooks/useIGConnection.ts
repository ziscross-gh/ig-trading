'use client';

import { useState, useCallback } from 'react';
import type { IGCredentials, IGSession } from '@/types/ig';

interface ConnectionState {
  authenticated: boolean;
  session: Partial<IGSession> | null;
  loading: boolean;
  error: string | null;
  isDemo: boolean;
}

export function useIGConnection() {
  const [state, setState] = useState<ConnectionState>({
    authenticated: false,
    session: null,
    loading: false,
    error: null,
    isDemo: true,
  });

  const checkSession = useCallback(async () => {
    try {
      setState((prev) => ({ ...prev, loading: true }));
      const response = await fetch('/api/ig/auth');
      const data = await response.json();
      if (data.authenticated && data.session) {
        setState({ authenticated: true, session: data.session, loading: false, error: null, isDemo: true });
      } else {
        setState({ authenticated: false, session: null, loading: false, error: null, isDemo: true });
      }
    } catch {
      setState((prev) => ({ ...prev, loading: false, error: 'Failed to check session' }));
    }
  }, []);

  const connect = useCallback(async (credentials: IGCredentials) => {
    try {
      setState((prev) => ({ ...prev, loading: true, error: null }));
      const response = await fetch('/api/ig/auth', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(credentials) });
      const data = await response.json();
      if (data.success && data.session) {
        setState({ authenticated: true, session: data.session, loading: false, error: null, isDemo: credentials.isDemo });
        return { success: true };
      } else {
        setState((prev) => ({ ...prev, loading: false, error: data.error || 'Authentication failed' }));
        return { success: false, error: data.error };
      }
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : 'Connection failed';
      setState((prev) => ({ ...prev, loading: false, error: errorMessage }));
      return { success: false, error: errorMessage };
    }
  }, []);

  const disconnect = useCallback(async () => {
    try {
      setState((prev) => ({ ...prev, loading: true }));
      await fetch('/api/ig/auth', { method: 'DELETE' });
      setState({ authenticated: false, session: null, loading: false, error: null, isDemo: true });
      return { success: true };
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : 'Logout failed';
      setState((prev) => ({ ...prev, loading: false, error: errorMessage }));
      return { success: false, error: errorMessage };
    }
  }, []);

  const clearError = useCallback(() => {
    setState((prev) => ({ ...prev, error: null }));
  }, []);

  return { ...state, connect, disconnect, checkSession, clearError };
}
