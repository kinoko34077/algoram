import { useEffect, useMemo } from "react";
import type { AlgoramGraph } from "./algoram";
import type {
  ExecutionAccessRequirement,
  PlanResponse,
  RunResponse,
  RuntimeBridgeSettings,
} from "./runtimeBridge";

export type ExecutionPhase =
  | "idle"
  | "planning"
  | "ready"
  | "running"
  | "succeeded"
  | "failed";

interface ExecutionPanelProps {
  graph: AlgoramGraph;
  phase: ExecutionPhase;
  settings: RuntimeBridgeSettings;
  preview: PlanResponse | null;
  result: RunResponse | null;
  error: string | null;
  grantedImplementationRefs: Set<string>;
  onSettingsChange: (settings: RuntimeBridgeSettings) => void;
  onGrantChange: (implementationRef: string, granted: boolean) => void;
  onRetryPlan: () => void;
  onDismiss: () => void;
}

interface GrantGroup {
  implementationRef: string;
  requirement: ExecutionAccessRequirement;
  stepIds: string[];
  blockIds: string[];
}

function commandText(program: string, args: string[]): string {
  return [program, ...args].join(" ");
}

function groupRequirements(
  requirements: ExecutionAccessRequirement[],
): GrantGroup[] {
  const groups = new Map<string, GrantGroup>();

  for (const requirement of requirements) {
    const existing = groups.get(requirement.implementation_ref);
    if (existing) {
      existing.stepIds.push(requirement.step_id);
      for (const blockId of requirement.origin_block_ids) {
        if (!existing.blockIds.includes(blockId)) {
          existing.blockIds.push(blockId);
        }
      }
      continue;
    }

    groups.set(requirement.implementation_ref, {
      implementationRef: requirement.implementation_ref,
      requirement,
      stepIds: [requirement.step_id],
      blockIds: [...new Set(requirement.origin_block_ids)],
    });
  }

  return [...groups.values()].sort((left, right) =>
    left.implementationRef.localeCompare(right.implementationRef),
  );
}

export function ExecutionPanel({
  graph,
  phase,
  settings,
  preview,
  result,
  error,
  grantedImplementationRefs,
  onSettingsChange,
  onGrantChange,
  onRetryPlan,
  onDismiss,
}: ExecutionPanelProps) {
  const grantGroups = useMemo(
    () => groupRequirements(preview?.access_report.requirements ?? []),
    [preview],
  );

  useEffect(() => {
    function handleEscape(event: KeyboardEvent) {
      if (
        event.key !== "Escape" ||
        event.isComposing ||
        phase === "idle" ||
        phase === "running"
      ) {
        return;
      }

      event.preventDefault();
      onDismiss();
    }

    window.addEventListener("keydown", handleEscape);
    return () => window.removeEventListener("keydown", handleEscape);
  }, [onDismiss, phase]);

  return (
    <section className="execution-panel" aria-label="Runtime execution">
      <div className="panel-heading-row">
        <div>
          <p className="eyebrow">Runtime</p>
          <strong>Guarded execution</strong>
        </div>
        <span className={`execution-state ${phase}`} aria-live="polite">
          {phase}
        </span>
      </div>

      <details className="bridge-settings">
        <summary>Bridge connection</summary>
        <div className="bridge-settings-fields">
          <label>
            Bridge URL
            <input
              type="url"
              value={settings.baseUrl}
              inputMode="url"
              autoComplete="off"
              disabled={phase === "running"}
              onChange={(event) =>
                onSettingsChange({
                  ...settings,
                  baseUrl: event.target.value,
                })
              }
            />
          </label>
          <label>
            Bearer token
            <input
              type="password"
              value={settings.bearerToken}
              autoComplete="off"
              disabled={phase === "running"}
              onChange={(event) =>
                onSettingsChange({
                  ...settings,
                  bearerToken: event.target.value,
                })
              }
            />
          </label>
          <p className="compact-hint">
            Session only. The browser bridge is restricted to loopback
            localhost and the token is never written into Graph export.
          </p>
        </div>
      </details>

      {phase === "idle" ? (
        <p className="compact-hint">
          Plan the current Graph to inspect concrete host-process access before
          execution.
        </p>
      ) : null}

      {phase === "planning" ? (
        <p className="inline-status" role="status">
          Validating and planning on the trusted host…
        </p>
      ) : null}

      {phase === "running" ? (
        <p className="inline-status" role="status">
          Running through the guarded host runtime…
        </p>
      ) : null}

      {error ? (
        <div className="execution-error" role="alert">
          <strong>Execution blocked</strong>
          <p>{error}</p>
        </div>
      ) : null}

      {preview ? (
        <div className="execution-preview">
          <div className="execution-summary">
            <span>{preview.plan.steps.length} steps</span>
            <span>
              {preview.access_report.requirements.length} access requirements
            </span>
            <span>{grantGroups.length} grants</span>
          </div>

          <div className="execution-requirements">
            <strong>Host access</strong>
            <p className="compact-hint">
              These actions run with ambient host-process authority. No
              fine-grained sandbox is implied.
            </p>
            <ul>
              {grantGroups.map((group) => {
                const { requirement } = group;
                return (
                  <li key={group.implementationRef}>
                    <label className="execution-grant">
                      <input
                        type="checkbox"
                        checked={grantedImplementationRefs.has(
                          group.implementationRef,
                        )}
                        onChange={(event) =>
                          onGrantChange(
                            group.implementationRef,
                            event.target.checked,
                          )
                        }
                        disabled={phase === "running"}
                      />
                      <span>
                        <strong>{group.implementationRef}</strong>
                        <small>{requirement.access_class}</small>
                      </span>
                    </label>
                    <code
                      title={commandText(
                        requirement.action.program,
                        requirement.action.args,
                      )}
                    >
                      {commandText(
                        requirement.action.program,
                        requirement.action.args,
                      )}
                    </code>
                    {requirement.action.current_dir ? (
                      <small>
                        Working directory: {requirement.action.current_dir}
                      </small>
                    ) : null}
                    <small>
                      Blocks: {group.blockIds.join(", ") || "none"}
                    </small>
                    <small>
                      Steps: {group.stepIds.join(", ")}
                    </small>
                  </li>
                );
              })}
            </ul>
          </div>
        </div>
      ) : null}

      {result ? (
        <div className="execution-result" role="status">
          <strong>
            {result.trace.entries.every(
              (entry) => entry.status === "succeeded",
            )
              ? "Run completed"
              : "Run finished with failure"}
          </strong>
          <ul>
            {result.trace.entries.map((entry) => (
              <li key={entry.step_id}>
                <span>{entry.step_id}</span>
                <strong>{entry.status}</strong>
                {entry.stderr.trim() ? <small>{entry.stderr.trim()}</small> : null}
              </li>
            ))}
          </ul>
        </div>
      ) : null}

      {preview || error || result || phase === "planning" ? (
        <div className="execution-panel-actions">
          {phase === "failed" ? (
            <button
              type="button"
              className="secondary-action"
              onClick={onRetryPlan}
            >
              Retry plan
            </button>
          ) : null}
          <button
            type="button"
            className="tertiary-action"
            onClick={onDismiss}
            disabled={phase === "running"}
          >
            {phase === "planning" ? "Cancel planning" : "Close preview"}
          </button>
        </div>
      ) : null}

      <p className="compact-hint execution-graph-id">
        Current Graph: {graph.id}
      </p>
    </section>
  );
}
