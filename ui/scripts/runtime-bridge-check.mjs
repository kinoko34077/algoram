import assert from "node:assert/strict";
import {
  normalizeLoopbackBridgeUrl,
  planGraph,
  requiredImplementationRefs,
  runGraph,
} from "../src/runtimeBridge.ts";

assert.equal(
  normalizeLoopbackBridgeUrl("http://127.0.0.1:39091/"),
  "http://127.0.0.1:39091",
);
assert.equal(
  normalizeLoopbackBridgeUrl("http://localhost:39091"),
  "http://localhost:39091",
);
assert.throws(
  () => normalizeLoopbackBridgeUrl("https://example.com"),
  /loopback localhost/i,
);
assert.throws(
  () => normalizeLoopbackBridgeUrl("file:///tmp/bridge"),
  /http or https/i,
);
assert.throws(
  () => normalizeLoopbackBridgeUrl("http://user:pass@127.0.0.1:39091"),
  /credentials/i,
);

const graph = {
  schema_version: "algoram.graph/0.1",
  id: "graph:ui-runtime-test",
  blocks: [],
  connections: [],
};
const plan = {
  reference_graph_id: graph.id,
  steps: [
    {
      id: "step:test",
      implementation_ref: "impl:a",
      action: { program: "trusted", args: [] },
      origin_block_ids: ["block:test"],
    },
  ],
};
const preview = {
  plan,
  access_report: {
    reference_graph_id: graph.id,
    requirements: [
      {
        step_id: "step:test",
        implementation_ref: "impl:a",
        origin_block_ids: ["block:test"],
        access_class: "ambient_host_process",
        action: { program: "trusted", args: [] },
      },
      {
        step_id: "step:test-2",
        implementation_ref: "impl:a",
        origin_block_ids: ["block:test-2"],
        access_class: "ambient_host_process",
        action: { program: "trusted", args: [] },
      },
      {
        step_id: "step:test-3",
        implementation_ref: "impl:b",
        origin_block_ids: ["block:test-3"],
        access_class: "ambient_host_process",
        action: { program: "trusted", args: [] },
      },
    ],
  },
};

assert.deepEqual(requiredImplementationRefs(preview), ["impl:a", "impl:b"]);

const calls = [];
globalThis.fetch = async (url, init) => {
  calls.push({ url: String(url), init });
  const request = JSON.parse(String(init?.body ?? "{}"));
  if (String(url).endsWith("/v1/plan")) {
    assert.deepEqual(request, { graph });
    assert.equal(JSON.stringify(request).includes("super-secret"), false);
    return new Response(JSON.stringify(preview), {
      status: 200,
      headers: { "Content-Type": "application/json" },
    });
  }

  assert.deepEqual(request, {
    graph,
    expected_plan: plan,
    allowed_implementation_refs: ["impl:a"],
  });
  assert.equal(JSON.stringify(request).includes("super-secret"), false);
  return new Response(
    JSON.stringify({
      access_report: preview.access_report,
      trace: { reference_graph_id: graph.id, entries: [] },
    }),
    {
      status: 200,
      headers: { "Content-Type": "application/json" },
    },
  );
};

const settings = {
  baseUrl: "http://127.0.0.1:39091",
  bearerToken: "super-secret",
};
await planGraph(settings, graph);
await runGraph(settings, graph, plan, ["impl:a"]);

assert.equal(calls.length, 2);
for (const call of calls) {
  assert.equal(
    call.init.headers.Authorization,
    "Bearer super-secret",
  );
  assert.equal(call.url.startsWith("http://127.0.0.1:39091/v1/"), true);
}

let fetched = false;
globalThis.fetch = async () => {
  fetched = true;
  throw new Error("must not fetch");
};
await assert.rejects(
  () =>
    planGraph(
      {
        baseUrl: "https://evil.example",
        bearerToken: "super-secret",
      },
      graph,
    ),
  /loopback localhost/i,
);
assert.equal(fetched, false);

console.log(
  JSON.stringify({
    kind: "runtime-bridge-ui-check",
    loopback_url_guard: "pass",
    bearer_header_only: "pass",
    graph_body_has_no_secret: "pass",
    explicit_grant_body: "pass",
  }),
);
