import phase1RouteGraphData from "./generated/phase1-route.json";
import phase2RustRouteGraphData from "./generated/phase2-rust-route.json";
import type {
  AlgoramBlock,
  AlgoramConnection,
  AlgoramGraph,
  AlgoramPort,
  ReferenceBundle,
  SourceAnchor,
  SourceArtifact,
} from "./algoram";
import { jaJP } from "./locales/ja-JP";

const pythonSource = `#!/usr/bin/env python3

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
`;

const cSource = `#include <stdint.h>

#if defined(_WIN32)
#define ALGORAM_EXPORT __declspec(dllexport)
#else
#define ALGORAM_EXPORT __attribute__((visibility("default")))
#endif

ALGORAM_EXPORT int32_t algoram_checked_double(int32_t value) {
    if (value < 0) {
        return -1;
    }
    return value * 2;
}
`;


const rustPythonSource = `#!/usr/bin/env python3

import ctypes
import pathlib
import sys


def main() -> int:
    if len(sys.argv) != 3:
        raise SystemExit("usage: call.py <shared-library> <integer>")

    library_path = pathlib.Path(sys.argv[1]).resolve()
    value = int(sys.argv[2])

    library = ctypes.CDLL(str(library_path))
    function = library.algoram_checked_triple
    function.argtypes = [ctypes.c_int32]
    function.restype = ctypes.c_int32

    result = function(value)
    if result < 0:
        print(
            f"native failure: algoram_checked_triple({value}) returned {result}",
            file=sys.stderr,
        )
        return 23

    print(result)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
`;

const rustSource = `#[no_mangle]
pub extern "C" fn algoram_checked_triple(value: i32) -> i32 {
    if value < 0 {
        -1
    } else {
        value * 3
    }
}
`;

const pythonArtifact: SourceArtifact = {
  id: "artifact:phase1-python",
  origin: "fixtures/execution-plan/call.py",
  language: "python",
  revision: "accepted-fixture",
};

const cArtifact: SourceArtifact = {
  id: "artifact:phase1-c",
  origin: "fixtures/execution-plan/bridge.c",
  language: "c",
  revision: "accepted-fixture",
};


const rustPythonArtifact: SourceArtifact = {
  id: "artifact:phase2-python-rust",
  origin: "fixtures/python-rust-cabi/call.py",
  language: "python",
  revision: "accepted-fixture",
};

const rustArtifact: SourceArtifact = {
  id: "artifact:python-rust-cabi",
  origin: "fixtures/python-rust-cabi/bridge.rs",
  language: "rust",
  revision: "accepted-fixture",
};

const encoder = new TextEncoder();

function byteOffset(source: string, charOffset: number): number {
  return encoder.encode(source.slice(0, charOffset)).length;
}

function anchorWhole(artifact: SourceArtifact, source: string): SourceAnchor {
  return {
    artifact_id: artifact.id,
    start_byte: 0,
    end_byte: encoder.encode(source).length,
  };
}

function anchorFragment(
  artifact: SourceArtifact,
  source: string,
  fragment: string,
  semanticKey?: string,
): SourceAnchor {
  const startChar = source.indexOf(fragment);
  if (startChar < 0) {
    throw new Error(`Fixture fragment not found: ${fragment}`);
  }

  const anchor: SourceAnchor = {
    artifact_id: artifact.id,
    start_byte: byteOffset(source, startChar),
    end_byte: byteOffset(source, startChar + fragment.length),
  };

  if (semanticKey) {
    anchor.semantic_key = semanticKey;
  }

  return anchor;
}

function port(
  id: string,
  direction: "in" | "out",
  channel: "flow" | "data",
  contract?: string,
): AlgoramPort {
  return {
    id,
    direction,
    channel,
    ...(contract ? { contract } : {}),
  };
}

function connection(
  id: string,
  sourceBlock: string,
  sourcePort: string,
  targetBlock: string,
  targetPort: string,
): AlgoramConnection {
  return {
    id,
    source: {
      block_id: sourceBlock,
      port_id: sourcePort,
    },
    target: {
      block_id: targetBlock,
      port_id: targetPort,
    },
  };
}

