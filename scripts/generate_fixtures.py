#!/usr/bin/env python3
"""
Generate test fixtures by capturing CCXT Python output.

This script calls CCXT Python for each exchange + method + symbol combination,
saving both the raw HTTP response and the parsed unified output as JSON fixtures.

Usage:
    pip install -r scripts/requirements.txt
    python scripts/generate_fixtures.py [--exchange binance] [--method fetch_ticker]

Fixtures are saved to tests/fixtures/{exchange}/.
"""

import argparse
import json
import os
import sys
import traceback
from datetime import datetime
from decimal import Decimal
from pathlib import Path

import ccxt


class DecimalEncoder(json.JSONEncoder):
    """JSON encoder that handles Decimal types."""

    def default(self, obj):
        if isinstance(obj, Decimal):
            return float(obj)
        if isinstance(obj, datetime):
            return obj.isoformat()
        return super().default(obj)


# Default skip keys for non-deterministic fields
DEFAULT_SKIP_KEYS = ["timestamp", "datetime", "info"]

# Default tolerance for decimal comparisons
DEFAULT_TOLERANCE = {"percentage": 0.001, "average": 0.001}

# Methods to capture per exchange with their arguments
METHODS = {
    "fetch_ticker_BTC_USDT": {
        "method": "fetch_ticker",
        "args": ["BTC/USDT"],
        "kwargs": {},
    },
    "fetch_ticker_ETH_USDT": {
        "method": "fetch_ticker",
        "args": ["ETH/USDT"],
        "kwargs": {},
    },
    "fetch_order_book_BTC_USDT": {
        "method": "fetch_order_book",
        "args": ["BTC/USDT"],
        "kwargs": {"limit": 5},
    },
    "fetch_trades_BTC_USDT": {
        "method": "fetch_trades",
        "args": ["BTC/USDT"],
        "kwargs": {"limit": 5},
    },
    "fetch_ohlcv_BTC_USDT": {
        "method": "fetch_ohlcv",
        "args": ["BTC/USDT", "1h"],
        "kwargs": {"limit": 3},
    },
    "fetch_markets": {
        "method": "fetch_markets",
        "args": [],
        "kwargs": {},
        "post_process": "first_5",
    },
    "fetch_tickers_BTC_ETH": {
        "method": "fetch_tickers",
        "args": [["BTC/USDT", "ETH/USDT"]],
        "kwargs": {},
    },
}

# Exchanges to capture
EXCHANGES = ["binance", "bybit", "okx"]


def get_exchange_instance(exchange_id):
    """Create a CCXT exchange instance."""
    exchange_class = getattr(ccxt, exchange_id)
    return exchange_class({"enableRateLimit": True})


def sanitize_for_json(obj):
    """Recursively convert an object to JSON-serializable form."""
    if obj is None:
        return None
    if isinstance(obj, (bool, int, float, str)):
        return obj
    if isinstance(obj, Decimal):
        return float(obj)
    if isinstance(obj, dict):
        return {k: sanitize_for_json(v) for k, v in obj.items()}
    if isinstance(obj, (list, tuple)):
        return [sanitize_for_json(v) for v in obj]
    if isinstance(obj, datetime):
        return obj.isoformat()
    # Fallback: convert to string
    return str(obj)


