#!/usr/bin/env python3
"""
sentiment_agent.py — Gold News Sentiment Signal
-------------------------------------------------
Polls free RSS news feeds every 15 minutes, scores Gold-relevant headlines
using one of three backends, and writes the result to:

    data/gold_sentiment_latest.json

The Rust engine reads this file at each bar-close and injects a sentiment
signal into the Gold ensemble if the score exceeds ±0.55.

Scoring backends (auto-detected, or forced with --mode):
  1. keyword  — regex keyword matching. No LLM, zero cost, works always.
  2. ollama   — local Ollama (llama3). Set OLLAMA_HOST if not localhost.
  3. claude   — Anthropic API (claude-haiku). Requires ANTHROPIC_API_KEY.

Usage:
    # Daemon mode (runs forever, polls every 15 minutes)
    python scripts/sentiment_agent.py

    # Cron mode (run once and exit — schedule with crontab every 15min)
    python scripts/sentiment_agent.py --once

    # Force a specific scoring mode
    python scripts/sentiment_agent.py --mode keyword
    python scripts/sentiment_agent.py --mode ollama
    python scripts/sentiment_agent.py --mode claude

    # Custom poll interval
    python scripts/sentiment_agent.py --interval 600  # 10 minutes

Cron example:
    */15 * * * * cd /path/to/ig-trading && python scripts/sentiment_agent.py --once >> logs/sentiment.log 2>&1

Environment variables:
    ANTHROPIC_API_KEY  — enables Claude API scoring
    OLLAMA_HOST        — Ollama host (default: http://localhost:11434)
    OLLAMA_MODEL       — Ollama model (default: llama3)
"""

import argparse
import json
import os
import time
import xml.etree.ElementTree as ET
import urllib.error
import urllib.request
from datetime import datetime, timezone

# ── Paths ─────────────────────────────────────────────────────────────────────

_HERE     = os.path.dirname(os.path.abspath(__file__))
_ROOT     = os.path.join(_HERE, "..")
DATA_DIR  = os.path.join(_ROOT, "data")
OUT_FILE  = os.path.join(DATA_DIR, "gold_sentiment_latest.json")

# ── News sources ──────────────────────────────────────────────────────────────

NEWS_SOURCES = [
    ("Reuters Business",    "https://feeds.reuters.com/reuters/businessNews"),
    ("Yahoo Finance Gold",  "https://finance.yahoo.com/rss/headline?s=GC=F"),
    ("Kitco News",          "https://www.kitco.com/rss/"),
]

# Keywords that trigger filtering headlines as Gold-relevant
GOLD_FILTER_WORDS = {
    "gold", "xau", "bullion", "precious metal",
    "inflation", "cpi", "fed", "federal reserve", "fomc",
    "dollar", "usd", "interest rate", "bond yield", "treasury",
    "geopolit", "war", "conflict", "recession", "safe haven",
    "commodity", "silver", "copper",  # often co-move with gold
}

# ── Keyword scoring ───────────────────────────────────────────────────────────

BULLISH_TERMS = {
    "inflation":      0.60, "cpi rises":        0.65, "cpi hot":        0.65,
    "geopolitical":   0.70, "war":              0.60, "conflict":       0.55,
    "recession":      0.55, "rate cut":         0.80, "rate cuts":      0.80,
    "fed cut":        0.80, "dovish":           0.65, "dollar falls":   0.70,
    "dollar slips":   0.65, "dollar weakens":   0.65, "usd falls":      0.65,
    "safe haven":     0.80, "gold rally":       0.90, "gold rises":     0.80,
    "gold jumps":     0.80, "gold surges":      0.90, "gold soars":     0.90,
    "gold hits":      0.70, "record high":      0.80, "all-time high":  0.80,
    "bank failure":   0.70, "crisis":           0.60, "uncertainty":    0.45,
    "risk off":       0.65, "flight to safety": 0.80, "debt ceiling":   0.55,
    "gold demand":    0.60, "central bank buy": 0.70, "de-dollarization": 0.65,
}

BEARISH_TERMS = {
    "rate hike":      0.80, "rate hikes":       0.80, "hawkish":        0.65,
    "dollar rises":   0.70, "dollar gains":     0.65, "dollar strengthens": 0.70,
    "strong dollar":  0.70, "usd rises":        0.65, "usd gains":      0.65,
    "gold falls":     0.90, "gold drops":       0.90, "gold tumbles":   0.90,
    "gold slides":    0.80, "gold slips":       0.75, "gold selloff":   0.90,
    "gold retreats":  0.70, "gold loses":       0.70, "gold declines":  0.70,
    "risk on":        0.55, "equities rally":   0.45, "stocks surge":   0.40,
    "fed hike":       0.80, "tightening":       0.55, "yields rise":    0.65,
    "strong jobs":    0.55, "nfp beats":        0.60, "jobs beat":      0.55,
}

