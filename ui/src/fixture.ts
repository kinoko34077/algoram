import type {
  AlgoramBlock,
  AlgoramGraph,
  AlgoramPort,
  ReferenceBundle,
  SourceAnchor,
  SourceArtifact,
} from "./algoram";

const source = `class Greeter:
    def hello(self, name):
        prefix = "hi"
        print(prefix)
        if name:
            return name
`;

const artifact: SourceArtifact = {
  id: "artifact:ui-demo",
  origin: "fixtures/greeter.py",
  language: "python",
  revision: "fixture",
};

const encoder = new TextEncoder();

function byteOffset(charOffset: number): number {
  return encoder.encode(source.slice(0, charOffset)).length;
}

function pointAt(charOffset: number): { line: number; column: number } {
  const prefix = source.slice(0, charOffset);
  const lines = prefix.split("\n");
  const tail = lines.at(-1) ?? "";
  return {
    line: lines.length - 1,
    column: encoder.encode(tail).length,
  };
}

function nthIndexOf(fragment: string, occurrence = 0): number {
  let cursor = 0;
  for (let index = 0; index <= occurrence; index += 1) {
    const found = source.indexOf(fragment, cursor);
    if (found < 0) {
      throw new Error(`Fixture fragment not found: ${fragment}`);
    }
    if (index === occurrence) {
      return found;
    }
    cursor = found + fragment.length;
  }
  throw new Error(`Fixture occurrence not found: ${fragment}`);
}

function anchor(fragment: string, occurrence = 0): SourceAnchor {
  const startChar = nthIndexOf(fragment, occurrence);
  const endChar = startChar + fragment.length;
  const start = pointAt(startChar);
  const end = pointAt(endChar);

  return {
    artifact_id: artifact.id,
    start_byte: byteOffset(startChar),
    end_byte: byteOffset(endChar),
    start_line: start.line,
    start_column: start.column,
    end_line: end.line,
    end_column: end.column,
  };
}

function pythonExtensions(kind: string): Record<string, unknown> {
  return {
    python: {
      semantic_kind: kind,
    },
  };
}

function flowPorts(): AlgoramPort[] {
  return [
    { id: "flow_in", direction: "in", channel: "flow" },
    { id: "flow_out", direction: "out", channel: "flow" },
  ];
}

function graph(
  id: string,
  label: string,
  blocks: AlgoramBlock[],
  connections: AlgoramGraph["connections"] = [],
): AlgoramGraph {
  return {
    schema_version: "algoram.graph/0.1",
    id,
    label,
    blocks,
    connections,
    source_artifacts: [artifact],
  };
}

const rootGraphId = "graph:python:artifact:ui-demo:document";
const moduleGraphId = "graph:python:artifact:ui-demo:module";
const fileBlockId = "python:artifact:ui-demo:file";
const classBlockId = `${fileBlockId}/class:Greeter`;
const classGraphId = `graph:${classBlockId}`;
const functionBlockId = `${classBlockId}/function:hello`;
const functionGraphId = `graph:${functionBlockId}`;
const ifBlockId = `${functionBlockId}/if`;
const ifGraphId = `graph:${ifBlockId}`;

const fileBlock: AlgoramBlock = {
  id: fileBlockId,
  label: artifact.origin,
  internal_graph_ref: moduleGraphId,
  source_anchor: anchor(source),
  extensions: pythonExtensions("source_file"),
};

const classFragment = source;
const classBlock: AlgoramBlock = {
  id: classBlockId,
  label: "class Greeter",
  internal_graph_ref: classGraphId,
  source_anchor: anchor(classFragment),
  extensions: pythonExtensions("class_definition"),
};

const functionFragment = `def hello(self, name):
        prefix = "hi"
        print(prefix)
        if name:
            return name`;
const functionBlock: AlgoramBlock = {
  id: functionBlockId,
  label: "fn hello",
  internal_graph_ref: functionGraphId,
  source_anchor: anchor(functionFragment),
  ports: [
    {
      id: "param:self",
      direction: "in",
      channel: "data",
      contract: "python:unknown",
    },
    {
      id: "param:name",
      direction: "in",
      channel: "data",
      contract: "python:unknown",
    },
    {
      id: "return",
      direction: "out",
      channel: "data",
      contract: "python:unknown",
    },
  ],
  extensions: pythonExtensions("function_definition"),
};

const parameterSelf: AlgoramBlock = {
  id: `${functionBlockId}/parameter:self`,
  label: "parameter self",
  source_anchor: anchor("self"),
  extensions: pythonExtensions("parameter"),
};

const parameterName: AlgoramBlock = {
  id: `${functionBlockId}/parameter:name`,
  label: "parameter name",
  source_anchor: anchor("name"),
  extensions: pythonExtensions("parameter"),
};

const assignmentId = `${functionBlockId}/assignment:prefix`;
const assignment: AlgoramBlock = {
  id: assignmentId,
  label: "assign prefix",
  ports: flowPorts(),
  source_anchor: anchor('prefix = "hi"'),
  extensions: pythonExtensions("assignment"),
};

const callId = `${functionBlockId}/call:print`;
const call: AlgoramBlock = {
  id: callId,
  label: "call print(prefix)",
  ports: flowPorts(),
  source_anchor: anchor("print(prefix)"),
  extensions: pythonExtensions("call"),
};

const conditional: AlgoramBlock = {
  id: ifBlockId,
  label: "if name",
  ports: flowPorts(),
  internal_graph_ref: ifGraphId,
  source_anchor: anchor(`if name:
            return name`),
  extensions: pythonExtensions("if_statement"),
};

const returnBlock: AlgoramBlock = {
  id: `${ifBlockId}/return`,
  label: "return name",
  ports: flowPorts(),
  source_anchor: anchor("return name"),
  extensions: pythonExtensions("return_statement"),
};

const root = graph(rootGraphId, "Python source document", [fileBlock]);
const moduleGraph = graph(moduleGraphId, "Python module", [classBlock]);
const classGraph = graph(classGraphId, "Python class: Greeter", [functionBlock]);
const functionGraph = graph(
  functionGraphId,
  "Python function: hello",
  [parameterSelf, parameterName, assignment, call, conditional],
  [
    {
      id: "flow:assignment-call",
      source: { block_id: assignmentId, port_id: "flow_out" },
      target: { block_id: callId, port_id: "flow_in" },
    },
    {
      id: "flow:call-if",
      source: { block_id: callId, port_id: "flow_out" },
      target: { block_id: ifBlockId, port_id: "flow_in" },
    },
  ],
);
const ifGraph = graph(ifGraphId, "If body: name", [returnBlock]);

export const demoBundle: ReferenceBundle = {
  rootGraphId,
  graphs: {
    [root.id]: root,
    [moduleGraph.id]: moduleGraph,
    [classGraph.id]: classGraph,
    [functionGraph.id]: functionGraph,
    [ifGraph.id]: ifGraph,
  },
  sources: {
    [artifact.id]: {
      artifact,
      text: source,
    },
  },
};
