import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import {
  GraphExportValidationError,
  graphExportFilename,
  prepareCanonicalGraphExport,
  serializeCanonicalGraph,
} from "../src/graphExport.ts";

const fixtureUrl = new URL(
  "../../fixtures/gui-export/portable.algoram.json",
  import.meta.url,
);
const fixtureText = await readFile(fixtureUrl, "utf8");
const normalizedFixtureText = fixtureText.replace(/\r\n/g, "\n");
const graph = JSON.parse(fixtureText);

const localEditorState = {
  positions: {
    "block:source": {
      x: 12345,
      y: 67890,
      marker: "local-only-position-marker",
    },
  },
  viewport: {
    x: 1,
    y: 2,
    zoom: 3,
    marker: "local-only-viewport-marker",
  },
  selection: "block:source",
  draftLinks: [{ id: "local-only-draft-marker" }],
};

const payload = prepareCanonicalGraphExport(graph, () => []);
const serialized = serializeCanonicalGraph(graph);

assert.equal(payload.json, normalizedFixtureText);
assert.equal(serialized, normalizedFixtureText);
assert.equal(
  payload.filename,
  "algoram-graph-gui-export-portable.algoram.json",
);
assert.equal(graphExportFilename(graph), payload.filename);

const restored = JSON.parse(payload.json);
assert.deepEqual(restored, graph);
assert.deepEqual(restored.extensions, graph.extensions);
assert.deepEqual(restored.presentation, graph.presentation);
assert.equal(restored.presentation["canonical.note"], "preserve-me");

for (const marker of [
  localEditorState.positions["block:source"].marker,
  localEditorState.viewport.marker,
  localEditorState.draftLinks[0].id,
]) {
  assert.equal(payload.json.includes(marker), false);
}

assert.throws(
  () =>
    prepareCanonicalGraphExport(graph, () => [
      { message: "synthetic validation failure" },
    ]),
  (error) => {
    assert.equal(error instanceof GraphExportValidationError, true);
    assert.match(error.message, /synthetic validation failure/);
    return true;
  },
);

const browserDownloadSource = await readFile(
  new URL("../src/browserDownload.ts", import.meta.url),
  "utf8",
);
assert.match(browserDownloadSource, /new Blob\(/);
assert.match(browserDownloadSource, /URL\.createObjectURL\(/);
assert.match(browserDownloadSource, /document\.createElement\("a"\)/);
assert.match(browserDownloadSource, /anchor\.download = payload\.filename/);
assert.match(browserDownloadSource, /anchor\.click\(\)/);
assert.match(browserDownloadSource, /URL\.revokeObjectURL\(objectUrl\)/);
assert.equal(browserDownloadSource.includes("showSaveFilePicker"), false);
assert.equal(browserDownloadSource.includes("node:fs"), false);
assert.equal(browserDownloadSource.includes("electron"), false);

console.log(
  JSON.stringify({
    kind: "canonical-graph-export-check",
    fixture_bytes: Buffer.byteLength(payload.json),
    filename: payload.filename,
    exact_fixture_match: true,
    unknown_extensions_preserved: true,
    canonical_presentation_preserved: true,
    editor_local_state_excluded: true,
    validation_gate: "pass",
    browser_download_baseline: "Blob/objectURL/anchor",
  }),
);