const rootGraphId = "graph:phase1:e2e-root";
const internalGraphId = "graph:composite:python-c";
const routeGraph = {
  ...(phase1RouteGraphData as AlgoramGraph),
  label: `${jaJP.demo.interopRoute}: python:ctypes:c_int → c:function:algoram_checked_double:int32`,
} as AlgoramGraph;
const routeGraphId = routeGraph.id;
const rustInternalGraphId = "graph:phase2:python-rust-cabi";
const rustRouteGraph = {
  ...(phase2RustRouteGraphData as AlgoramGraph),
  label: `${jaJP.demo.interopRoute}: python:ctypes:c_int → rust:extern-c:function:algoram_checked_triple:int32`,
} as AlgoramGraph;
const rustRouteGraphId = rustRouteGraph.id;

const compositeBlock: AlgoramBlock = {
  id: "block:phase1-composite",
  label: jaJP.demo.graphPythonC,
  ports: [
    port("value", "in", "data", "python:ctypes:c_int"),
    port("result", "out", "data", "c:abi:int32"),
  ],
  internal_graph_ref: internalGraphId,
  definition_ref: "definition:python-c-double",
  extensions: {
    composite: {
      definition_id: "definition:python-c-double",
      internal_graph_id: internalGraphId,
    },
  },
};


const rustCompositeBlock: AlgoramBlock = {
  id: "block:phase2-python-rust",
  label: jaJP.demo.graphPythonRust,
  ports: [
    port("value", "in", "data", "python:ctypes:c_int"),
    port("result", "out", "data", "c:abi:int32"),
  ],
  internal_graph_ref: rustInternalGraphId,
  definition_ref: "definition:python-rust-triple",
  extensions: {
    composite: {
      definition_id: "definition:python-rust-triple",
      internal_graph_id: rustInternalGraphId,
    },
  },
};

const rootGraph: AlgoramGraph = {
  schema_version: "algoram.graph/0.1",
  id: rootGraphId,
  label: jaJP.demo.rootGraph,
  blocks: [compositeBlock, rustCompositeBlock],
  connections: [],
};

const boundaryInput: AlgoramBlock = {
  id: "block:boundary-input",
  label: jaJP.demo.pythonInputBoundary,
  ports: [port("value", "out", "data", "python:ctypes:c_int")],
  source_anchor: anchorWhole(pythonArtifact, pythonSource),
};

const buildC: AlgoramBlock = {
  id: "block:build-c",
  label: jaJP.demo.buildCSharedLibrary,
  ports: [port("flow_out", "out", "flow")],
  implementation_ref: "fixture:composite-build-c",
  source_anchor: anchorWhole(cArtifact, cSource),
};

const invokeC: AlgoramBlock = {
  id: "block:invoke-c",
  label: jaJP.demo.callCThroughCtypes,
  ports: [
    port("flow_in", "in", "flow"),
    port(
      "value",
      "in",
      "data",
      "c:function:algoram_checked_double:int32",
    ),
    port("result", "out", "data", "c:abi:int32"),
  ],
  implementation_ref: "fixture:composite-invoke-c",
  source_anchor: anchorFragment(
    cArtifact,
    cSource,
    "int32_t algoram_checked_double",
    "c:algoram_checked_double",
  ),
};

const boundaryOutput: AlgoramBlock = {
  id: "block:boundary-output",
  label: jaJP.demo.pythonResultBoundary,
  ports: [port("result", "in", "data", "c:abi:int32")],
  source_anchor: anchorWhole(pythonArtifact, pythonSource),
};

const internalGraph: AlgoramGraph = {
  schema_version: "algoram.graph/0.1",
  id: internalGraphId,
  label: jaJP.demo.pythonCInternalGraph,
  blocks: [boundaryInput, buildC, invokeC, boundaryOutput],
  connections: [
    connection(
      "flow:build-invoke",
      buildC.id,
      "flow_out",
      invokeC.id,
      "flow_in",
    ),
    connection(
      "data:input-invoke",
      boundaryInput.id,
      "value",
      invokeC.id,
      "value",
    ),
    connection(
      "data:invoke-output",
      invokeC.id,
      "result",
      boundaryOutput.id,
      "result",
    ),
  ],
  source_artifacts: [cArtifact, pythonArtifact],
};


