import assert from "node:assert/strict";
import {
  buildRecoverySelections,
  recoveryRouteCandidateKey,
  runtimeSelectionBlocksApply,
  selectedRuntimeRecovery,
} from "../src/recoverySelection.ts";

const options = {
  report: {
    reference_graph_id: "graph:test",
    failures: [],
    impact: {
      failed_block_ids: ["block:failed"],
      affected_block_ids: ["block:downstream"],
      traversed_connections: [],
    },
    selected_route_health: [],
  },
  route_candidates: [
    {
      step_id: "step:route",
      origin_block_ids: ["block:failed"],
      connection_id: "connection:b",
      source_contract: "contract:a",
      target_contract: "contract:b",
      connector_ids: ["connector:2"],
    },
    {
      step_id: "step:route",
      origin_block_ids: ["block:failed"],
      connection_id: "connection:a",
      source_contract: "contract:a",
      target_contract: "contract:b",
      connector_ids: ["connector:1"],
    },
  ],
  implementation_candidates: [],
  runtime_candidates: [
    {
      step_id: "step:failed",
      current_runtime_ref: "runtime:local",
      candidates: [
        {
          candidate: {
            runtime_ref: "runtime:local",
            class: "local_process",
            is_current: true,
          },
          placement_validated: true,
          executable_by_bridge: true,
        },
        {
          candidate: {
            runtime_ref: "runtime:remote",
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

assert.deepEqual(buildRecoverySelections(options, {}, {}), {
  route_overrides: [],
  implementation_overrides: [],
});

const explicit = buildRecoverySelections(
  options,
  {
    "connection:b": recoveryRouteCandidateKey(
      "connection:b",
      ["connector:2"],
    ),
    "connection:a": recoveryRouteCandidateKey(
      "connection:a",
      ["connector:1"],
    ),
  },
  {
    "logical:z": "impl:z",
    "logical:a": "impl:a",
  },
);
assert.deepEqual(explicit, {
  route_overrides: [
    {
      connection_id: "connection:a",
      connector_ids: ["connector:1"],
    },
    {
      connection_id: "connection:b",
      connector_ids: ["connector:2"],
    },
  ],
  implementation_overrides: [
    {
      logical_ref: "logical:a",
      implementation_ref: "impl:a",
    },
    {
      logical_ref: "logical:z",
      implementation_ref: "impl:z",
    },
  ],
});

assert.equal(
  runtimeSelectionBlocksApply(options, {
    "step:failed": "runtime:remote",
  }),
  true,
);
assert.equal(
  runtimeSelectionBlocksApply(options, {
    "step:failed": "runtime:local",
  }),
  false,
);
assert.deepEqual(
  selectedRuntimeRecovery(options, {
    "step:failed": "runtime:remote",
  }).map(({ stepId, option }) => ({
    stepId,
    runtimeRef: option.candidate.runtime_ref,
    executable: option.executable_by_bridge,
  })),
  [
    {
      stepId: "step:failed",
      runtimeRef: "runtime:remote",
      executable: false,
    },
  ],
);

const before = structuredClone(options);
buildRecoverySelections(options, {}, {
  "logical:test": "impl:test",
});
assert.deepEqual(options, before, "selection projection must not mutate options");

console.log(
  JSON.stringify({
    kind: "recovery-selection-check",
    default_selection: "none",
    explicit_payload: "deterministic",
    remote_runtime_block: "pass",
    immutable_options: "pass",
  }),
);
