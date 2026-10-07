import { useEffect, useMemo, useState } from "react";
import type {
  AlternativeRouteCandidate,
  RecoveryOptionsResponse,
  RecoverySelections,
  RuntimeRecoveryOption,
} from "./runtimeBridge";
import {
  emptyRecoverySelections,
  hasExecutableRecoverySelection,
} from "./runtimeBridge";

export type RecoveryDiscoveryPhase =
  | "idle"
  | "loading"
  | "ready"
  | "failed";

interface RecoveryPanelProps {
  options: RecoveryOptionsResponse | null;
  phase: RecoveryDiscoveryPhase;
  error: string | null;
  disabled: boolean;
  onApply: (selections: RecoverySelections) => void;
}

function candidateKey(connectionId: string, connectorIds: string[]): string {
  return `${connectionId}::${connectorIds.join(">")}`;
}

function runtimeOptionLabel(option: RuntimeRecoveryOption): string {
  const parts = [option.candidate.runtime_ref, option.candidate.class];
  if (option.candidate.is_current) {
    parts.push("current");
  }
  if (!option.placement_validated) {
    parts.push("invalid placement");
  } else if (!option.executable_by_bridge) {
    parts.push("bridge unavailable");
  }
  return parts.join(" · ");
}

export function RecoveryPanel({
  options,
  phase,
  error,
  disabled,
  onApply,
}: RecoveryPanelProps) {
  const [routeSelections, setRouteSelections] = useState<
    Record<string, string>
  >({});
  const [providerSelections, setProviderSelections] = useState<
    Record<string, string>
  >({});
  const [runtimeSelections, setRuntimeSelections] = useState<
    Record<string, string>
  >({});

  useEffect(() => {
    setRouteSelections({});
    setProviderSelections({});
    setRuntimeSelections({});
  }, [options]);

  const routeByKey = useMemo(() => {
    const candidates = new Map<string, AlternativeRouteCandidate>();
    for (const candidate of options?.route_candidates ?? []) {
      candidates.set(
        candidateKey(candidate.connection_id, candidate.connector_ids),
        candidate,
      );
    }
    return candidates;
  }, [options]);

  const providerRows = useMemo(
    () =>
      options?.implementation_candidates.flatMap((step) =>
        step.candidates.trusted_local.map((candidate) => ({
          stepId: step.step_id,
          logicalRef: step.candidates.logical_implementation_ref,
          selectedImplementationRef: step.selected_implementation_ref,
          candidate,
        })),
      ) ?? [],
    [options],
  );

  const runtimeRows = useMemo(
    () =>
      options?.runtime_candidates.flatMap((step) =>
        step.candidates.map((option) => ({
          stepId: step.step_id,
          currentRuntimeRef: step.current_runtime_ref,
          option,
        })),
      ) ?? [],
    [options],
  );

  const selections = useMemo<RecoverySelections>(() => {
    if (!options) {
      return emptyRecoverySelections();
    }

    const route_overrides = Object.entries(routeSelections)
      .flatMap(([, key]) => {
        const candidate = routeByKey.get(key);
        return candidate
          ? [
              {
                connection_id: candidate.connection_id,
                connector_ids: candidate.connector_ids,
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
  }, [options, providerSelections, routeByKey, routeSelections]);

  const selectedRuntimeRows = useMemo(
    () =>
      Object.entries(runtimeSelections)
        .map(([stepId, runtimeRef]) =>
          runtimeRows.find(
            (row) =>
              row.stepId === stepId &&
              row.option.candidate.runtime_ref === runtimeRef,
          ),
        )
        .filter((row) => row !== undefined),
    [runtimeRows, runtimeSelections],
  );

  const runtimeBlocksApply = selectedRuntimeRows.some(
    (row) =>
      !row.option.placement_validated || !row.option.executable_by_bridge,
  );
  const canApply =
    !disabled &&
    !runtimeBlocksApply &&
    hasExecutableRecoverySelection(selections);

  if (phase === "idle" && !options && !error) {
    return null;
  }

  return (
    <section className="recovery-panel" aria-label="Manual recovery">
      <div className="panel-heading-row">
        <div>
          <p className="eyebrow">Recovery</p>
          <strong>Manual alternatives</strong>
        </div>
        <span className={`recovery-state ${phase}`} aria-live="polite">
          {phase}
        </span>
      </div>

      {phase === "loading" ? (
        <p className="inline-status" role="status">
          Inspecting observed failure and trusted alternatives…
        </p>
      ) : null}

      {error ? (
        <div className="execution-error" role="alert">
          <strong>Recovery unavailable</strong>
          <p>{error}</p>
        </div>
      ) : null}

      {options ? (
        <>
          <div className="recovery-impact" aria-label="Observed impact">
            <span>
              Failed <strong>{options.report.impact.failed_block_ids.length}</strong>
            </span>
            <span>
              Affected{" "}
              <strong>{options.report.impact.affected_block_ids.length}</strong>
            </span>
          </div>
          <p className="compact-hint">
            Observed from the failed Trace. Choosing an option changes no
            canonical Graph data and does nothing until Apply &amp; re-plan.
          </p>

          {options.route_candidates.length > 0 ? (
            <fieldset className="recovery-group">
              <legend>Route</legend>
              {options.route_candidates.map((candidate) => {
                const key = candidateKey(
                  candidate.connection_id,
                  candidate.connector_ids,
                );
                return (
                  <label className="recovery-choice" key={key}>
                    <input
                      type="radio"
                      name={`recovery-route-${candidate.connection_id}`}
                      checked={
                        routeSelections[candidate.connection_id] === key
                      }
                      disabled={disabled}
                      onChange={() =>
                        setRouteSelections((current) => ({
                          ...current,
                          [candidate.connection_id]: key,
                        }))
                      }
                    />
                    <span>
                      <strong>{candidate.connection_id}</strong>
                      <small>
                        {candidate.source_contract} → {candidate.target_contract}
                      </small>
                      <code>{candidate.connector_ids.join(" → ")}</code>
                    </span>
                  </label>
                );
              })}
            </fieldset>
          ) : null}

          {providerRows.length > 0 ? (
            <fieldset className="recovery-group">
              <legend>Provider</legend>
              {providerRows.map((row) => (
                <label
                  className="recovery-choice"
                  key={`${row.stepId}:${row.logicalRef}:${row.candidate.implementation_ref}`}
                >
                  <input
                    type="radio"
                    name={`recovery-provider-${row.logicalRef}`}
                    checked={
                      providerSelections[row.logicalRef] ===
                      row.candidate.implementation_ref
                    }
                    disabled={disabled}
                    onChange={() =>
                      setProviderSelections((current) => ({
                        ...current,
                        [row.logicalRef]: row.candidate.implementation_ref,
                      }))
                    }
                  />
                  <span>
                    <strong>{row.candidate.implementation_ref}</strong>
                    <small>
                      {row.logicalRef}
                      {row.candidate.implementation_ref ===
                      row.selectedImplementationRef
                        ? " · current plan"
                        : ""}
                      {row.candidate.is_default ? " · host default" : ""}
                    </small>
                  </span>
                </label>
              ))}
              {!options.catalog_connected ? (
                <p className="compact-hint">
                  Marketplace catalog is not connected to this bridge; only
                  host-trusted local providers are shown.
                </p>
              ) : null}
            </fieldset>
          ) : null}

          {runtimeRows.length > 0 ? (
            <fieldset className="recovery-group">
              <legend>Runtime</legend>
              {runtimeRows.map((row) => (
                <label
                  className="recovery-choice"
                  key={`${row.stepId}:${row.option.candidate.runtime_ref}`}
                >
                  <input
                    type="radio"
                    name={`recovery-runtime-${row.stepId}`}
                    checked={
                      runtimeSelections[row.stepId] ===
                      row.option.candidate.runtime_ref
                    }
                    disabled={disabled}
                    onChange={() =>
                      setRuntimeSelections((current) => ({
                        ...current,
                        [row.stepId]: row.option.candidate.runtime_ref,
                      }))
                    }
                  />
                  <span>
                    <strong>{runtimeOptionLabel(row.option)}</strong>
                    <small>{row.stepId}</small>
                  </span>
                </label>
              ))}
            </fieldset>
          ) : null}

          {runtimeBlocksApply ? (
            <p className="inline-status error-text" role="alert">
              The selected runtime placement is not executable by this local
              bridge. No local fallback will be used. Connect distributed
              runtime transport or choose the current local runtime.
            </p>
          ) : null}

          {options.route_candidates.length === 0 &&
          providerRows.length === 0 &&
          runtimeRows.length === 0 ? (
            <p className="compact-hint">
              No trusted recovery candidate is available for the observed
              failure.
            </p>
          ) : null}

          <div className="recovery-actions">
            <button
              type="button"
              className="primary-action"
              disabled={!canApply}
              onClick={() => onApply(selections)}
            >
              Apply &amp; re-plan
            </button>
          </div>
        </>
      ) : null}
    </section>
  );
}
