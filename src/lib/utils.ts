import { clsx, type ClassValue } from "clsx"
import { twMerge } from "tailwind-merge"

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

// Format date/time in GMT+8 (Singapore Time)
const SGT_OPTIONS: Intl.DateTimeFormatOptions = {
  timeZone: 'Asia/Singapore',
  hour12: false,
};

export function formatTimeSGT(date: Date | string): string {
  const d = typeof date === 'string' ? new Date(date) : date;
  return d.toLocaleTimeString('en-US', { ...SGT_OPTIONS, hour: '2-digit', minute: '2-digit', second: '2-digit' });
}

export function formatDateTimeSGT(date: Date | string): string {
  const d = typeof date === 'string' ? new Date(date) : date;
  return d.toLocaleString('en-US', { ...SGT_OPTIONS, year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit', second: '2-digit' });
}

export function formatDateSGT(date: Date | string): string {
  const d = typeof date === 'string' ? new Date(date) : date;
  return d.toLocaleDateString('en-US', { ...SGT_OPTIONS, month: 'short', day: 'numeric' });
}
