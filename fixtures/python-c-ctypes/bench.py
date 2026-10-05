#!/usr/bin/env python3

import ctypes
import json
import pathlib
import platform
import sys
import time


def main() -> int:
    if len(sys.argv) != 3:
        raise SystemExit("usage: bench.py <shared-library> <iterations>")

    library_path = pathlib.Path(sys.argv[1]).resolve()
    iterations = int(sys.argv[2])
    if iterations <= 0:
        raise SystemExit("iterations must be positive")

    library = ctypes.CDLL(str(library_path))
    function = library.algoram_double
    function.argtypes = [ctypes.c_int]
    function.restype = ctypes.c_int

    for value in range(1000):
        if function(value) != value * 2:
            raise SystemExit("ctypes warmup returned an unexpected value")

    start = time.perf_counter_ns()
    checksum = 0
    for value in range(iterations):
        checksum += function(value & 0x3FFF)
    elapsed_ns = time.perf_counter_ns() - start

    print(
        json.dumps(
            {
                "kind": "metric",
                "metric": "python_ctypes_c_abi_call_loop",
                "iterations": iterations,
                "elapsed_ns": elapsed_ns,
                "ns_per_call": elapsed_ns / iterations,
                "checksum": checksum,
                "python": platform.python_version(),
                "platform": platform.platform(),
            },
            separators=(",", ":"),
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
