import { renderHook, act } from '@testing-library/react';
import { vi, describe, it, expect, beforeEach } from 'vitest';
import { useEngineControl } from '../useEngineControl';
import * as api from '../useEngineAPI';

vi.mock('../useEngineAPI', () => ({
    engineFetch: vi.fn(),
}));

describe('useEngineControl', () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    it('startEngine sets loading and calls API', async () => {
        const setState = vi.fn();
        const fetchStatus = vi.fn().mockResolvedValue(true);
        const fetchConfig = vi.fn().mockResolvedValue(true);

        vi.mocked(api.engineFetch).mockResolvedValue({ success: true });

        const { result } = renderHook(() => useEngineControl(setState, { fetchStatus, fetchConfig }));

        let res;
        await act(async () => {
            res = await result.current.startEngine();
        });

        expect(api.engineFetch).toHaveBeenCalledWith('/api/control', {
            method: 'POST',
            body: JSON.stringify({ action: 'start' }),
        });

        expect(setState).toHaveBeenCalled();
        expect(fetchStatus).toHaveBeenCalled();
        expect(res).toEqual({ success: true });
    });

    it('pauseEngine calls API correctly', async () => {
        const setState = vi.fn();
        const fetchStatus = vi.fn().mockResolvedValue(true);
        const fetchConfig = vi.fn().mockResolvedValue(true);

        vi.mocked(api.engineFetch).mockResolvedValue({ success: true });

        const { result } = renderHook(() => useEngineControl(setState, { fetchStatus, fetchConfig }));

        let res;
        await act(async () => {
            res = await result.current.pauseEngine();
        });

        expect(api.engineFetch).toHaveBeenCalledWith('/api/control', {
            method: 'POST',
            body: JSON.stringify({ action: 'pause' }),
        });

        expect(fetchStatus).toHaveBeenCalled();
        expect(res).toEqual({ success: true });
    });
});
