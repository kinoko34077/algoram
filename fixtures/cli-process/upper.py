#!/usr/bin/env python3

import sys


def main() -> int:
    if len(sys.argv) != 2:
        raise SystemExit("usage: upper.py <text>")

    value = sys.argv[1]
    if value == "__fail__":
        print("cli failure: requested failure", file=sys.stderr)
        return 17

    print(value.upper())
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
