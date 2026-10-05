import ELK from "elkjs/lib/elk.bundled.js";

const elk = new ELK();

console.log(
  JSON.stringify({
    kind: "environment",
    benchmark: "elk_layout",
    node: process.version,
    platform: process.platform,
    arch: process.arch,
  }),
);

for (const size of [50, 200, 500]) {
  const graph = {
    id: `perf-${size}`,
    layoutOptions: {
      "elk.algorithm": "layered",
      "elk.direction": "RIGHT",
      "elk.spacing.nodeNode": "48",
      "elk.layered.spacing.nodeNodeBetweenLayers": "96",
    },
    children: Array.from({ length: size }, (_, index) => ({
      id: `node-${index}`,
      width: 240,
      height: 96,
    })),
    edges: Array.from({ length: Math.max(0, size - 1) }, (_, index) => ({
      id: `edge-${index}`,
      sources: [`node-${index}`],
      targets: [`node-${index + 1}`],
    })),
  };

  await elk.layout(graph);

  const iterations = size <= 200 ? 3 : 2;
  const start = performance.now();
  for (let iteration = 0; iteration < iterations; iteration += 1) {
    const result = await elk.layout(graph);
    if ((result.children?.length ?? 0) !== size) {
      throw new Error(`ELK baseline lost nodes at size ${size}`);
    }
  }
  const elapsedMs = performance.now() - start;

  console.log(
    JSON.stringify({
      kind: "metric",
      metric: "elk_layout",
      size,
      iterations,
      elapsed_ms: elapsedMs,
      ms_per_iteration: elapsedMs / iterations,
    }),
  );
}
