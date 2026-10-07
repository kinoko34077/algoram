import type { AlgoramGraph, SourceAnchor } from "./algoram";
import { jaJP } from "./locales/ja-JP";

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

export interface RouteOverride {
  connection_id: string;
  connector_ids: string[];
}

export interface ImplementationOverride {
  logical_ref: string;
  implementation_ref: string;
}

export interface RecoverySelections {
  route_overrides: RouteOverride[];
  implementation_overrides: ImplementationOverride[];
}

export interface FailureEvidence {
  runtime_ref?: string;
  entry: TraceEntry;
}

export interface ImpactConnection {
  connection_id: string;
  source_block_id: string;
  target_block_id: string;
  channel: "flow" | "data";
}

export interface DependencyImpact {
  failed_block_ids: string[];
  affected_block_ids: string[];
  traversed_connections: ImpactConnection[];
}

export interface SelectedRouteHealth {
  step_id: string;
  origin_block_ids: string[];
  connector_id: string;
  health: "available" | "unavailable" | "unknown";
}

export interface ResilienceReport {
  reference_graph_id: string;
  failures: FailureEvidence[];
  impact: DependencyImpact;
  selected_route_health: SelectedRouteHealth[];
}

export interface AlternativeRouteCandidate {
  step_id: string;
  origin_block_ids: string[];
  connection_id: string;
  source_contract: string;
  target_contract: string;
  connector_ids: string[];
}

export interface TrustedImplementationCandidate {
  implementation_ref: string;
  is_default: boolean;
}

export interface CatalogImplementationCandidate {
  listing_id: string;
  supplier: string;
  implementation_ref: string;
}

export interface ImplementationRecoveryCandidates {
  logical_implementation_ref: string;
  trusted_local: TrustedImplementationCandidate[];
  catalog_only: CatalogImplementationCandidate[];
}

export interface StepImplementationRecovery {
  step_id: string;
  selected_implementation_ref: string;
  candidates: ImplementationRecoveryCandidates;
}

export interface RuntimeRecoveryCandidate {
  runtime_ref: string;
  class: "local_process" | "runtime_agent";
  is_current: boolean;
}

export interface RuntimeRecoveryOption {
  candidate: RuntimeRecoveryCandidate;
  placement_validated: boolean;
  executable_by_bridge: boolean;
}

export interface StepRuntimeRecovery {
  step_id: string;
  current_runtime_ref: string;
  candidates: RuntimeRecoveryOption[];
}

export interface RecoveryOptionsResponse {
  report: ResilienceReport;
  route_candidates: AlternativeRouteCandidate[];
  implementation_candidates: StepImplementationRecovery[];
  runtime_candidates: StepRuntimeRecovery[];
  catalog_connected: boolean;
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
  recoveryOptions(
    graph: AlgoramGraph,
    expectedPlan: ExecutionPlan,
    trace: ExecutionTrace,
    signal?: AbortSignal,
  ): Promise<RecoveryOptionsResponse>;
  recoveryPlan(
    graph: AlgoramGraph,
    selections: RecoverySelections,
    signal?: AbortSignal,
  ): Promise<PlanResponse>;
  recoveryRun(
    graph: AlgoramGraph,
    expectedPlan: ExecutionPlan,
    allowedImplementationRefs: string[],
    selections: RecoverySelections,
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
    throw new Error(jaJP.errors.runtimeBridge.invalidUrl);
  }

  if (url.protocol !== "http:" && url.protocol !== "https:") {
    throw new Error(jaJP.errors.runtimeBridge.httpOnly);
  }
  if (!isLoopbackHost(url.hostname)) {
    throw new Error(jaJP.errors.runtimeBridge.loopbackOnly);
  }
  if (url.username || url.password || url.search || url.hash) {
    throw new Error(jaJP.errors.runtimeBridge.noCredentialsQueryFragment);
  }
  if (url.pathname !== "/" && url.pathname !== "") {
    throw new Error(jaJP.errors.runtimeBridge.originOnly);
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
        jaJP.errors.runtimeBridge.returnedHttp.replace("{status}", String(response.status)),
      );
    }
  }

  if (!response.ok) {
    const apiError = payload as ApiError | null;
    throw new RuntimeBridgeError(
      response.status,
      apiError?.code ?? "bridge_error",
      apiError?.message ??
        jaJP.errors.runtimeBridge.requestFailedHttp.replace("{status}", String(response.status)),
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
    throw new Error(jaJP.errors.runtimeBridge.tokenRequired);
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
          ? jaJP.errors.runtimeBridge.unavailableWithError.replace("{error}", error.message)
          : jaJP.errors.runtimeBridge.unavailable,
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

    recoveryOptions(graph, expectedPlan, trace, signal) {
      return requestJson<RecoveryOptionsResponse>(
        "/v1/recovery/options",
        {
          graph,
          expected_plan: expectedPlan,
          trace,
        },
        signal,
      );
    },

    recoveryPlan(graph, selections, signal) {
      return requestJson<PlanResponse>(
        "/v1/recovery/plan",
        {
          graph,
          selections,
        },
        signal,
      );
    },

    recoveryRun(
      graph,
      expectedPlan,
      allowedImplementationRefs,
      selections,
      signal,
    ) {
      return requestJson<RunResponse>(
        "/v1/recovery/run",
        {
          graph,
          expected_plan: expectedPlan,
          allowed_implementation_refs: allowedImplementationRefs,
          selections,
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


export function recoveryOptions(
  settings: RuntimeBridgeSettings,
  graph: AlgoramGraph,
  expectedPlan: ExecutionPlan,
  trace: ExecutionTrace,
  signal?: AbortSignal,
): Promise<RecoveryOptionsResponse> {
  return createRuntimeBridgeClient(settings).recoveryOptions(
    graph,
    expectedPlan,
    trace,
    signal,
  );
}

export function recoveryPlanGraph(
  settings: RuntimeBridgeSettings,
  graph: AlgoramGraph,
  selections: RecoverySelections,
  signal?: AbortSignal,
): Promise<PlanResponse> {
  return createRuntimeBridgeClient(settings).recoveryPlan(
    graph,
    selections,
    signal,
  );
}

export function recoveryRunGraph(
  settings: RuntimeBridgeSettings,
  graph: AlgoramGraph,
  expectedPlan: ExecutionPlan,
  allowedImplementationRefs: string[],
  selections: RecoverySelections,
  signal?: AbortSignal,
): Promise<RunResponse> {
  return createRuntimeBridgeClient(settings).recoveryRun(
    graph,
    expectedPlan,
    allowedImplementationRefs,
    selections,
    signal,
  );
}

export function emptyRecoverySelections(): RecoverySelections {
  return {
    route_overrides: [],
    implementation_overrides: [],
  };
}

export function hasExecutableRecoverySelection(
  selections: RecoverySelections,
): boolean {
  return (
    selections.route_overrides.length > 0 ||
    selections.implementation_overrides.length > 0
  );
}
