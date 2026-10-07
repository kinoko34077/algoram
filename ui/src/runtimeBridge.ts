import type { AlgoramGraph, SourceAnchor } from "./algoram";

export interface ProcessAction {
  program: string;
  args: string[];
  argv_ports?: string[];
  current_dir?: string;
}

export interface ArgvPortBinding {
  target_port_id: string;
  source:
    | { kind: "external_port"; block_id: string; port_id: string }
    | { kind: "step_stdout"; step_id: string };
}

export interface ExecutionStep {
  id: string;
  implementation_ref: string;
  action: ProcessAction;
  origin_block_ids: string[];
  source_anchors?: SourceAnchor[];
  route_connector_ids?: string[];
  argv_bindings?: ArgvPortBinding[];
}

export interface ExecutionPlan {
  reference_graph_id: string;
  steps: ExecutionStep[];
}

export interface ExecutionAccessRequirement {
  step_id: string;
  implementation_ref: string;
  origin_block_ids: string[];
  access_class: "ambient_host_process";
  action: ProcessAction;
  argv_bindings?: ArgvPortBinding[];
}

export interface ExecutionAccessReport {
  reference_graph_id: string;
  requirements: ExecutionAccessRequirement[];
}

export interface PlanResponse {
  plan: ExecutionPlan;
  access_report: ExecutionAccessReport;
}

export interface TraceEntry {
  step_id: string;
  implementation_ref: string;
  status: "succeeded" | "failed" | "not_run";
  origin_block_ids: string[];
  source_anchors?: SourceAnchor[];
  route_connector_ids?: string[];
  exit_code?: number;
  stdout: string;
  stderr: string;
}

export interface ExecutionTrace {
  reference_graph_id: string;
  entries: TraceEntry[];
}

export interface RunResponse {
  access_report: ExecutionAccessReport;
  trace: ExecutionTrace;
}

export interface RuntimeBridgeSettings {
  baseUrl: string;
  bearerToken: string;
}

export interface RuntimeBridgeClient {
  plan(graph: AlgoramGraph, signal?: AbortSignal): Promise<PlanResponse>;
  run(
    graph: AlgoramGraph,
    expectedPlan: ExecutionPlan,
    allowedImplementationRefs: string[],
    signal?: AbortSignal,
  ): Promise<RunResponse>;
}

export type RuntimeBridgeFetch = (
  input: RequestInfo | URL,
  init?: RequestInit,
) => Promise<Response>;

interface ApiError {
  code?: string;
  message?: string;
}

export class RuntimeBridgeError extends Error {
  readonly status: number;
  readonly code: string;

  constructor(status: number, code: string, message: string) {
    super(message);
    this.name = "RuntimeBridgeError";
    this.status = status;
    this.code = code;
  }
}

function isLoopbackHost(hostname: string): boolean {
  const normalized = hostname.toLowerCase();
  return (
    normalized === "localhost" ||
    normalized === "127.0.0.1" ||
    normalized === "::1" ||
    normalized === "[::1]"
  );
}

export function normalizeLoopbackBridgeUrl(value: string): string {
  let url: URL;
  try {
    url = new URL(value);
  } catch {
    throw new Error("Bridge URL must be a valid http(s) URL.");
  }

  if (url.protocol !== "http:" && url.protocol !== "https:") {
    throw new Error("Bridge URL must use http or https.");
  }
  if (!isLoopbackHost(url.hostname)) {
    throw new Error("Runtime bridge URL must resolve to loopback localhost.");
  }
  if (url.username || url.password || url.search || url.hash) {
    throw new Error("Bridge URL cannot contain credentials, query, or fragment.");
  }
  if (url.pathname !== "/" && url.pathname !== "") {
    throw new Error("Bridge URL must be an origin without a path.");
  }

  return url.origin;
}

async function decodeResponse<T>(response: Response): Promise<T> {
  let payload: unknown = null;
  try {
    payload = await response.json();
  } catch {
    if (!response.ok) {
      throw new RuntimeBridgeError(
        response.status,
        "bridge_error",
        `Runtime bridge returned HTTP ${response.status}.`,
      );
    }
  }

  if (!response.ok) {
    const apiError = payload as ApiError | null;
    throw new RuntimeBridgeError(
      response.status,
      apiError?.code ?? "bridge_error",
      apiError?.message ??
        `Runtime bridge request failed with HTTP ${response.status}.`,
    );
  }

  return payload as T;
}

export function createRuntimeBridgeClient(
  settings: RuntimeBridgeSettings,
  fetchImpl: RuntimeBridgeFetch = fetch,
): RuntimeBridgeClient {
  const baseUrl = normalizeLoopbackBridgeUrl(settings.baseUrl);
  if (settings.bearerToken.trim().length === 0) {
    throw new Error("Enter the runtime bridge bearer token.");
  }

  async function requestJson<T>(
    path: string,
    body: unknown,
    signal?: AbortSignal,
  ): Promise<T> {
    let response: Response;
    try {
      response = await fetchImpl(`${baseUrl}${path}`, {
        method: "POST",
        headers: {
          Authorization: `Bearer ${settings.bearerToken}`,
          "Content-Type": "application/json",
        },
        body: JSON.stringify(body),
        signal,
      });
    } catch (error: unknown) {
      if (error instanceof DOMException && error.name === "AbortError") {
        throw error;
      }
      throw new Error(
        error instanceof Error
          ? `Runtime bridge unavailable: ${error.message}`
          : "Runtime bridge unavailable.",
      );
    }

    return decodeResponse<T>(response);
  }

  return {
    plan(graph, signal) {
      return requestJson<PlanResponse>("/v1/plan", { graph }, signal);
    },

    run(graph, expectedPlan, allowedImplementationRefs, signal) {
      return requestJson<RunResponse>(
        "/v1/run",
        {
          graph,
          expected_plan: expectedPlan,
          allowed_implementation_refs: allowedImplementationRefs,
        },
        signal,
      );
    },
  };
}

export function planGraph(
  settings: RuntimeBridgeSettings,
  graph: AlgoramGraph,
  signal?: AbortSignal,
): Promise<PlanResponse> {
  return createRuntimeBridgeClient(settings).plan(graph, signal);
}

export function runGraph(
  settings: RuntimeBridgeSettings,
  graph: AlgoramGraph,
  expectedPlan: ExecutionPlan,
  allowedImplementationRefs: string[],
  signal?: AbortSignal,
): Promise<RunResponse> {
  return createRuntimeBridgeClient(settings).run(
    graph,
    expectedPlan,
    allowedImplementationRefs,
    signal,
  );
}

export function requiredImplementationRefs(
  preview: PlanResponse,
): string[] {
  return [
    ...new Set(
      preview.access_report.requirements.map(
        (requirement) => requirement.implementation_ref,
      ),
    ),
  ].sort();
}
