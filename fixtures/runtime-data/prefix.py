#!/usr/bin/env python3

import sys


def main() -> int:
    if len(sys.argv) != 2:
        raise SystemExit("usage: prefix.py <text>")

    sys.stdout.write(f"RESULT:{sys.argv[1]}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
