import assert from "node:assert/strict";
import { readdir, readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../src", import.meta.url));

async function filesUnder(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const nested = await Promise.all(
    entries.map(async (entry) => {
      const target = path.join(directory, entry.name);
      if (entry.isDirectory()) return filesUnder(target);
      return /\.[cm]?[jt]sx?$/.test(entry.name) ? [target] : [];
    }),
  );
  return nested.flat();
}

const forbidden = [
  { label: "node: builtin import", pattern: /(?:from\s+|import\s*\()["']node:/ },
  { label: "Node fs import", pattern: /(?:from\s+|import\s*\()["'](?:fs|fs\/promises)["']/ },
  { label: "Node child_process import", pattern: /(?:from\s+|import\s*\()["']child_process["']/ },
  { label: "Electron import", pattern: /(?:from\s+|import\s*\()["']electron["']/ },
  { label: "CommonJS require", pattern: /\brequire\s*\(/ },
  { label: "Electron bridge global", pattern: /\b(?:ipcRenderer|window\.electron|window\.api)\b/ },
];

const violations = [];
for (const file of await filesUnder(root)) {
  const source = await readFile(file, "utf8");
  for (const rule of forbidden) {
    if (rule.pattern.test(source)) {
      violations.push({
        file: path.relative(root, file),
        rule: rule.label,
      });
    }
  }
}

assert.deepEqual(
  violations,
  [],
  `ui/src must remain Web Platform-first: ${JSON.stringify(violations)}`,
);

console.log(
  JSON.stringify({
    kind: "browser-platform-check",
    ui_source_files: (await filesUnder(root)).length,
    node_or_electron_runtime_dependencies: 0,
    baseline: "DOM/CSS/ES modules",
  }),
);
