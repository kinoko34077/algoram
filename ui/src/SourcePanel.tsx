import { useMemo } from "react";
import { jaJP } from "./locales/ja-JP";
import type {
  AlgoramBlock,
  AlgoramGraph,
  ReferenceBundle,
  SourceAnchor,
  SourceDocument,
} from "./algoram";

interface SourcePanelProps {
  bundle: ReferenceBundle;
  graph: AlgoramGraph;
  selectedBlock: AlgoramBlock | null;
  onSelectBlock: (blockId: string) => void;
}

interface SourceLine {
  number: number;
  text: string;
  startByte: number;
  endByte: number;
  probeByte: number | null;
}

const encoder = new TextEncoder();
const decoder = new TextDecoder();

function byteOffset(source: string, charOffset: number): number {
  return encoder.encode(source.slice(0, charOffset)).length;
}

function sourceLines(source: string): SourceLine[] {
  const lines: SourceLine[] = [];
  const pattern = /(.*?)(\r\n|\r|\n|$)/g;

  for (const match of source.matchAll(pattern)) {
    const whole = match[0];
    if (whole.length === 0) {
      break;
    }

    const text = match[1] ?? "";
    const charStart = match.index ?? 0;
    const charEnd = charStart + text.length;
    const firstVisible = text.search(/\S/);
    const probeChar = firstVisible >= 0 ? charStart + firstVisible : null;

    lines.push({
      number: lines.length + 1,
      text,
      startByte: byteOffset(source, charStart),
      endByte: byteOffset(source, charEnd),
      probeByte: probeChar === null ? null : byteOffset(source, probeChar),
    });
  }

  return lines;
}

function sourceDocumentFor(
  bundle: ReferenceBundle,
  graph: AlgoramGraph,
  selectedBlock: AlgoramBlock | null,
): SourceDocument | null {
  const selectedArtifact = selectedBlock?.source_anchor?.artifact_id;
  if (selectedArtifact && bundle.sources[selectedArtifact]) {
    return bundle.sources[selectedArtifact];
  }

  for (const artifact of graph.source_artifacts ?? []) {
    const document = bundle.sources[artifact.id];
    if (document) {
      return document;
    }
  }

  return null;
}

function byteSnippet(source: string, anchor: SourceAnchor): string {
  const bytes = encoder.encode(source);
  return decoder.decode(bytes.slice(anchor.start_byte, anchor.end_byte));
}

function rangeSize(anchor: SourceAnchor): number {
  return anchor.end_byte - anchor.start_byte;
}

function blockAtByte(
  blocks: AlgoramBlock[],
  artifactId: string,
  byte: number,
): AlgoramBlock | null {
  return (
    blocks
      .filter((block) => {
        const anchor = block.source_anchor;
        return (
          anchor?.artifact_id === artifactId &&
          anchor.start_byte <= byte &&
          byte < anchor.end_byte
        );
      })
      .sort((a, b) => {
        const aAnchor = a.source_anchor;
        const bAnchor = b.source_anchor;
        if (!aAnchor || !bAnchor) {
          return 0;
        }
        return rangeSize(aAnchor) - rangeSize(bAnchor);
      })[0] ?? null
  );
}

function lineIntersectsAnchor(line: SourceLine, anchor: SourceAnchor): boolean {
  const end = Math.max(line.endByte, line.startByte + 1);
  return anchor.start_byte < end && anchor.end_byte > line.startByte;
}

export function SourcePanel({
  bundle,
  graph,
  selectedBlock,
  onSelectBlock,
}: SourcePanelProps) {
  const document = sourceDocumentFor(bundle, graph, selectedBlock);
  const lines = useMemo(
    () => (document ? sourceLines(document.text) : []),
    [document],
  );

  if (!document) {
    return (
      <aside className="source-panel">
        <h2>{jaJP.authoring.sourcePanel.source}</h2>
        <p>{jaJP.authoring.sourcePanel.noSourceArtifact}</p>
      </aside>
    );
  }

  const selectedAnchor =
    selectedBlock?.source_anchor?.artifact_id === document.artifact.id
      ? selectedBlock.source_anchor
      : undefined;

  return (
    <aside className="source-panel">
      <div className="source-heading">
        <div>
          <h2>{jaJP.authoring.sourcePanel.source}</h2>
          <p>{document.artifact.origin}</p>
        </div>
        <span>{document.artifact.language}</span>
      </div>

      {selectedBlock && selectedAnchor ? (
        <section className="selection-card">
          <strong>{selectedBlock.label}</strong>
          <span>
            {jaJP.authoring.sourcePanel.bytes.replace("{start}", String(selectedAnchor.start_byte)).replace("{end}", String(selectedAnchor.end_byte))}
          </span>
          <pre>{byteSnippet(document.text, selectedAnchor)}</pre>
        </section>
      ) : (
        <p className="hint">{jaJP.authoring.sourcePanel.selectHint}</p>
      )}

      <div className="source-code" aria-label={jaJP.authoring.sourcePanel.sourceCode}>
        {lines.map((line) => {
          const highlighted =
            selectedAnchor !== undefined &&
            lineIntersectsAnchor(line, selectedAnchor);

          return (
            <button
              key={line.number}
              type="button"
              className={highlighted ? "source-line selected" : "source-line"}
              aria-pressed={highlighted}
              onClick={() => {
                if (line.probeByte === null) {
                  return;
                }
                const block = blockAtByte(
                  graph.blocks,
                  document.artifact.id,
                  line.probeByte,
                );
                if (block) {
                  onSelectBlock(block.id);
                }
              }}
            >
              <span className="line-number">{line.number}</span>
              <code>{line.text || " "}</code>
            </button>
          );
        })}
      </div>
    </aside>
  );
}
