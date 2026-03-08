#!/usr/bin/env python3
"""
compare_params.py — Auto-apply improved optimizer results to default.toml
-------------------------------------------------------------------------
Reads data/optimize_results.json (written by optimize.py --output),
compares against current ig-engine/config/default.toml, and applies
improvements when the profit_factor gain exceeds IMPROVEMENT_THRESHOLD.

SAFE params (auto-applied):
  - strategies.instrument_overrides.{epic}.adx_range_max
  - strategies.instrument_overrides.{epic}.adx_range_filter

NEVER modified automatically:
  - max_risk_per_trade, max_daily_loss_pct, max_weekly_drawdown_pct
  - any [risk] section parameter

Usage:
    python scripts/compare_params.py              # compare + apply if improved
    python scripts/compare_params.py --dry-run    # show changes without writing
    python scripts/compare_params.py --threshold 0.10  # require 10% PF improvement

Exit codes:
    0  — no changes (params already optimal)
    1  — changes applied to config
    2  — error (missing files, parse failure, etc.)
"""

import argparse
import json
import os
import re
import shutil
import sys
import urllib.parse
import urllib.request
from datetime import datetime, timezone

# ── Paths ─────────────────────────────────────────────────────────────────────

_HERE        = os.path.dirname(os.path.abspath(__file__))
_ROOT        = os.path.join(_HERE, "..")
RESULTS_PATH = os.path.join(_ROOT, "data", "optimize_results.json")
CONFIG_PATH  = os.path.join(_ROOT, "ig-engine", "config", "default.toml")
SNAPSHOT_DIR = os.path.join(_ROOT, "ig-engine", "config", "snapshots")

# ── Instrument → IG epic mapping ──────────────────────────────────────────────

EPIC_MAP = {
    "EURUSD": "CS.D.EURUSD.CSD.IP",
    "USDJPY": "CS.D.USDJPY.CSD.IP",
    "GOLD":   "CS.D.CFIGOLD.CFI.IP",
}

# ── TOML helpers ──────────────────────────────────────────────────────────────

def _section_re(epic: str) -> re.Pattern:
    """Regex that captures the body of [strategies.instrument_overrides."<epic>"]."""
    return re.compile(
        r'(\[strategies\.instrument_overrides\."' + re.escape(epic) + r'"\])'
        r'(.*?)'
        r'(?=\n\[|\Z)',
        re.DOTALL,
    )

def get_current_adx_max(toml_text: str, epic: str) -> float | None:
    m = _section_re(epic).search(toml_text)
    if not m:
        return None
    val = re.search(r'adx_range_max\s*=\s*([0-9.]+)', m.group(2))
    return float(val.group(1)) if val else None

def patch_adx_max(toml_text: str, epic: str, new_val: float) -> tuple[str, bool]:
    """Return (new_toml_text, changed) after updating adx_range_max for epic."""
    def replacer(m):
        header = m.group(1)
        body   = m.group(2)
        new_body = re.sub(
            r'(adx_range_max\s*=\s*)([0-9.]+)',
            lambda bm: f'{bm.group(1)}{new_val:.1f}',
            body,
        )
        return header + new_body

    new_text, n = _section_re(epic).subn(replacer, toml_text)
    if n == 0:
        return toml_text, False
    return new_text, (new_text != toml_text)

# ── Config backup ─────────────────────────────────────────────────────────────

def backup_config() -> str:
    os.makedirs(SNAPSHOT_DIR, exist_ok=True)
    ts  = datetime.now(tz=timezone.utc).strftime("%Y-%m-%d_%H%M%S")
    dst = os.path.join(SNAPSHOT_DIR, f"{ts}.toml")
    shutil.copy2(CONFIG_PATH, dst)
    return dst

# ── Telegram notification ─────────────────────────────────────────────────────

def send_telegram(message: str) -> None:
    token   = os.getenv("TELEGRAM_BOT_TOKEN", "")
    chat_id = os.getenv("TELEGRAM_CHAT_ID", "")
    if not token or not chat_id:
        print("  (TELEGRAM_BOT_TOKEN / TELEGRAM_CHAT_ID not set — skipping Telegram)")
        return
    try:
        data = urllib.parse.urlencode({
            "chat_id":    chat_id,
            "text":       message,
            "parse_mode": "Markdown",
        }).encode()
        urllib.request.urlopen(
            f"https://api.telegram.org/bot{token}/sendMessage",
            data=data,
            timeout=10,
        )
        print("  📱 Telegram notification sent.")
    except Exception as e:
        print(f"  ⚠️  Telegram notification failed: {e}")

# ── Main ──────────────────────────────────────────────────────────────────────

