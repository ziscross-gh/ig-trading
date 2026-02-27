'use client';

import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { Cpu, FlaskConical, Rocket } from 'lucide-react';

interface MobileNavProps {
    activeTab: string;
    setActiveTab: (tab: string) => void;
}

export function MobileNav({ activeTab, setActiveTab }: MobileNavProps) {
    return (
        <div className="md:hidden mb-4">
            <Tabs value={activeTab} onValueChange={setActiveTab} className="w-full">
                <TabsList className="grid grid-cols-3 w-full h-auto">
                    <TabsTrigger value="engine">
                        <Cpu className="h-4 w-4" />
                    </TabsTrigger>
                    <TabsTrigger value="strategy-lab">
                        <FlaskConical className="h-4 w-4" />
                    </TabsTrigger>
                    <TabsTrigger value="setup">
                        <Rocket className="h-4 w-4" />
                    </TabsTrigger>
                </TabsList>
            </Tabs>
        </div>
    );
}