def keyword_score(headlines: list[str]) -> dict:
    """Keyword-based sentiment scoring. Always available, no LLM needed."""
    text = " ".join(headlines).lower()
    bull_score = 0.0
    bear_score = 0.0
    drivers = []

    for term, weight in BULLISH_TERMS.items():
        if term in text:
            bull_score += weight
            drivers.append(f"+{term}")

    for term, weight in BEARISH_TERMS.items():
        if term in text:
            bear_score += weight
            drivers.append(f"-{term}")

    total = bull_score + bear_score
    if total < 0.01:
        return {"score": 0.0, "confidence": 0.0, "key_drivers": []}

    # Normalise to -1.0 … +1.0
    raw_score = (bull_score - bear_score) / max(total, 1.0)
    raw_score = max(-1.0, min(1.0, raw_score))

    # Confidence = how many signals fired relative to max possible
    confidence = min(1.0, total / 5.0)

    return {
        "score":       round(raw_score, 3),
        "confidence":  round(confidence, 3),
        "key_drivers": sorted(drivers, key=lambda d: d[1:])[:8],
    }

# ── LLM scoring prompt ────────────────────────────────────────────────────────

LLM_PROMPT = """\
You are a Gold (XAUUSD) trading sentiment analyst.
Analyse these Gold-relevant headlines and return a JSON sentiment score.

Headlines:
{headlines}

Return ONLY valid JSON, no prose:
{{
  "score": <float, -1.0 (very bearish) to +1.0 (very bullish)>,
  "confidence": <float, 0.0 to 1.0>,
  "key_drivers": ["<driver1>", "<driver2>"]
}}

Key Gold drivers:
  Bullish: inflation, geopolitical risk, USD weakness, rate cuts, bank stress
  Bearish: USD strength, rate hikes, rising yields, risk-on sentiment, NFP beat
"""

# ── Ollama scoring ────────────────────────────────────────────────────────────

def score_with_ollama(headlines: list[str]) -> dict | None:
    host  = os.getenv("OLLAMA_HOST", "http://localhost:11434")
    model = os.getenv("OLLAMA_MODEL", "llama3")
    prompt = LLM_PROMPT.format(headlines="\n".join(f"- {h}" for h in headlines[:10]))

    payload = json.dumps({
        "model":  model,
        "prompt": prompt,
        "stream": False,
        "format": "json",
    }).encode()

    try:
        req = urllib.request.Request(
            f"{host}/api/generate",
            data=payload,
            headers={"Content-Type": "application/json"},
        )
        resp = urllib.request.urlopen(req, timeout=30)
        data = json.loads(resp.read())
        result = json.loads(data["response"])
        # Clamp values
        result["score"]      = max(-1.0, min(1.0, float(result.get("score", 0.0))))
        result["confidence"] = max(0.0,  min(1.0, float(result.get("confidence", 0.5))))
        result["key_drivers"] = result.get("key_drivers", [])[:8]
        return result
    except Exception as e:
        print(f"  ⚠️  Ollama error ({host}): {e}")
        return None

# ── Claude API scoring ────────────────────────────────────────────────────────

def score_with_claude(headlines: list[str]) -> dict | None:
    api_key = os.getenv("ANTHROPIC_API_KEY", "")
    if not api_key:
        return None

    model  = os.getenv("ANTHROPIC_MODEL", "claude-haiku-4-5")
    prompt = LLM_PROMPT.format(headlines="\n".join(f"- {h}" for h in headlines[:10]))

    payload = json.dumps({
        "model":      model,
        "max_tokens": 256,
        "messages":   [{"role": "user", "content": prompt}],
    }).encode()

    try:
        req = urllib.request.Request(
            "https://api.anthropic.com/v1/messages",
            data=payload,
            headers={
                "Content-Type":      "application/json",
                "x-api-key":         api_key,
                "anthropic-version": "2023-06-01",
            },
        )
        resp   = urllib.request.urlopen(req, timeout=15)
        data   = json.loads(resp.read())
        text   = data["content"][0]["text"]
        result = json.loads(text)
        result["score"]       = max(-1.0, min(1.0, float(result.get("score", 0.0))))
        result["confidence"]  = max(0.0,  min(1.0, float(result.get("confidence", 0.5))))
        result["key_drivers"] = result.get("key_drivers", [])[:8]
        return result
    except Exception as e:
        print(f"  ⚠️  Claude API error: {e}")
        return None

# ── News fetcher ──────────────────────────────────────────────────────────────