def main() -> int:
    parser = argparse.ArgumentParser(description="Compare optimizer results to current config and auto-apply improvements")
    parser.add_argument("--dry-run",   action="store_true",
                        help="Show changes without writing to disk")
    parser.add_argument("--threshold", type=float, default=0.05,
                        help="Minimum profit-factor improvement ratio to apply (default: 0.05 = 5%%)")
    args = parser.parse_args()

    # ── Load optimizer results ─────────────────────────────────────────────
    if not os.path.exists(RESULTS_PATH):
        print(f"❌  No optimizer results at {RESULTS_PATH}")
        print(f"    Run: python scripts/optimize.py --output {RESULTS_PATH}")
        return 2

    with open(RESULTS_PATH) as f:
        results = json.load(f)

    generated_at = results.get("generated_at", "unknown")
    print(f"\n🔍 compare_params.py — Walk-forward param diff")
    print(f"   Optimizer results from: {generated_at}")
    print(f"   Improvement threshold:  {args.threshold * 100:.0f}%")
    print(f"   Dry run:                {'yes' if args.dry_run else 'no'}")

    # ── Load current config ────────────────────────────────────────────────
    if not os.path.exists(CONFIG_PATH):
        print(f"❌  Config not found: {CONFIG_PATH}")
        return 2

    with open(CONFIG_PATH) as f:
        toml_text = f.read()

    # ── Compare per instrument ─────────────────────────────────────────────
    changes = []

    for name, epic in EPIC_MAP.items():
        inst_results = results.get("results_per_instrument", {}).get(name, [])
        if not inst_results:
            print(f"\n  ⚠️  {name}: no optimizer results — skipping")
            continue

        best = inst_results[0]  # sorted by profit_fac desc (from optimize.py)

        current_adx_max = get_current_adx_max(toml_text, epic)
        if current_adx_max is None:
            print(f"\n  ⚠️  {name}: could not parse adx_range_max from config — skipping")
            continue

        # Find the optimizer result that matches current TOML params
        current_result = next(
            (r for r in inst_results
             if r.get("adx_range_filter") and abs(r.get("adx_max", 0) - current_adx_max) < 0.1),
            None,
        )
        current_pf = current_result["profit_fac"] if current_result else 0.0
        best_pf    = best["profit_fac"]
        best_adx   = best.get("adx_max", current_adx_max)

        improvement = (best_pf - current_pf) / max(current_pf, 0.01)

        print(f"\n  {name}  [{epic}]")
        print(f"    Current: adx_range_max={current_adx_max:.1f}, PF={current_pf:.3f}")
        print(f"    Best:    adx_range_max={best_adx:.1f},  PF={best_pf:.3f}  "
              f"(+{improvement * 100:.1f}%)")

        if abs(best_adx - current_adx_max) < 0.1:
            print(f"    → No change (adx_max already optimal)")
            continue

        if improvement < args.threshold:
            print(f"    → Improvement {improvement * 100:.1f}% < threshold "
                  f"{args.threshold * 100:.0f}% — skip")
            continue

        changes.append({
            "name":           name,
            "epic":           epic,
            "old_adx_max":   current_adx_max,
            "new_adx_max":   best_adx,
            "old_pf":        current_pf,
            "new_pf":        best_pf,
            "improvement":   improvement,
        })
        print(f"    → ✅ Will update adx_range_max  "
              f"{current_adx_max:.1f} → {best_adx:.1f}")

    # ── Summary ───────────────────────────────────────────────────────────
    print()
    if not changes:
        print("✔  No improvements found — config unchanged.")
        return 0

    if args.dry_run:
        print(f"🔍 DRY RUN — {len(changes)} change(s) identified (not written — remove --dry-run to apply)")
        return 0

    # ── Backup and apply ──────────────────────────────────────────────────
    backup_path = backup_config()
    print(f"💾 Config backed up → {backup_path}")

    for c in changes:
        toml_text, patched = patch_adx_max(toml_text, c["epic"], c["new_adx_max"])
        if patched:
            print(f"  ✅ {c['name']}: adx_range_max  "
                  f"{c['old_adx_max']:.1f} → {c['new_adx_max']:.1f}  "
                  f"(PF {c['old_pf']:.3f} → {c['new_pf']:.3f}, "
                  f"+{c['improvement'] * 100:.1f}%)")
        else:
            print(f"  ⚠️  {c['name']}: patch failed — keeping original")

    with open(CONFIG_PATH, "w") as f:
        f.write(toml_text)

    print(f"\n✅ Config updated: {CONFIG_PATH}")

    # ── Build summary string ───────────────────────────────────────────────
    lines = [f"🤖 *Auto Re-optimise Complete*\n_{generated_at}_\n"]
    for c in changes:
        lines.append(
            f"• *{c['name']}*: ADX max "
            f"{c['old_adx_max']:.0f} → {c['new_adx_max']:.0f}  "
            f"(PF +{c['improvement'] * 100:.1f}%)"
        )
    lines.append("\nEngine config hot-reloaded via SIGUSR1.")
    summary = "\n".join(lines)

    print("\nSUMMARY")
    print(summary)

    # Telegram notification
    send_telegram(summary)

    return 1   # signal to shell script: changes were applied


if __name__ == "__main__":
    sys.exit(main())
