import { useEffect } from "react";
import type { AlgoramGraph } from "./algoram";
import type {
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
  onDismiss: () => void;
}

function commandText(program: string, args: string[]): string {
  return [program, ...args].join(" ");
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
  onDismiss,
}: ExecutionPanelProps) {
  useEffect(() => {
    function handleEscape(event: KeyboardEvent) {
      if (event.key !== "Escape" || phase === "running") {
        return;
      }
      const target = event.target;
      if (
        target instanceof HTMLInputElement ||
        target instanceof HTMLTextAreaElement ||
        target instanceof HTMLSelectElement
      ) {
        return;
      }
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
            <span>{preview.access_report.requirements.length} access requirements</span>
          </div>

          <div className="execution-requirements">
            <strong>Host access</strong>
            <p className="compact-hint">
              These actions run with ambient host-process authority. No
              fine-grained sandbox is implied.
            </p>
            <ul>
              {preview.access_report.requirements.map((requirement) => (
                <li key={requirement.step_id}>
                  <label className="execution-grant">
                    <input
                      type="checkbox"
                      checked={grantedImplementationRefs.has(
                        requirement.implementation_ref,
                      )}
                      onChange={(event) =>
                        onGrantChange(
                          requirement.implementation_ref,
                          event.target.checked,
                        )
                      }
                      disabled={phase === "running"}
                    />
                    <span>
                      <strong>{requirement.implementation_ref}</strong>
                      <small>{requirement.access_class}</small>
                    </span>
                  </label>
                  <code title={commandText(
                    requirement.action.program,
                    requirement.action.args,
                  )}>
                    {commandText(
                      requirement.action.program,
                      requirement.action.args,
                    )}
                  </code>
                  <small>
                    Blocks: {requirement.origin_block_ids.join(", ")}
                  </small>
                </li>
              ))}
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

      {preview || error || result ? (
        <div className="execution-panel-actions">
          <button
            type="button"
            className="tertiary-action"
            onClick={onDismiss}
            disabled={phase === "running"}
          >
            Close preview
          </button>
        </div>
      ) : null}

      <p className="compact-hint execution-graph-id">
        Current Graph: {graph.id}
      </p>
    </section>
  );
}
