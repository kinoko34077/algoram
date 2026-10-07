import type { AlgoramGraph } from "./algoram";
import { validateGraph } from "./graphValidation";

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
        ? `Invalid Graph JSON: ${error.message}`
        : "Invalid Graph JSON.",
    );
  }

  if (!isRecord(parsed)) {
    throw new Error("Graph JSON must contain one object.");
  }
  if (typeof parsed.schema_version !== "string") {
    throw new Error("Graph JSON is missing schema_version.");
  }
  if (typeof parsed.id !== "string") {
    throw new Error("Graph JSON is missing id.");
  }
  if (!Array.isArray(parsed.blocks)) {
    throw new Error("Graph JSON is missing blocks.");
  }
  if (!Array.isArray(parsed.connections)) {
    throw new Error("Graph JSON is missing connections.");
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
