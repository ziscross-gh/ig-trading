'use client';

import { useState, useEffect, useCallback } from 'react';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { ScrollArea } from '@/components/ui/scroll-area';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Calendar, Clock, AlertTriangle, RefreshCw } from 'lucide-react';

interface EconomicEvent {
  id: string;
  date: Date;
  time: string;
  currency: string;
  name: string;
  impact: 'HIGH' | 'MEDIUM' | 'LOW';
  forecast?: string;
  previous?: string;
}

const IMPACT_COLORS = {
  HIGH: 'bg-red-500/10 text-red-500 border-red-500/20',
  MEDIUM: 'bg-yellow-500/10 text-yellow-600 border-yellow-500/20',
  LOW: 'bg-green-500/10 text-green-500 border-green-500/20',
};

const CURRENCY_FLAGS: Record<string, string> = {
  USD: '🇺🇸', EUR: '🇪🇺', GBP: '🇬🇧', JPY: '🇯🇵', AUD: '🇦🇺',
};

function generateMockEvents(): EconomicEvent[] {
  const events: EconomicEvent[] = [];
  const now = new Date();
  for (let d = 0; d < 7; d++) {
    const date = new Date(now.getTime() + d * 24 * 60 * 60 * 1000);
    if (date.getDay() === 0 || date.getDay() === 6) continue;
    const numEvents = 2 + Math.floor(Math.random() * 3);
    for (let i = 0; i < numEvents; i++) {
      const hour = 8 + Math.floor(Math.random() * 10);
      const eventDate = new Date(date);
      eventDate.setHours(hour, Math.random() > 0.5 ? 0 : 30, 0, 0);
      if (d === 0 && eventDate < now) continue;
      events.push({
        id: `event-${d}-${i}`,
        date: eventDate,
        time: `${hour.toString().padStart(2, '0')}:${Math.random() > 0.5 ? '00' : '30'}`,
        currency: ['USD', 'EUR', 'GBP', 'JPY', 'AUD'][Math.floor(Math.random() * 5)],
        name: ['Non-Farm Payrolls', 'CPI', 'GDP', 'Interest Rate', 'PMI', 'Unemployment'][Math.floor(Math.random() * 6)],
        impact: ['HIGH', 'MEDIUM', 'LOW'][Math.floor(Math.random() * 3)] as 'HIGH' | 'MEDIUM' | 'LOW',
        forecast: (Math.random() * 10 - 5).toFixed(1),
        previous: (Math.random() * 10 - 5).toFixed(1),
      });
    }
  }
  return events.sort((a, b) => new Date(a.date).getTime() - new Date(b.date).getTime());
}

export function EconomicCalendarPanel() {
  const [events, setEvents] = useState<EconomicEvent[]>([]);
  const [loading, setLoading] = useState(false);
  const [filter, setFilter] = useState<'all' | 'HIGH' | 'MEDIUM'>('HIGH');

  const fetchEvents = useCallback(async () => {
    setLoading(true);
    try {
      setEvents(generateMockEvents());
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchEvents();
    const interval = setInterval(fetchEvents, 300000);
    return () => clearInterval(interval);
  }, [fetchEvents]);

  const filteredEvents = events.filter((e) => {
    if (filter === 'all') return true;
    const impactOrder = { HIGH: 0, MEDIUM: 1, LOW: 2 };
    return impactOrder[e.impact] <= impactOrder[filter];
  });

  const formatEventTime = (date: Date) => {
    const d = new Date(date);
    const now = new Date();
    const isToday = d.toDateString() === now.toDateString();
    if (isToday) return `Today ${d.toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit', hour12: false })}`;
    return d.toLocaleDateString('en-US', { weekday: 'short', month: 'short', day: 'numeric' }) + ` ${d.toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit', hour12: false })}`;
  };

  return (
    <Card>
      <CardHeader className="pb-3">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Calendar className="h-5 w-5 text-primary" />
            <CardTitle className="text-lg">Economic Calendar</CardTitle>
          </div>
          <div className="flex items-center gap-2">
            <Button variant={filter === 'HIGH' ? 'default' : 'outline'} size="sm" onClick={() => setFilter('HIGH')}>High</Button>
            <Button variant={filter === 'MEDIUM' ? 'default' : 'outline'} size="sm" onClick={() => setFilter('MEDIUM')}>Medium+</Button>
            <Button variant={filter === 'all' ? 'default' : 'outline'} size="sm" onClick={() => setFilter('all')}>All</Button>
            <Button variant="ghost" size="icon" onClick={fetchEvents} disabled={loading}>
              <RefreshCw className={`h-4 w-4 ${loading ? 'animate-spin' : ''}`} />
            </Button>
          </div>
        </div>
        <CardDescription>Upcoming market-moving events</CardDescription>
      </CardHeader>
      <CardContent>
        {filteredEvents.length === 0 ? (
          <div className="text-center text-muted-foreground py-8">No events to display</div>
        ) : (
          <ScrollArea className="h-[400px] pr-4">
            <div className="space-y-2">
              {filteredEvents.map((event) => (
                <div key={event.id} className={`p-3 rounded-lg border ${event.impact === 'HIGH' ? 'border-red-500/30 bg-red-500/5' : 'border-border'}`}>
                  <div className="flex items-start justify-between">
                    <div className="flex items-center gap-2">
                      <span className="text-lg">{CURRENCY_FLAGS[event.currency] || '🌐'}</span>
                      <div>
                        <div className="font-medium text-sm">{event.name}</div>
                        <div className="text-xs text-muted-foreground">{event.currency}</div>
                      </div>
                    </div>
                    <Badge variant="outline" className={IMPACT_COLORS[event.impact]}>{event.impact}</Badge>
                  </div>
                  <div className="mt-2 flex items-center gap-1 text-xs text-muted-foreground">
                    <Clock className="h-3 w-3" />
                    <span>{formatEventTime(event.date)}</span>
                  </div>
                </div>
              ))}
            </div>
          </ScrollArea>
        )}
      </CardContent>
    </Card>
  );
}
