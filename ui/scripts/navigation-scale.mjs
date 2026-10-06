import assert from "node:assert/strict";
import {
  buildNavigationIndex,
  resolveBlockPath,
  searchNavigation,
} from "../src/navigation.ts";

function pad(value) {
  return String(value).padStart(3, "0");
}

function graph(id, label, blocks, sourceArtifacts = []) {
  return {
    schema_version: "algoram.graph/0.1",
    id,
    label,
    blocks,
    connections: [],
    source_artifacts: sourceArtifacts,
  };
}

function makeScaleBundle({ modules, functions, statements }) {
  const rootGraphId = "graph:scale:repository";
  const graphs = {};
  const rootBlocks = [];
  let target = null;

  for (let moduleIndex = 0; moduleIndex < modules; moduleIndex += 1) {
    const moduleKey = pad(moduleIndex);
    const moduleGraphId = `graph:scale:module:${moduleKey}`;
    const artifact = {
      id: `artifact:scale:module:${moduleKey}`,
      origin: `src/module-${moduleKey}.ts`,
      language: "typescript",
      revision: "generated-scale-fixture",
    };

    rootBlocks.push({
      id: `block:scale:module:${moduleKey}`,
      label: `Module ${moduleKey}`,
      internal_graph_ref: moduleGraphId,
    });

    const functionBlocks = [];

    for (let functionIndex = 0; functionIndex < functions; functionIndex += 1) {
      const functionKey = pad(functionIndex);
      const functionGraphId =
        `graph:scale:module:${moduleKey}:function:${functionKey}`;

      functionBlocks.push({
        id: `block:scale:module:${moduleKey}:function:${functionKey}`,
        label: `Function ${moduleKey}/${functionKey}`,
        internal_graph_ref: functionGraphId,
        source_anchor: {
          artifact_id: artifact.id,
          start_byte: functionIndex * 1000,
          end_byte: functionIndex * 1000 + 100,
          semantic_key: `module:${moduleKey}/function:${functionKey}`,
        },
      });

      const statementBlocks = [];

      for (
        let statementIndex = 0;
        statementIndex < statements;
        statementIndex += 1
      ) {
        const statementKey = pad(statementIndex);
        const blockId =
          `block:scale:module:${moduleKey}:function:${functionKey}:statement:${statementKey}`;
        const semanticKey =
          `module:${moduleKey}/function:${functionKey}/statement:${statementKey}`;

        statementBlocks.push({
          id: blockId,
          label: `Statement ${moduleKey}/${functionKey}/${statementKey}`,
          source_anchor: {
            artifact_id: artifact.id,
            start_byte:
              functionIndex * 1000 + statementIndex * 10,
            end_byte:
              functionIndex * 1000 + statementIndex * 10 + 8,
            semantic_key: semanticKey,
          },
        });

        if (
          moduleIndex === modules - 1 &&
          functionIndex === functions - 1 &&
          statementIndex === statements - 1
        ) {
          target = {
            blockId,
            semanticKey,
            sourceOrigin: artifact.origin,
            graphId: functionGraphId,
            moduleGraphId,
          };
        }
      }

      graphs[functionGraphId] = graph(
        functionGraphId,
        `Function ${moduleKey}/${functionKey}`,
        statementBlocks,
      );
    }

    graphs[moduleGraphId] = graph(
      moduleGraphId,
      `Module ${moduleKey}`,
      functionBlocks,
      [artifact],
    );
  }

  graphs[rootGraphId] = graph(
    rootGraphId,
    "Generated repository scale fixture",
    rootBlocks,
  );

  assert.ok(target, "scale fixture target must be generated");

  return {
    bundle: {
      rootGraphId,
      graphs,
      sources: {},
    },
    target,
  };
}

function time(action) {
  const start = performance.now();
  const result = action();
  return {
    result,
    elapsedMs: performance.now() - start,
  };
}

const cases = [
  { name: "small", modules: 10, functions: 10, statements: 10 },
  { name: "medium", modules: 25, functions: 20, statements: 10 },
  { name: "large", modules: 50, functions: 20, statements: 20 },
];

for (const scale of cases) {
  const { bundle, target } = makeScaleBundle(scale);
  const expectedRecords =
    scale.modules +
    scale.modules * scale.functions +
    scale.modules * scale.functions * scale.statements;
  const expectedGraphs = 1 + scale.modules + scale.modules * scale.functions;
  const maxSingleGraphBlocks = Math.max(
    scale.modules,
    scale.functions,
    scale.statements,
  );

  assert.equal(Object.keys(bundle.graphs).length, expectedGraphs);

  const indexed = time(() => buildNavigationIndex(bundle));
  const index = indexed.result;

  assert.equal(index.records.length, expectedRecords);

  const searched = time(() =>
    searchNavigation(index, target.semanticKey),
  );
  assert.equal(searched.result.length, 1);

  const record = searched.result[0];
  assert.equal(record.blockId, target.blockId);
  assert.equal(record.graphId, target.graphId);
  assert.equal(record.sourceOrigin, target.sourceOrigin);
  assert.equal(record.sourceLanguage, "typescript");
  assert.equal(record.semanticKey, target.semanticKey);

  const resolved = time(() => resolveBlockPath(index, record));
  const path = resolved.result;
  assert.ok(path, "deep target must resolve from root");
  assert.deepEqual(
    path.map((entry) => entry.graphId),
    [bundle.rootGraphId, target.moduleGraphId, target.graphId],
  );

  const actualMaxGraphBlocks = Math.max(
    ...Object.values(bundle.graphs).map((item) => item.blocks.length),
  );
  assert.equal(actualMaxGraphBlocks, maxSingleGraphBlocks);
  assert.equal(bundle.graphs[bundle.rootGraphId].blocks.length, scale.modules);

  if (scale.name !== "small") {
    assert.ok(
      expectedRecords > actualMaxGraphBlocks * 100,
      "indexed structure must materially exceed one-Graph materialization",
    );
  }

  for (const metric of [
    indexed.elapsedMs,
    searched.elapsedMs,
    resolved.elapsedMs,
  ]) {
    assert.ok(Number.isFinite(metric) && metric >= 0);
  }

  console.log(
    JSON.stringify({
      kind: "navigation_scale",
      case: scale.name,
      modules: scale.modules,
      functions_per_module: scale.functions,
      statements_per_function: scale.statements,
      graph_count: expectedGraphs,
      total_indexed_blocks: expectedRecords,
      root_current_graph_blocks: scale.modules,
      max_single_graph_blocks: actualMaxGraphBlocks,
      index_build_ms: indexed.elapsedMs,
      deep_search_ms: searched.elapsedMs,
      path_resolution_ms: resolved.elapsedMs,
      resolved_path_depth: path.length,
    }),
  );
}
