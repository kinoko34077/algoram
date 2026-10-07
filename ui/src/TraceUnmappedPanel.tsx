import type { TraceProjection } from "./traceProjection";

interface TraceUnmappedPanelProps {
  projection: TraceProjection;
}

export function TraceUnmappedPanel({
  projection,
}: TraceUnmappedPanelProps) {
  if (projection.unmappedOrigins.length === 0) {
    return null;
  }

  return (
    <details className="trace-unmapped-panel">
      <summary>
        {projection.unmappedOrigins.length} unmapped trace origin
        {projection.unmappedOrigins.length === 1 ? "" : "s"}
      </summary>
      <ul>
        {projection.unmappedOrigins.map((evidence, index) => (
          <li key={`${evidence.stepId}:${index}`}>
            <strong>{evidence.stepId}</strong>
            <span>{evidence.originBlockIds.join(", ")}</span>
          </li>
        ))}
      </ul>
      <p className="compact-hint">
        Observed evidence only. Unknown origin IDs are not attached to a
        visible Block by inference.
      </p>
    </details>
  );
}
