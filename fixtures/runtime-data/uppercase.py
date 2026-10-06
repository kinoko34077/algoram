#!/usr/bin/env python3

import sys


def main() -> int:
    if len(sys.argv) != 2:
        raise SystemExit("usage: uppercase.py <text>")

    sys.stdout.write(sys.argv[1].upper())
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
