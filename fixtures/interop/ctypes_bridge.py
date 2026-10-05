from __future__ import annotations

import ctypes
import pathlib
import sys


def main() -> int:
    if len(sys.argv) != 2:
        raise SystemExit("usage: ctypes_bridge.py <shared-library>")

    library_path = pathlib.Path(sys.argv[1]).resolve()
    library = ctypes.CDLL(str(library_path))

    function = library.algoram_double
    function.argtypes = [ctypes.c_int32]
    function.restype = ctypes.c_int32

    actual = function(21)
    expected = 42

    if actual != expected:
        raise SystemExit(f"ctypes/C ABI fixture failed: expected {expected}, got {actual}")

    print(f"ctypes/C ABI fixture OK: algoram_double(21)={actual}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
