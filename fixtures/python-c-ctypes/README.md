# Python ctypes → C ABI fixture

This fixture is the real low-level bridge used by Phase 1 S4.

Registered route:

```text
python:ctypes:c_int
  -- python-ctypes-c-abi-int32 / copy+sync -->
c:abi:int32
  -- c-abi-call-algoram-double / copy+sync -->
c:function:algoram_double:int32
```

The fixture deliberately uses existing interoperability rather than an Algoram-specific calling convention.

CI builds `bridge.c` as a Linux shared library and calls `algoram_double` through Python's standard-library `ctypes`.

This is a bounded route proof, not the general Algoram Execution Plan/runtime.