def capture_fixture(exchange, method_name, args, kwargs, post_process=None):
    """
    Call a CCXT Python method and capture the result.

    We capture the parsed unified output from CCXT Python directly.
    For the http_response, we use the exchange's last_http_response if available,
    or re-derive it from the raw info field in the parsed response.
    """
    method = getattr(exchange, method_name)

    try:
        result = method(*args, **kwargs)
    except Exception as e:
        print(f"    ERROR calling {method_name}: {e}")
        traceback.print_exc()
        return None

    # Post-process if needed
    if post_process == "first_5" and isinstance(result, list):
        result = result[:5]

    # Extract raw http_response from the result's info field if available
    http_response = None
    if isinstance(result, dict) and "info" in result:
        http_response = result.get("info")
    elif isinstance(result, list) and len(result) > 0:
        if isinstance(result[0], dict) and "info" in result[0]:
            # For list results, collect all info fields
            http_response = [item.get("info") for item in result if isinstance(item, dict)]
        else:
            # For OHLCV-style array results, the raw data is the result itself
            http_response = result

    # Also try to get the raw HTTP response from exchange attributes
    if hasattr(exchange, "last_json_response") and exchange.last_json_response:
        http_response = exchange.last_json_response
    elif hasattr(exchange, "last_http_response") and exchange.last_http_response:
        try:
            http_response = json.loads(exchange.last_http_response)
        except (json.JSONDecodeError, TypeError):
            pass

    return {
        "http_response": sanitize_for_json(http_response),
        "parsed_response": sanitize_for_json(result),
    }


def generate_fixtures(exchange_ids=None, method_filter=None):
    """Generate fixtures for specified exchanges and methods."""
    if exchange_ids is None:
        exchange_ids = EXCHANGES

    fixtures_dir = Path(__file__).parent.parent / "tests" / "fixtures"

    for exchange_id in exchange_ids:
        print(f"\n{'='*60}")
        print(f"Generating fixtures for {exchange_id.upper()}")
        print(f"{'='*60}")

        try:
            exchange = get_exchange_instance(exchange_id)
            # Load markets first (required for most methods)
            print(f"  Loading markets...")
            exchange.load_markets()
        except Exception as e:
            print(f"  ERROR initializing {exchange_id}: {e}")
            continue

        exchange_dir = fixtures_dir / exchange_id
        exchange_dir.mkdir(parents=True, exist_ok=True)

        for fixture_name, config in METHODS.items():
            method_name = config["method"]

            if method_filter and method_filter not in fixture_name:
                continue

            print(f"  Capturing {fixture_name}...")

            result = capture_fixture(
                exchange,
                method_name,
                config["args"],
                config.get("kwargs", {}),
                config.get("post_process"),
            )

            if result is None:
                print(f"    SKIPPED (error)")
                continue

            fixture = {
                "exchange": exchange_id,
                "method": method_name,
                "args": config["args"],
                "http_response": result["http_response"],
                "parsed_response": result["parsed_response"],
                "skip_keys": DEFAULT_SKIP_KEYS,
                "tolerance": DEFAULT_TOLERANCE,
            }

            fixture_path = exchange_dir / f"{fixture_name}.json"
            with open(fixture_path, "w") as f:
                json.dump(fixture, f, indent=2, cls=DecimalEncoder)

            # Print summary
            parsed = result["parsed_response"]
            if isinstance(parsed, list):
                print(f"    OK ({len(parsed)} items)")
            elif isinstance(parsed, dict):
                keys = list(parsed.keys())[:5]
                print(f"    OK (keys: {keys}...)")
            else:
                print(f"    OK")

        # Clean up
        if hasattr(exchange, 'close'):
            try:
                exchange.close()
            except Exception:
                pass

    print(f"\n{'='*60}")
    print(f"Fixture generation complete!")
    print(f"Fixtures saved to: {fixtures_dir}")
    print(f"{'='*60}")


def main():
    parser = argparse.ArgumentParser(description="Generate CCXT test fixtures")
    parser.add_argument(
        "--exchange",
        "-e",
        nargs="+",
        choices=EXCHANGES + ["all"],
        default=["all"],
        help="Exchanges to generate fixtures for (default: all)",
    )
    parser.add_argument(
        "--method",
        "-m",
        type=str,
        default=None,
        help="Filter methods by name substring (e.g., 'fetch_ticker')",
    )
    args = parser.parse_args()

    exchange_ids = EXCHANGES if "all" in args.exchange else args.exchange

    generate_fixtures(exchange_ids=exchange_ids, method_filter=args.method)


if __name__ == "__main__":
    main()
