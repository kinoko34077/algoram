#!/usr/bin/env python3

import ctypes
import pathlib
import sys


def main() -> int:
    if len(sys.argv) != 3:
        raise SystemExit("usage: call.py <shared-library> <integer>")

    library_path = pathlib.Path(sys.argv[1]).resolve()
    value = int(sys.argv[2])

    library = ctypes.CDLL(str(library_path))
    function = library.algoram_double
    function.argtypes = [ctypes.c_int]
    function.restype = ctypes.c_int

    result = function(value)
    print(result)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