def fetch_headlines() -> list[str]:
    """Fetch and filter Gold-relevant headlines from RSS feeds."""
    all_headlines = []

    for name, url in NEWS_SOURCES:
        try:
            req  = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0"})
            resp = urllib.request.urlopen(req, timeout=10)
            xml  = resp.read()
            root = ET.fromstring(xml)

            for item in root.findall(".//item/title")[:8]:
                if item.text:
                    all_headlines.append(item.text.strip())
        except Exception as e:
            print(f"  ⚠️  {name}: {e}")

    # Filter to Gold-relevant headlines (case-insensitive word match)
    relevant = [
        h for h in all_headlines
        if any(kw in h.lower() for kw in GOLD_FILTER_WORDS)
    ]

    # If nothing matched the filter, use all headlines (broad signal)
    return relevant if relevant else all_headlines[:15]

# ── Auto-detect scoring mode ──────────────────────────────────────────────────

def detect_mode() -> str:
    """Return the best available scoring mode."""
    if os.getenv("ANTHROPIC_API_KEY"):
        return "claude"
    # Quick Ollama probe
    try:
        host = os.getenv("OLLAMA_HOST", "http://localhost:11434")
        urllib.request.urlopen(f"{host}/api/tags", timeout=2)
        return "ollama"
    except Exception:
        pass
    return "keyword"

# ── Main poll ─────────────────────────────────────────────────────────────────

def poll_once(mode: str) -> dict:
    """Fetch headlines, score, write output file. Returns the result dict."""
    now_utc = datetime.now(tz=timezone.utc)
    print(f"\n[{now_utc.strftime('%Y-%m-%dT%H:%M:%SZ')}] Polling Gold sentiment (mode={mode})...")

    headlines = fetch_headlines()
    if not headlines:
        print("  ⚠️  No headlines retrieved — writing neutral signal")
        result = {"score": 0.0, "confidence": 0.0, "key_drivers": []}
    else:
        print(f"  {len(headlines)} relevant headline(s):")
        for h in headlines[:5]:
            print(f"    • {h}")
        if len(headlines) > 5:
            print(f"    … (+{len(headlines)-5} more)")

        if mode == "claude":
            result = score_with_claude(headlines) or keyword_score(headlines)
            if not score_with_claude(headlines):
                print("  ↩  Claude failed — falling back to keyword scoring")
        elif mode == "ollama":
            result = score_with_ollama(headlines) or keyword_score(headlines)
            if not score_with_ollama(headlines):
                print("  ↩  Ollama failed — falling back to keyword scoring")
        else:
            result = keyword_score(headlines)

    output = {
        "timestamp":      int(now_utc.timestamp()),
        "score":          result["score"],
        "confidence":     result["confidence"],
        "mode":           mode,
        "headline_count": len(headlines),
        "key_drivers":    result.get("key_drivers", []),
    }

    # Emoji legend for quick visual
    score = result["score"]
    if score >= 0.55:
        label = "🟢 Bullish"
    elif score <= -0.55:
        label = "🔴 Bearish"
    else:
        label = "⚪ Neutral"

    print(f"  → Score: {score:+.3f}  Confidence: {result['confidence']:.2f}  {label}")
    if result.get("key_drivers"):
        print(f"  → Drivers: {', '.join(result['key_drivers'][:4])}")

    os.makedirs(DATA_DIR, exist_ok=True)
    with open(OUT_FILE, "w") as f:
        json.dump(output, f, indent=2)
    print(f"  → Written → {OUT_FILE}")

    return output

# ── Entry point ───────────────────────────────────────────────────────────────

def main():
    parser = argparse.ArgumentParser(description="Gold News Sentiment Agent")
    parser.add_argument(
        "--mode",
        choices=["auto", "keyword", "ollama", "claude"],
        default="auto",
        help="Scoring backend (default: auto-detect)",
    )
    parser.add_argument(
        "--once",
        action="store_true",
        help="Run one poll cycle and exit (for cron use)",
    )
    parser.add_argument(
        "--interval",
        type=int,
        default=900,
        help="Poll interval in seconds for daemon mode (default: 900 = 15min)",
    )
    args = parser.parse_args()

    mode = args.mode if args.mode != "auto" else detect_mode()
    print(f"Gold Sentiment Agent — mode={mode}, once={args.once}, interval={args.interval}s")

    if args.once:
        poll_once(mode)
        return

    # Daemon mode
    print(f"Running as daemon (Ctrl+C to stop)...")
    while True:
        try:
            poll_once(mode)
        except KeyboardInterrupt:
            print("\nStopped.")
            break
        except Exception as e:
            print(f"  ❌  Unexpected error: {e} — retrying next cycle")

        time.sleep(args.interval)


if __name__ == "__main__":
    main()
