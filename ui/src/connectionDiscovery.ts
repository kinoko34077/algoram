import type { AlgoramBlock, AlgoramPort } from "./algoram.ts";

/**
 * Discovery is NOT compatibility. Even identical Port contract strings do
 * not prove an actual usable Interop Route or source-language validity.
 */
export type ConnectionDiscoveryStatus =
  | "select-output"
  | "candidate-unverified"
  | "needs-mapper-or-route"
  | "insufficient-information"
  | "no-direct-input";

export function discoverConnectionStatus(
  output: AlgoramPort | undefined,
  target: AlgoramBlock,
): ConnectionDiscoveryStatus {
  if (!output || output.direction !== "out") {
    return "select-output";
  }
  const directInputs = (target.ports ?? []).filter(
    (port) => port.direction === "in" && port.channel === output.channel,
  );
  if (!directInputs.length) {
    return "no-direct-input";
  }
  const src = output.contract;
  if (typeof src !== "string" || directInputs.some((p) => typeof p.contract !== "string")) {
    return "insufficient-information";
  }
  if (directInputs.some((p) => p.contract === src)) {
    return "candidate-unverified";
  }
  return "needs-mapper-or-route";
}
