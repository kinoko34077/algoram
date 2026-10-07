import assert from "node:assert/strict";
import {
  RuntimeBridgeError,
  createRuntimeBridgeClient,
  normalizeLoopbackBridgeUrl,
  requiredImplementationRefs,
} from "../src/runtimeBridge.ts";
import { jaJP } from "../src/locales/ja-JP/index.ts";

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
  (error) => error instanceof Error && error.message === jaJP.errors.runtimeBridge.loopbackOnly,
);
assert.throws(
  () => normalizeLoopbackBridgeUrl("file:///tmp/bridge"),
  (error) => error instanceof Error && error.message === jaJP.errors.runtimeBridge.httpOnly,
);
assert.throws(
  () => normalizeLoopbackBridgeUrl("http://user:pass@127.0.0.1:39091"),
  (error) =>
    error instanceof Error &&
    error.message === jaJP.errors.runtimeBridge.noCredentialsQueryFragment,
);
assert.throws(
  () => normalizeLoopbackBridgeUrl("http://127.0.0.1:39091/admin"),
  (error) => error instanceof Error && error.message === jaJP.errors.runtimeBridge.originOnly,
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
const failedTrace = {
  reference_graph_id: graph.id,
  entries: [
    {
      step_id: "step:test",
      implementation_ref: "impl:a",
      status: "failed",
      origin_block_ids: ["block:test"],
      source_anchors: [],
      route_connector_ids: [],
      exit_code: 17,
      stdout: "",
      stderr: "provider unavailable",
    },
  ],
};
const recoverySelections = {
  route_overrides: [],
  implementation_overrides: [
    {
      logical_ref: "logical:test",
      implementation_ref: "impl:b",
    },
  ],
};
const recoveryOptions = {
  report: {
    reference_graph_id: graph.id,
    failures: [{ entry: failedTrace.entries[0] }],
    impact: {
      failed_block_ids: ["block:test"],
      affected_block_ids: [],
      traversed_connections: [],
    },
    selected_route_health: [],
  },
  route_candidates: [],
  implementation_candidates: [
    {
      step_id: "step:test",
      selected_implementation_ref: "impl:a",
      candidates: {
        logical_implementation_ref: "logical:test",
        trusted_local: [
          { implementation_ref: "impl:a", is_default: true },
          { implementation_ref: "impl:b", is_default: false },
        ],
        catalog_only: [],
      },
    },
  ],
  runtime_candidates: [
    {
      step_id: "step:test",
      current_runtime_ref: "runtime:local-bridge",
      candidates: [
        {
          candidate: {
            runtime_ref: "runtime:agent-alt",
            class: "runtime_agent",
            is_current: false,
          },
          placement_validated: true,
          executable_by_bridge: false,
        },
      ],
    },
  ],
  catalog_connected: false,
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
const mockFetch = async (url, init) => {
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

  if (String(url).endsWith("/v1/run")) {
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
  }

  if (String(url).endsWith("/v1/recovery/options")) {
    assert.deepEqual(request, {
      graph,
      expected_plan: plan,
      trace: failedTrace,
    });
    return new Response(JSON.stringify(recoveryOptions), {
      status: 200,
      headers: { "Content-Type": "application/json" },
    });
  }

  if (String(url).endsWith("/v1/recovery/plan")) {
    assert.deepEqual(request, {
      graph,
      selections: recoverySelections,
    });
    return new Response(JSON.stringify(preview), {
      status: 200,
      headers: { "Content-Type": "application/json" },
    });
  }

  assert.equal(String(url).endsWith("/v1/recovery/run"), true);
  assert.deepEqual(request, {
    graph,
    expected_plan: plan,
    allowed_implementation_refs: ["impl:b"],
    selections: recoverySelections,
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
const client = createRuntimeBridgeClient(settings, mockFetch);
await client.plan(graph);
await client.run(graph, plan, ["impl:a"]);
assert.deepEqual(
  await client.recoveryOptions(graph, plan, failedTrace),
  recoveryOptions,
);
await client.recoveryPlan(graph, recoverySelections);
await client.recoveryRun(graph, plan, ["impl:b"], recoverySelections);

assert.equal(calls.length, 5);
for (const call of calls) {
  assert.equal(call.init.headers.Authorization, "Bearer super-secret");
  assert.equal(call.url.startsWith("http://127.0.0.1:39091/v1/"), true);
}

let fetched = false;
assert.throws(
  () =>
    createRuntimeBridgeClient(
      {
        baseUrl: "https://evil.example",
        bearerToken: "super-secret",
      },
      async () => {
        fetched = true;
        throw new Error("must not fetch");
      },
    ),
  (error) => error instanceof Error && error.message === jaJP.errors.runtimeBridge.loopbackOnly,
);
assert.equal(fetched, false);

const deniedClient = createRuntimeBridgeClient(settings, async () =>
  new Response(
    JSON.stringify({
      code: "execution_denied",
      message: "guarded execution denied: implementation is not allowed",
    }),
    {
      status: 403,
      headers: { "Content-Type": "application/json" },
    },
  ),
);
await assert.rejects(
  () => deniedClient.plan(graph),
  (error) =>
    error instanceof RuntimeBridgeError &&
    error.status === 403 &&
    error.code === "execution_denied",
);

let observedSignal = null;
const cancelClient = createRuntimeBridgeClient(settings, async (_url, init) => {
  observedSignal = init?.signal ?? null;
  return await new Promise((_resolve, reject) => {
    observedSignal?.addEventListener(
      "abort",
      () => reject(new DOMException("Aborted", "AbortError")),
      { once: true },
    );
  });
});
const controller = new AbortController();
const pending = cancelClient.plan(graph, controller.signal);
controller.abort();
await assert.rejects(
  () => pending,
  (error) => error instanceof DOMException && error.name === "AbortError",
);
assert.equal(observedSignal, controller.signal);

console.log(
  JSON.stringify({
    kind: "runtime-bridge-ui-check",
    loopback_url_guard: "pass",
    replaceable_fetch_adapter: "pass",
    bearer_header_only: "pass",
    graph_body_has_no_secret: "pass",
    explicit_grant_body: "pass",
    typed_api_error: "pass",
    planning_abort_signal: "pass",
    recovery_discovery_payload: "pass",
    recovery_plan_payload: "pass",
    recovery_run_payload: "pass",
  }),
);
