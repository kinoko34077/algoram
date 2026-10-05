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
    function = library.algoram_checked_double
    function.argtypes = [ctypes.c_int32]
    function.restype = ctypes.c_int32

    result = function(value)
    if result < 0:
        print(
            f"native failure: algoram_checked_double({value}) returned {result}",
            file=sys.stderr,
        )
        return 23

    print(result)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
