#!/usr/bin/env python3
"""
fetch_calendar.py — Download this week + next week's HIGH-impact economic events
from ForexFactory and write them to data/economic_calendar.json.

Run daily via cron (before London open is ideal):
  0 6 * * * cd /path/to/ig-engine && python3 scripts/fetch_calendar.py

The engine reads data/economic_calendar.json to decide live blackout windows
instead of firing the same static times every single day.
"""

import json
import os
import sys
from datetime import datetime, timezone, timedelta
from urllib.request import urlopen, Request
from urllib.error import URLError
from typing import Optional

# Only blackout for these currencies (instruments we trade)
WATCHED_CURRENCIES = {"USD", "EUR", "JPY", "GBP", "XAU", "AUD"}

# Blackout minutes around each event (overridden per title below)
DEFAULT_BLACKOUT_MINS = 30

# Per-keyword blackout overrides (checked against event title)
BLACKOUT_OVERRIDES = {
    "FOMC":          60,
    "Fed":           60,
    "Rate Decision": 60,
    "Press Conference": 45,
    "ECB":           45,
    "BOE":           45,
    "Bank of England": 45,
    "NFP":           30,
    "Nonfarm":       30,
    "CPI":           30,
    "Core CPI":      30,
    "PCE":           30,
    "Retail Sales":  20,
    "GDP":           20,
    "PMI":           20,
    "ISM":           20,
}

CALENDAR_URLS = [
    "https://nfs.faireconomy.media/ff_calendar_thisweek.json",
    "https://nfs.faireconomy.media/ff_calendar_nextweek.json",
]

OUTPUT_PATH = "data/economic_calendar.json"


def blackout_for_title(title: str) -> int:
    for keyword, mins in BLACKOUT_OVERRIDES.items():
        if keyword.lower() in title.lower():
            return mins
    return DEFAULT_BLACKOUT_MINS


def fetch_week(url: str) -> list:
    req = Request(url, headers={"User-Agent": "ig-trading-bot/1.0"})
    try:
        with urlopen(req, timeout=15) as resp:
            return json.loads(resp.read())
    except URLError as e:
        print(f"  WARNING: failed to fetch {url}: {e}", file=sys.stderr)
        return []


def to_utc_iso(date_str: str) -> Optional[str]:
    """Convert ForexFactory date (ISO with tz offset) to UTC ISO string."""
    for fmt in ("%Y-%m-%dT%H:%M:%S%z", "%Y-%m-%dT%H:%M%z"):
        try:
            dt = datetime.strptime(date_str, fmt)
            return dt.astimezone(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
        except ValueError:
            continue
    return None


def main():
    os.makedirs("data", exist_ok=True)

    all_events = []
    for url in CALENDAR_URLS:
        print(f"Fetching {url} ...")
        raw = fetch_week(url)
        for item in raw:
            impact  = item.get("impact", "")
            country = item.get("country", "").upper()
            title   = item.get("title", "")
            date    = item.get("date", "")

            if impact != "High":
                continue
            if country not in WATCHED_CURRENCIES:
                continue
            if not date:
                continue

            utc_iso = to_utc_iso(date)
            if not utc_iso:
                print(f"  WARNING: could not parse date '{date}' for '{title}'", file=sys.stderr)
                continue

            all_events.append({
                "datetime_utc":  utc_iso,
                "title":         title,
                "country":       country,
                "impact":        impact,
                "blackout_mins": blackout_for_title(title),
            })

    all_events.sort(key=lambda e: e["datetime_utc"])

    output = {
        "fetched_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "events":     all_events,
    }

    with open(OUTPUT_PATH, "w") as f:
        json.dump(output, f, indent=2)

    print(f"Wrote {len(all_events)} high-impact events to {OUTPUT_PATH}")
    for e in all_events:
        print(f"  {e['datetime_utc']}  [{e['country']}] {e['title']} (±{e['blackout_mins']}min)")


if __name__ == "__main__":
    main()
