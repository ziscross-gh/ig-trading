'use client';

import { useEffect } from 'react';
import { AlertCircle, RefreshCcw } from 'lucide-react';
import { Button } from '@/components/ui/button';

export default function GlobalError({
    error,
    reset,
}: {
    error: Error & { digest?: string };
    reset: () => void;
}) {
    useEffect(() => {
        // Log the error to an error reporting service
        console.error('Dashboard Error Global Boundary:', error);
    }, [error]);

    return (
        <div className="flex h-screen w-full flex-col items-center justify-center bg-gray-950 px-4 text-center">
            <div className="mx-auto flex max-w-[500px] flex-col items-center justify-center space-y-4 rounded-xl border border-red-500/20 bg-gray-900 p-8 shadow-2xl">
                <div className="rounded-full bg-red-500/10 p-3 text-red-500">
                    <AlertCircle className="h-10 w-10" />
                </div>
                <div className="space-y-2">
                    <h2 className="text-2xl font-bold tracking-tighter text-gray-50">
                        Something went wrong
                    </h2>
                    <p className="text-gray-400">
                        The dashboard encountered an unexpected error. The trading engine continues to run safely in the background.
                    </p>
                </div>
                <div className="mt-6 w-full max-w-sm rounded-md bg-gray-950 p-4 text-left text-sm text-red-400 overflow-auto">
                    <code className="text-xs">{error.message || 'Unknown error occurred'}</code>
                </div>
                <Button
                    onClick={() => reset()}
                    className="mt-4 bg-blue-600 hover:bg-blue-700 text-white min-w-[200px]"
                >
                    <RefreshCcw className="mr-2 h-4 w-4" />
                    Reload Dashboard
                </Button>
            </div>
        </div>
    );
}
