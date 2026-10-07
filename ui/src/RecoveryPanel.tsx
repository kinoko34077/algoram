import { useEffect, useMemo, useState } from "react";
import { jaJP } from "./locales/ja-JP";
import type {
  RecoveryOptionsResponse,
  RecoverySelections,
  RuntimeRecoveryOption,
} from "./runtimeBridge";
import { hasExecutableRecoverySelection } from "./runtimeBridge";
import {
  buildRecoverySelections,
  recoveryRouteCandidateKey,
  runtimeSelectionBlocksApply,
} from "./recoverySelection";

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

function runtimeOptionLabel(option: RuntimeRecoveryOption): string {
  const parts = [option.candidate.runtime_ref, option.candidate.class];
  if (option.candidate.is_current) {
    parts.push(jaJP.common.states.current);
  }
  if (!option.placement_validated) {
    parts.push(jaJP.recovery.invalidPlacement);
  } else if (!option.executable_by_bridge) {
    parts.push(jaJP.recovery.bridgeUnavailable);
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

  const selections = useMemo<RecoverySelections>(
    () =>
      buildRecoverySelections(
        options,
        routeSelections,
        providerSelections,
      ),
    [options, providerSelections, routeSelections],
  );

  const runtimeBlocksApply = runtimeSelectionBlocksApply(
    options,
    runtimeSelections,
  );
  const canApply =
    !disabled &&
    !runtimeBlocksApply &&
    hasExecutableRecoverySelection(selections);

  if (phase === "idle" && !options && !error) {
    return null;
  }

  return (
    <section className="recovery-panel" aria-label={jaJP.recovery.manualRecovery}>
      <div className="panel-heading-row">
        <div>
          <p className="eyebrow">{jaJP.recovery.recovery}</p>
          <strong>{jaJP.recovery.manualAlternatives}</strong>
        </div>
        <span className={`recovery-state ${phase}`} aria-live="polite">
          {jaJP.common.states[phase]}
        </span>
      </div>

      {phase === "loading" ? (
        <p className="inline-status" role="status">
          {jaJP.recovery.loading}
        </p>
      ) : null}

      {error ? (
        <div className="execution-error" role="alert">
          <strong>{jaJP.recovery.unavailable}</strong>
          <p>{error}</p>
        </div>
      ) : null}

      {options ? (
        <>
          <div className="recovery-impact" aria-label={jaJP.recovery.observedImpact}>
            <span>
              {jaJP.recovery.failedCount.replace("{count}", String(options.report.impact.failed_block_ids.length))}
            </span>
            <span>
              {jaJP.recovery.affectedCount.replace("{count}", String(options.report.impact.affected_block_ids.length))}
            </span>
          </div>
          <p className="compact-hint">
            {jaJP.recovery.observedImpactHint}
          </p>

          {options.route_candidates.length > 0 ? (
            <fieldset className="recovery-group">
              <legend>{jaJP.recovery.route}</legend>
              {options.route_candidates.map((candidate) => {
                const key = recoveryRouteCandidateKey(
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
              <legend>{jaJP.recovery.provider}</legend>
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
                    disabled={
                      disabled ||
                      row.candidate.implementation_ref ===
                        row.selectedImplementationRef
                    }
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
                        ? ` · ${jaJP.recovery.currentPlan}`
                        : ""}
                      {row.candidate.is_default ? ` · ${jaJP.recovery.hostDefault}` : ""}
                    </small>
                  </span>
                </label>
              ))}
              {!options.catalog_connected ? (
                <p className="compact-hint">
                  {jaJP.recovery.marketplaceDisconnected}
                </p>
              ) : null}
            </fieldset>
          ) : null}

          {runtimeRows.length > 0 ? (
            <fieldset className="recovery-group">
              <legend>{jaJP.recovery.runtime}</legend>
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
              {jaJP.recovery.runtimeNotExecutable}
            </p>
          ) : null}

          {options.route_candidates.length === 0 &&
          providerRows.length === 0 &&
          runtimeRows.length === 0 ? (
            <p className="compact-hint">
              {jaJP.recovery.noTrustedCandidate}
            </p>
          ) : null}

          <div className="recovery-actions">
            <button
              type="button"
              className="primary-action"
              disabled={!canApply}
              onClick={() => onApply(selections)}
            >
              {jaJP.recovery.applyAndReplan}
            </button>
          </div>
        </>
      ) : null}
    </section>
  );
}
