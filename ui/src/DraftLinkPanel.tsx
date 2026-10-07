import { useEffect, useMemo, useState } from "react";
import type { AlgoramBlock, AlgoramGraph } from "./algoram";
import {
  makeDraftLink,
  type DraftLink,
  type GraphPresentationState,
} from "./presentation";

type ConnectionMode = "canonical" | "draft";

interface DraftLinkPanelProps {
  graph: AlgoramGraph;
  selectedBlock: AlgoramBlock;
  presentation: GraphPresentationState;
  mode: ConnectionMode;
  validateCandidate: (link: DraftLink) => string | null;
  onAdd: (link: DraftLink) => string | null;
}

export function DraftLinkPanel({
  graph,
  selectedBlock,
  presentation,
  mode,
  validateCandidate,
  onAdd,
}: DraftLinkPanelProps) {
  const outputs = useMemo(
    () =>
      (selectedBlock.ports ?? []).filter(
        (port) => port.direction === "out",
      ),
    [selectedBlock],
  );
  const [sourcePortId, setSourcePortId] = useState(outputs[0]?.id ?? "");
  const [targetBlockId, setTargetBlockId] = useState("");
  const [targetPortId, setTargetPortId] = useState("");
  const [status, setStatus] = useState<string | null>(null);

  useEffect(() => {
    setSourcePortId(outputs[0]?.id ?? "");
    setTargetBlockId("");
    setTargetPortId("");
    setStatus(null);
  }, [outputs, selectedBlock.id]);

  const sourcePort = outputs.find((port) => port.id === sourcePortId);

  const targetBlocks = useMemo(() => {
    if (!sourcePort) {
      return [];
    }

    return graph.blocks.filter((block) =>
      (block.ports ?? []).some((port) => {
        if (port.direction !== "in" || port.channel !== sourcePort.channel) {
          return false;
        }

        const link = makeDraftLink(
          selectedBlock.id,
          sourcePort.id,
          block.id,
          port.id,
        );
        return validateCandidate(link) === null;
      }),
    );
  }, [graph, selectedBlock.id, sourcePort, validateCandidate, presentation]);

  useEffect(() => {
    if (
      targetBlockId &&
      targetBlocks.some((block) => block.id === targetBlockId)
    ) {
      return;
    }

    setTargetBlockId(targetBlocks[0]?.id ?? "");
  }, [targetBlockId, targetBlocks]);

  const targetPorts = useMemo(() => {
    if (!sourcePort || !targetBlockId) {
      return [];
    }

    const target = graph.blocks.find((block) => block.id === targetBlockId);
    return (target?.ports ?? []).filter((port) => {
      if (port.direction !== "in" || port.channel !== sourcePort.channel) {
        return false;
      }

      const link = makeDraftLink(
        selectedBlock.id,
        sourcePort.id,
        targetBlockId,
        port.id,
      );
      return validateCandidate(link) === null;
    });
  }, [
    graph,
    selectedBlock.id,
    sourcePort,
    targetBlockId,
    validateCandidate,
    presentation,
  ]);

  useEffect(() => {
    if (
      targetPortId &&
      targetPorts.some((port) => port.id === targetPortId)
    ) {
      return;
    }

    setTargetPortId(targetPorts[0]?.id ?? "");
  }, [targetPortId, targetPorts]);

  const title = mode === "canonical" ? "Connection" : "Draft link";
  const scope = mode === "canonical" ? "canonical Graph" : "presentation only";

  if (outputs.length === 0) {
    return (
      <section className="draft-link-panel" aria-label={title}>
        <div className="panel-heading-row">
          <strong>{title}</strong>
          <span>{scope}</span>
        </div>
        <p className="compact-hint">This Block has no output ports.</p>
      </section>
    );
  }

  const canAdd = Boolean(sourcePortId && targetBlockId && targetPortId);

  return (
    <section className="draft-link-panel" aria-label={title}>
      <div className="panel-heading-row">
        <strong>{title}</strong>
        <span>{scope}</span>
      </div>

      <form
        className="draft-link-form"
        onSubmit={(event) => {
          event.preventDefault();
          if (!canAdd) {
            return;
          }

          const link = makeDraftLink(
            selectedBlock.id,
            sourcePortId,
            targetBlockId,
            targetPortId,
          );
          const error = onAdd(link);
          setStatus(
            error ??
              (mode === "canonical"
                ? "Canonical Connection created."
                : "Draft link added."),
          );
        }}
      >
        <label>
          <span>Output</span>
          <select
            value={sourcePortId}
            onChange={(event) => {
              setSourcePortId(event.target.value);
              setStatus(null);
            }}
          >
            {outputs.map((port) => (
              <option key={port.id} value={port.id}>
                {port.id} · {port.channel}
              </option>
            ))}
          </select>
        </label>

        <label>
          <span>Target Block</span>
          <select
            value={targetBlockId}
            onChange={(event) => {
              setTargetBlockId(event.target.value);
              setStatus(null);
            }}
            disabled={targetBlocks.length === 0}
          >
            {targetBlocks.length === 0 ? (
              <option value="">No compatible target</option>
            ) : null}
            {targetBlocks.map((block) => (
              <option key={block.id} value={block.id}>
                {block.label}
              </option>
            ))}
          </select>
        </label>

        <label>
          <span>Input</span>
          <select
            value={targetPortId}
            onChange={(event) => {
              setTargetPortId(event.target.value);
              setStatus(null);
            }}
            disabled={targetPorts.length === 0}
          >
            {targetPorts.length === 0 ? (
              <option value="">No compatible input</option>
            ) : null}
            {targetPorts.map((port) => (
              <option key={port.id} value={port.id}>
                {port.id} · {port.channel}
              </option>
            ))}
          </select>
        </label>

        <button type="submit" className="secondary-action" disabled={!canAdd}>
          {mode === "canonical" ? "Create Connection" : "Add draft"}
        </button>
      </form>

      <p className="compact-hint">
        {mode === "canonical"
          ? "Keyboard alternative to handle dragging. Creates one canonical Graph history step."
          : "Presentation-only planning. Canonical Graph data is not changed."}
      </p>
      {status ? (
        <p className="inline-status" role="status" aria-live="polite">
          {status}
        </p>
      ) : null}
    </section>
  );
}
