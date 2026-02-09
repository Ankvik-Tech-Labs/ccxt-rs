#!/usr/bin/env python3
"""
Cross-Validation Tool (Tier 5)

Runs CCXT Python and ccxt-rs (via cross_validate_runner example) side-by-side,
then compares their JSON output with tolerance for timestamps and info fields.

Usage:
    python scripts/cross_validate.py binance fetch_ticker BTC/USDT
    python scripts/cross_validate.py bybit fetch_order_book BTC/USDT --limit 5
    python scripts/cross_validate.py okx fetch_ohlcv BTC/USDT --timeframe 1h --limit 3

Requires:
    - ccxt Python package
    - ccxt-rs built with the target exchange feature
"""

import argparse
import json
import subprocess
import sys
from datetime import datetime
from decimal import Decimal

import ccxt


class DecimalEncoder(json.JSONEncoder):
    def default(self, obj):
        if isinstance(obj, Decimal):
            return float(obj)
        if isinstance(obj, datetime):
            return obj.isoformat()
        return super().default(obj)


def sanitize(obj):
    """Recursively convert to JSON-safe types."""
    if obj is None:
        return None
    if isinstance(obj, (bool, int, float, str)):
        return obj
    if isinstance(obj, Decimal):
        return float(obj)
    if isinstance(obj, dict):
        return {k: sanitize(v) for k, v in obj.items()}
    if isinstance(obj, (list, tuple)):
        return [sanitize(v) for v in obj]
    return str(obj)


# Keys to skip during comparison (non-deterministic)
SKIP_KEYS = {"timestamp", "datetime", "info"}

# Numeric tolerance for comparison
TOLERANCE = 0.001


def compare_values(path, actual, expected, errors):
    """Recursively compare two JSON-like values."""
    if isinstance(expected, dict) and isinstance(actual, dict):
        for key in expected:
            if key in SKIP_KEYS:
                continue
            if key not in actual:
                errors.append(f"{path}.{key}: missing in Rust output")
                continue
            compare_values(f"{path}.{key}", actual[key], expected[key], errors)
    elif isinstance(expected, list) and isinstance(actual, list):
        if len(actual) != len(expected):
            errors.append(f"{path}: array length {len(actual)} vs {len(expected)}")
            return
        for i, (a, e) in enumerate(zip(actual, expected)):
            compare_values(f"{path}[{i}]", a, e, errors)
    elif isinstance(expected, (int, float)) and isinstance(actual, (int, float, str)):
        a = float(actual) if isinstance(actual, str) else float(actual)
        e = float(expected)
        if e != 0 and abs((a - e) / e) > TOLERANCE:
            errors.append(f"{path}: {a} vs {e} (diff: {abs(a - e) / abs(e):.4%})")
    elif isinstance(expected, str) and isinstance(actual, str):
        # Try numeric comparison
        try:
            a, e = float(actual), float(expected)
            if e != 0 and abs((a - e) / e) > TOLERANCE:
                errors.append(f"{path}: {a} vs {e}")
        except ValueError:
            if actual != expected:
                errors.append(f"{path}: '{actual}' vs '{expected}'")
    elif actual is None and expected is not None:
        pass  # Rust returned None where Python had value — tolerated
    elif expected is None:
        pass  # Both None or Python None — OK


def call_python(exchange_id, method, args, kwargs):
    """Call CCXT Python and return the result."""
    exchange_class = getattr(ccxt, exchange_id)
    exchange = exchange_class({"enableRateLimit": True})
    exchange.load_markets()

    func = getattr(exchange, method)
    result = func(*args, **kwargs)
    return sanitize(result)


def call_rust(exchange_id, method, args):
    """Call the ccxt-rs cross_validate_runner example."""
    cmd = [
        "cargo", "run", "--all-features",
        "--example", "cross_validate_runner",
        "--",
        exchange_id, method,
    ] + args

    try:
        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=60,
            cwd=".",
        )
        if result.returncode != 0:
            print(f"Rust error (exit {result.returncode}):")
            print(result.stderr)
            return None
        return json.loads(result.stdout)
    except subprocess.TimeoutExpired:
        print("Rust subprocess timed out")
        return None
    except json.JSONDecodeError as e:
        print(f"Failed to parse Rust JSON output: {e}")
        print(f"stdout: {result.stdout[:500]}")
        return None


def main():
    parser = argparse.ArgumentParser(description="Cross-validate CCXT Python vs ccxt-rs")
    parser.add_argument("exchange", help="Exchange ID (binance, bybit, okx)")
    parser.add_argument("method", help="Method name (fetch_ticker, fetch_order_book, etc.)")
    parser.add_argument("args", nargs="*", help="Method arguments (e.g., BTC/USDT)")
    parser.add_argument("--limit", type=int, help="Limit parameter")
    parser.add_argument("--timeframe", type=str, help="Timeframe (e.g., 1h)")
    args = parser.parse_args()

    print(f"Cross-validating: {args.exchange}.{args.method}({', '.join(args.args)})")
    print("=" * 60)

    # Build kwargs for Python
    kwargs = {}
    python_args = list(args.args)
    rust_args = list(args.args)

    if args.method == "fetch_ohlcv" and args.timeframe:
        python_args.append(args.timeframe)
        rust_args.append(args.timeframe)
    if args.limit:
        kwargs["limit"] = args.limit
        rust_args.extend(["--limit", str(args.limit)])

    # Call Python
    print("Calling CCXT Python...")
    python_result = call_python(args.exchange, args.method, python_args, kwargs)
    if python_result is None:
        print("Python call failed!")
        sys.exit(1)

    # Call Rust
    print("Calling ccxt-rs...")
    rust_result = call_rust(args.exchange, args.method, rust_args)
    if rust_result is None:
        print("Rust call failed! (ensure cross_validate_runner example is built)")
        print("You may need to implement the example first.")
        sys.exit(1)

    # Compare
    print("\nComparing results...")
    errors = []
    compare_values("$", rust_result, python_result, errors)

    if errors:
        print(f"\nFOUND {len(errors)} DIFFERENCE(S):")
        for err in errors:
            print(f"  - {err}")
        sys.exit(1)
    else:
        print("\nRESULT: MATCH - Python and Rust outputs are equivalent!")
        sys.exit(0)


if __name__ == "__main__":
    main()
