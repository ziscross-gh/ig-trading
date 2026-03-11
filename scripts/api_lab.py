#!/usr/bin/env python3
"""
IG API Lab — Standalone Testing Script
--------------------------------------
Test custom payloads, deal sizes, and API behavior without touching the Rust engine.
Usage: python scripts/api_lab.py --epic CS.D.CFIGOLD.CFI.IP --size 2.9
"""

import argparse
import json
import os
import requests
import sys
from dotenv import load_dotenv

# Load environment
load_dotenv()

API_KEY     = os.getenv("IG_API_KEY")
IDENTIFIER  = os.getenv("IG_IDENTIFIER")
PASSWORD    = os.getenv("IG_PASSWORD")
ENVIRONMENT = os.getenv("IG_ENVIRONMENT", "demo").lower()

BASE_URLS = {
    "demo": "https://demo-api.ig.com/gateway/deal",
    "live": "https://api.ig.com/gateway/deal"
}

URL = BASE_URLS.get(ENVIRONMENT, BASE_URLS["demo"])

def login():
    """Authenticate and return (CST, X-SECURITY-TOKEN)"""
    headers = {
        "X-IG-API-KEY": API_KEY,
        "Content-Type": "application/json",
        "Accept": "application/json",
        "VERSION": "2"
    }
    payload = {
        "identifier": IDENTIFIER,
        "password": PASSWORD
    }
    
    print(f"Logging in to {ENVIRONMENT}...")
    resp = requests.post(f"{URL}/session", json=payload, headers=headers)
    if resp.status_code != 200:
        print(f"❌ Login failed ({resp.status_code}): {resp.text}")
        sys.exit(1)
        
    cst = resp.headers.get("CST")
    xst = resp.headers.get("X-SECURITY-TOKEN")
    print("✅ Authenticated successfully.")
    return cst, xst

def test_trade(cst, xst, epic, size, direction="BUY"):
    """Send a custom trade request"""
    headers = {
        "X-IG-API-KEY": API_KEY,
        "CST": cst,
        "X-SECURITY-TOKEN": xst,
        "Content-Type": "application/json",
        "Accept": "application/json",
        "VERSION": "2"
    }
    
    # Payload matches the stable Rust engine but allows size overrides
    payload = {
        "epic": epic,
        "direction": direction,
        "size": size,
        "orderType": "MARKET",
        "expiry": "-",
        "guaranteedStop": False, # Safer for sizing tests
        "forceOpen": True,
        "currencyCode": "SGD" if "GOLD" in epic or "CFI" in epic else "USD"
    }
    
    print(f"Injecting trade: {direction} {size} units of {epic}...")
    resp = requests.post(f"{URL}/positions/otc", json=payload, headers=headers)
    
    if resp.status_code == 200:
        data = resp.json()
        ref = data.get("dealReference")
        print(f"✅ Trade SUBMITTED. Reference: {ref}")
        print("Note: Check the platform or Telegram close-alerts for final acceptance.")
    else:
        print(f"❌ Trade REJECTED by API ({resp.status_code})")
        try:
            err = resp.json()
            print(f"Error Details: {json.dumps(err, indent=2)}")
        except:
            print(f"Raw Error: {resp.text}")

def main():
    parser = argparse.ArgumentParser(description="Standalone IG API Testing Lab")
    parser.add_argument("--epic", default="CS.D.CFIGOLD.CFI.IP", help="Instrument EPIC")
    parser.add_argument("--size", type=float, default=3.0, help="Deal size to test")
    parser.add_argument("--dir", default="BUY", choices=["BUY", "SELL"], help="Direction")
    args = parser.parse_args()

    print("="*60)
    print(" 🧪 IG API LAB — Standalone Injector")
    print("="*60)
    
    if not API_KEY or not IDENTIFIER:
        print("❌ Error: Missing credentials in .env file.")
        sys.exit(1)

    cst, xst = login()
    test_trade(cst, xst, args.epic, args.size, args.dir)

if __name__ == "__main__":
    main()