const rustBoundaryInput: AlgoramBlock = {
  id: "block:phase2-python-input",
  label: jaJP.demo.pythonCtypesInputBoundary,
  ports: [port("value", "out", "data", "python:ctypes:c_int")],
  source_anchor: anchorWhole(rustPythonArtifact, rustPythonSource),
};

const rustBuild: AlgoramBlock = {
  id: "block:phase2-build-rust",
  label: jaJP.demo.buildRustCdylib,
  ports: [port("flow_out", "out", "flow")],
  implementation_ref: "fixture:build-rust-cdylib",
  source_anchor: anchorWhole(rustArtifact, rustSource),
};

const rustInvoke: AlgoramBlock = {
  id: "rust:artifact:python-rust-cabi:file/function:algoram_checked_triple",
  label: jaJP.demo.invokeRustExternC,
  ports: [
    port("flow_in", "in", "flow"),
    port(
      "value",
      "in",
      "data",
      "rust:extern-c:function:algoram_checked_triple:int32",
    ),
    port("result", "out", "data", "c:abi:int32"),
  ],
  implementation_ref: "fixture:invoke-rust-through-python-ctypes",
  source_anchor: anchorFragment(
    rustArtifact,
    rustSource,
    'pub extern "C" fn algoram_checked_triple',
    "rust:algoram_checked_triple",
  ),
  extensions: {
    rust: {
      semantic_kind: "function_item",
      syntax: {
        kind: "function_item",
        name: "algoram_checked_triple",
        extern_abi: 'extern "C"',
      },
    },
  },
};

const rustBoundaryOutput: AlgoramBlock = {
  id: "block:phase2-python-output",
  label: jaJP.demo.pythonCtypesResultBoundary,
  ports: [port("result", "in", "data", "c:abi:int32")],
  source_anchor: anchorWhole(rustPythonArtifact, rustPythonSource),
};

const rustInternalGraph: AlgoramGraph = {
  schema_version: "algoram.graph/0.1",
  id: rustInternalGraphId,
  label: jaJP.demo.pythonRustInternalGraph,
  blocks: [rustBoundaryInput, rustBuild, rustInvoke, rustBoundaryOutput],
  connections: [
    connection(
      "flow:phase2-build-invoke",
      rustBuild.id,
      "flow_out",
      rustInvoke.id,
      "flow_in",
    ),
    connection(
      "data:phase2-input-invoke",
      rustBoundaryInput.id,
      "value",
      rustInvoke.id,
      "value",
    ),
    connection(
      "data:phase2-invoke-output",
      rustInvoke.id,
      "result",
      rustBoundaryOutput.id,
      "result",
    ),
  ],
  source_artifacts: [rustArtifact, rustPythonArtifact],
};

export const demoBundle: ReferenceBundle = {
  rootGraphId,
  editableGraphIds: [rootGraphId],
  graphs: {
    [rootGraph.id]: rootGraph,
    [internalGraph.id]: internalGraph,
    [routeGraph.id]: routeGraph,
    [rustInternalGraph.id]: rustInternalGraph,
    [rustRouteGraph.id]: rustRouteGraph,
  },
  sources: {
    [cArtifact.id]: {
      artifact: cArtifact,
      text: cSource,
    },
    [pythonArtifact.id]: {
      artifact: pythonArtifact,
      text: pythonSource,
    },
    [rustArtifact.id]: {
      artifact: rustArtifact,
      text: rustSource,
    },
    [rustPythonArtifact.id]: {
      artifact: rustPythonArtifact,
      text: rustPythonSource,
    },
  },
  routeInspections: {
    [invokeC.id]: routeGraphId,
    [rustInvoke.id]: rustRouteGraphId,
  },
};
