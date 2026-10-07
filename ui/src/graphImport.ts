import type { AlgoramGraph } from "./algoram";
import { validateGraph } from "./graphValidation";
import { jaJP } from "./locales/ja-JP/index.ts";

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

export function parseCanonicalGraphJson(input: string): AlgoramGraph {
  let parsed: unknown;
  try {
    parsed = JSON.parse(input);
  } catch (error: unknown) {
    throw new Error(
      error instanceof Error
        ? jaJP.errors.graphImport.invalidJsonWithError.replace("{error}", error.message)
        : jaJP.errors.graphImport.invalidJson,
    );
  }

  if (!isRecord(parsed)) {
    throw new Error(jaJP.errors.graphImport.mustContainObject);
  }
  if (typeof parsed.schema_version !== "string") {
    throw new Error(jaJP.errors.graphImport.missingSchemaVersion);
  }
  if (typeof parsed.id !== "string") {
    throw new Error(jaJP.errors.graphImport.missingId);
  }
  if (!Array.isArray(parsed.blocks)) {
    throw new Error(jaJP.errors.graphImport.missingBlocks);
  }
  if (!Array.isArray(parsed.connections)) {
    throw new Error(jaJP.errors.graphImport.missingConnections);
  }

  const graph = parsed as unknown as AlgoramGraph;
  const issues = validateGraph(graph);
  if (issues.length > 0) {
    throw new Error(
      issues
        .slice(0, 4)
        .map((issue) => issue.message)
        .join(" "),
    );
  }

  return structuredClone(graph);
}

export async function readCanonicalGraphFile(file: File): Promise<AlgoramGraph> {
  return parseCanonicalGraphJson(await file.text());
}
