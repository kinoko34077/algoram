import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { validateGraph } from "../src/graphValidation.ts";

const fixture = JSON.parse(
  await readFile(
    new URL("../../fixtures/graph-validation/browser-core-parity.json", import.meta.url),
    "utf8",
  ),
);

assert.equal(fixture.schema_version, "algoram.graph-validation-parity/1");

for (const testCase of fixture.cases) {
  const issues = validateGraph(testCase.graph);
  const actual = issues[0]?.code ?? null;
  assert.equal(
    actual,
    testCase.expected,
    `browser validator parity case '${testCase.name}'`,
  );
}

console.log(
  JSON.stringify({
    kind: "graph-validation-parity-browser",
    cases: fixture.cases.length,
    result: "pass",
  }),
);
