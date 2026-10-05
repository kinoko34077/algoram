import { readdir, readFile } from "node:fs/promises";
import { extname, join, relative } from "node:path";

const root = new URL("../src/", import.meta.url);
const allowedExtensions = new Set([".ts", ".tsx", ".css"]);
const failures = [];

async function visit(directoryUrl) {
  const entries = await readdir(directoryUrl, { withFileTypes: true });
  for (const entry of entries) {
    const child = new URL(entry.name + (entry.isDirectory() ? "/" : ""), directoryUrl);
    if (entry.isDirectory()) {
      await visit(child);
      continue;
    }
    if (!allowedExtensions.has(extname(entry.name))) {
      continue;
    }

    const content = await readFile(child, "utf8");
    const displayPath = relative(new URL("..", root).pathname, child.pathname);

    content.split("\n").forEach((line, index) => {
      if (/\s+$/.test(line) && line.length > 0) {
        failures.push(`${displayPath}:${index + 1}: trailing whitespace`);
      }
      if (line.includes("\t")) {
        failures.push(`${displayPath}:${index + 1}: tab character`);
      }
    });

    if (entry.name !== "fixture.ts") {
      const forbidden = [
        /extensions\s*\.\s*python/,
        /extensions\s*\[\s*["']python["']\s*\]/,
        /semantic_kind/,
      ];
      for (const pattern of forbidden) {
        if (pattern.test(content)) {
          failures.push(
            `${displayPath}: renderer/navigation code must not depend on Python-specific semantic metadata`,
          );
        }
      }
    }
  }
}

await visit(root);

if (failures.length > 0) {
  console.error(failures.join("\n"));
  process.exit(1);
}

console.log("UI lint checks passed.");
