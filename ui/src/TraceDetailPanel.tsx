import { jaJP } from "./locales/ja-JP";
import {
  observedStatusLabel,
  observedStatusSymbol,
  type BlockTraceObservation,
} from "./traceProjection";

interface TraceDetailPanelProps {
  observation: BlockTraceObservation;
}

function listOrDash(values: string[]): string {
  return values.length > 0 ? values.join(", ") : "—";
}

export function TraceDetailPanel({
  observation,
}: TraceDetailPanelProps) {
  return (
    <section
      className="trace-detail-panel"
      aria-label={jaJP.trace.selectedBlockObservedRun}
    >
      <div className="trace-detail-heading">
        <div>
          <p className="eyebrow">{jaJP.trace.observedRun}</p>
          <strong className={`observed-status ${observation.status}`}>
            <span aria-hidden="true">
              {observedStatusSymbol(observation.status)}
            </span>
            {observedStatusLabel(observation.status)}
          </strong>
        </div>
        <span>{jaJP.trace.stepCount.replace("{count}", String(observation.entries.length))}</span>
      </div>

      <dl className="trace-facts">
        <div>
          <dt>{jaJP.trace.step}</dt>
          <dd>{listOrDash(observation.stepIds)}</dd>
        </div>
        <div>
          <dt>{jaJP.trace.implementation}</dt>
          <dd>{listOrDash(observation.implementationRefs)}</dd>
        </div>
        {observation.routeConnectorIds.length > 0 ? (
          <div>
            <dt>{jaJP.trace.observedRoute}</dt>
            <dd>{observation.routeConnectorIds.join(", ")}</dd>
          </div>
        ) : null}
        {observation.exitCodes.length > 0 ? (
          <div>
            <dt>{jaJP.trace.exitCode}</dt>
            <dd>{observation.exitCodes.join(", ")}</dd>
          </div>
        ) : null}
      </dl>

      {observation.stderr.length > 0 ? (
        <div className="trace-stderr">
          <strong>stderr</strong>
          {observation.stderr.map((value, index) => (
            <pre key={`stderr:${index}`}>{value}</pre>
          ))}
        </div>
      ) : null}

      {observation.stdout.length > 0 ? (
        <details className="trace-output">
          <summary>stdout</summary>
          {observation.stdout.map((value, index) => (
            <pre key={`stdout:${index}`}>{value}</pre>
          ))}
        </details>
      ) : null}

      <p className="compact-hint">
        {jaJP.trace.evidenceOnlyHint}
      </p>
    </section>
  );
}
