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
      aria-label="Observed run for selected Block"
    >
      <div className="trace-detail-heading">
        <div>
          <p className="eyebrow">Observed run</p>
          <strong className={`observed-status ${observation.status}`}>
            <span aria-hidden="true">
              {observedStatusSymbol(observation.status)}
            </span>
            {observedStatusLabel(observation.status)}
          </strong>
        </div>
        <span>{observation.entries.length} step{observation.entries.length === 1 ? "" : "s"}</span>
      </div>

      <dl className="trace-facts">
        <div>
          <dt>Step</dt>
          <dd>{listOrDash(observation.stepIds)}</dd>
        </div>
        <div>
          <dt>Implementation</dt>
          <dd>{listOrDash(observation.implementationRefs)}</dd>
        </div>
        {observation.routeConnectorIds.length > 0 ? (
          <div>
            <dt>Observed route</dt>
            <dd>{observation.routeConnectorIds.join(", ")}</dd>
          </div>
        ) : null}
        {observation.exitCodes.length > 0 ? (
          <div>
            <dt>Exit code</dt>
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
        Observed execution evidence only. Route IDs are runtime evidence, not
        canonical Graph Connection IDs.
      </p>
    </section>
  );
}
