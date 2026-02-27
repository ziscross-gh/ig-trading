'use client';

import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Bot, Cpu, FlaskConical, Rocket, X, Menu } from 'lucide-react';
import type { EngineStatus, EngineConfig } from '@/hooks/engine/types';

interface DashboardHeaderEngine {
    connected: boolean;
    isRunning: boolean;
    status: EngineStatus | null;
    config: EngineConfig | null;
}

interface DashboardHeaderProps {
    activeTab: string;
    setActiveTab: (tab: string) => void;
    engine: DashboardHeaderEngine;
    mobileMenuOpen: boolean;
    setMobileMenuOpen: (open: boolean) => void;
}

export function DashboardHeader({
    activeTab,
    setActiveTab,
    engine,
    mobileMenuOpen,
    setMobileMenuOpen
}: DashboardHeaderProps) {
    return (
        <header className="sticky top-0 z-50 border-b bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/60">
            <div className="container flex h-14 items-center px-4">
                <div className="flex items-center gap-2 mr-4">
                    <Bot className="h-6 w-6 text-primary" />
                    <span className="font-bold text-lg hidden sm:inline">IG Trading Bot</span>
                </div>

                {/* Main Navigation — 3 tabs only */}
                <Tabs value={activeTab} onValueChange={setActiveTab} className="hidden md:block">
                    <TabsList>
                        <TabsTrigger value="engine" className="flex items-center gap-1">
                            <Cpu className="h-4 w-4" />
                            Engine
                        </TabsTrigger>
                        <TabsTrigger value="strategy-lab" className="flex items-center gap-1">
                            <FlaskConical className="h-4 w-4" />
                            Strategy Lab
                        </TabsTrigger>
                        <TabsTrigger value="setup" className="flex items-center gap-1">
                            <Rocket className="h-4 w-4" />
                            Setup
                        </TabsTrigger>
                    </TabsList>
                </Tabs>

                <div className="flex items-center gap-2 ml-auto">
                    {engine.config?.mode === 'paper' && (
                        <Badge variant="outline" className="bg-yellow-500/10 text-yellow-600 border-yellow-500/20">
                            Paper Mode
                        </Badge>
                    )}

                    <Badge
                        variant={engine.connected ? 'default' : 'secondary'}
                        className={`flex items-center gap-1 ${engine.isRunning ? 'bg-emerald-500 hover:bg-emerald-600' :
                                engine.connected ? 'bg-blue-500 hover:bg-blue-600' : ''
                            }`}
                    >
                        <Cpu className={`h-3 w-3 ${engine.isRunning ? 'animate-pulse' : ''}`} />
                        {engine.connected ? (engine.status?.status?.toUpperCase() || 'IDLE') : 'Engine Off'}
                    </Badge>
                </div>

                <Button
                    variant="ghost"
                    size="sm"
                    className="ml-2 md:hidden"
                    onClick={() => setMobileMenuOpen(!mobileMenuOpen)}
                >
                    {mobileMenuOpen ? <X className="h-5 w-5" /> : <Menu className="h-5 w-5" />}
                </Button>
            </div>
        </header>
    );
}
