import type {
  AlternativeRouteCandidate,
  RecoveryOptionsResponse,
  RecoverySelections,
  RuntimeRecoveryOption,
} from "./runtimeBridge";

export type RouteSelectionState = Record<string, string>;
export type ProviderSelectionState = Record<string, string>;
export type RuntimeSelectionState = Record<string, string>;

export function recoveryRouteCandidateKey(
  connectionId: string,
  connectorIds: string[],
): string {
  return `${connectionId}::${connectorIds.join(">")}`;
}

export function buildRecoverySelections(
  options: RecoveryOptionsResponse | null,
  routeSelections: RouteSelectionState,
  providerSelections: ProviderSelectionState,
): RecoverySelections {
  if (!options) {
    return {
      route_overrides: [],
      implementation_overrides: [],
    };
  }

  const routeByKey = new Map<string, AlternativeRouteCandidate>();
  for (const candidate of options.route_candidates) {
    routeByKey.set(
      recoveryRouteCandidateKey(
        candidate.connection_id,
        candidate.connector_ids,
      ),
      candidate,
    );
  }

  const route_overrides = Object.entries(routeSelections)
    .flatMap(([, key]) => {
      const candidate = routeByKey.get(key);
      return candidate
        ? [
            {
              connection_id: candidate.connection_id,
              connector_ids: [...candidate.connector_ids],
            },
          ]
        : [];
    })
    .sort((left, right) =>
      left.connection_id.localeCompare(right.connection_id),
    );

  const implementation_overrides = Object.entries(providerSelections)
    .filter(([, implementationRef]) => implementationRef.length > 0)
    .map(([logicalRef, implementationRef]) => ({
      logical_ref: logicalRef,
      implementation_ref: implementationRef,
    }))
    .sort((left, right) => left.logical_ref.localeCompare(right.logical_ref));

  return {
    route_overrides,
    implementation_overrides,
  };
}

export interface SelectedRuntimeRecovery {
  stepId: string;
  option: RuntimeRecoveryOption;
}

export function selectedRuntimeRecovery(
  options: RecoveryOptionsResponse | null,
  runtimeSelections: RuntimeSelectionState,
): SelectedRuntimeRecovery[] {
  if (!options) {
    return [];
  }

  const selected: SelectedRuntimeRecovery[] = [];
  for (const step of options.runtime_candidates) {
    const runtimeRef = runtimeSelections[step.step_id];
    if (!runtimeRef) {
      continue;
    }
    const option = step.candidates.find(
      (candidate) => candidate.candidate.runtime_ref === runtimeRef,
    );
    if (option) {
      selected.push({
        stepId: step.step_id,
        option,
      });
    }
  }

  return selected;
}

export function runtimeSelectionBlocksApply(
  options: RecoveryOptionsResponse | null,
  runtimeSelections: RuntimeSelectionState,
): boolean {
  return selectedRuntimeRecovery(options, runtimeSelections).some(
    ({ option }) =>
      !option.placement_validated || !option.executable_by_bridge,
  );
}
