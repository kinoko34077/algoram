import type { TraceProjection } from "./traceProjection";
import { jaJP } from "./locales/ja-JP";

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
        {jaJP.trace.unmappedOriginCount.replace("{count}", String(projection.unmappedOrigins.length))}
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
        {jaJP.trace.unmappedHint}
      </p>
    </details>
  );
}
