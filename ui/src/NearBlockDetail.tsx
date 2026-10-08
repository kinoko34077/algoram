import type { AlgoramBlock } from "./algoram";
import type { BlockTraceObservation } from "./traceProjection";
import { observedStatusLabel } from "./traceProjection";
import { jaJP } from "./locales/ja-JP";

interface Props {
  block: AlgoramBlock;
  observation?: BlockTraceObservation;
  onOpenComposite?: (graphId: string, viaBlock: AlgoramBlock) => void;
  onShowInspector?: () => void;
}

/** Read-only near-detail is a projection of canonical Graph and actual Trace. */
export function NearBlockDetail({
  block, observation, onOpenComposite, onShowInspector,
}: Props) {
  const ports = block.ports ?? [];
  const inputs = ports.filter((port) => port.direction === "in").length;
  const outputs = ports.length - inputs;
  return (
    <section
      className="near-block-detail nodrag nopan"
      aria-label={jaJP.authoring.canvas.nearDetailTitle}
      onPointerDown={(event) => event.stopPropagation()}
      onClick={(event) => event.stopPropagation()}
    >
      <strong>{block.label}</strong>
      {ports.length > 0 ? (
        <p className="near-block-ports">
          {jaJP.authoring.blockPalette.ioSummary
            .replace("{inputs}", String(inputs))
            .replace("{outputs}", String(outputs))}
        </p>
      ) : null}
      {observation ? (
        <div className="near-block-observation">
          <span>{jaJP.authoring.canvas.observedRun.replace(
            "{status}", observedStatusLabel(observation.status)
          )}</span>
          {observation.stdout.map((stdout, index) => (
            <pre key={index} className="near-block-stdout">
              {stdout}
            </pre>
          ))}
        </div>
      ) : null}
      <div className="near-block-actions">
        {block.internal_graph_ref && onOpenComposite ? (
          <button type="button" className="secondary-action"
            onClick={() => onOpenComposite(block.internal_graph_ref!, block)}
          >
            {jaJP.authoring.canvas.openInside}
          </button>
        ) : null}
        {onShowInspector ? (
          <button type="button" className="tertiary-action" onClick={onShowInspector}>
            {jaJP.authoring.canvas.moreDetails}
          </button>
        ) : null}
      </div>
    </section>
  );
}
