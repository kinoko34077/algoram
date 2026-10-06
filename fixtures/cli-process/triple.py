#!/usr/bin/env python3

import sys


def main() -> int:
    if len(sys.argv) != 2:
        raise SystemExit("usage: triple.py <integer>")

    value = int(sys.argv[1])
    print(value * 3)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
