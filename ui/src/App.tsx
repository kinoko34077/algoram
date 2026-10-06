import { useMemo, useState } from "react";
import type { AlgoramBlock } from "./algoram";
import { demoBundle } from "./fixture";
import { GraphCanvas } from "./GraphCanvas";
import { SourcePanel } from "./SourcePanel";

interface Breadcrumb {
  graphId: string;
  label: string;
}

function graphLabel(graphId: string): string {
  return demoBundle.graphs[graphId]?.label ?? graphId;
}

export function App() {
  const [path, setPath] = useState<Breadcrumb[]>([
    {
      graphId: demoBundle.rootGraphId,
      label: graphLabel(demoBundle.rootGraphId),
    },
  ]);
  const [selectedBlockId, setSelectedBlockId] = useState<string | null>(null);

  const currentGraphId = path.at(-1)?.graphId ?? demoBundle.rootGraphId;
  const currentGraph = demoBundle.graphs[currentGraphId];

  if (!currentGraph) {
    throw new Error(`Missing graph fixture: ${currentGraphId}`);
  }

  const selectedBlock = useMemo(
    () =>
      currentGraph.blocks.find((block) => block.id === selectedBlockId) ?? null,
    [currentGraph, selectedBlockId],
  );

  const selectedRouteGraphId =
    selectedBlockId === null
      ? undefined
      : demoBundle.routeInspections?.[selectedBlockId];

  function openGraphWithLabel(graphId: string, label: string) {
    if (!demoBundle.graphs[graphId]) {
      return;
    }

    setPath((current) => [
      ...current,
      {
        graphId,
        label,
      },
    ]);
    setSelectedBlockId(null);
  }

  function openGraph(graphId: string, viaBlock: AlgoramBlock) {
    openGraphWithLabel(graphId, viaBlock.label);
  }

  function jumpTo(index: number) {
    setPath((current) => current.slice(0, index + 1));
    setSelectedBlockId(null);
  }

  return (
    <div className="app-shell">
      <header className="app-header">
        <div>
          <p className="eyebrow">ALGOram / Reference Graph</p>
          <h1>{currentGraph.label ?? currentGraph.id}</h1>
        </div>
        <div className="header-status">
          <strong>{currentGraph.blocks.length}</strong>
          <span>visible Blocks</span>
          <small>Only the current hierarchy level is materialized.</small>
        </div>
      </header>

      <nav className="breadcrumbs" aria-label="Graph hierarchy">
        {path.map((entry, index) => (
          <button
            key={`${entry.graphId}:${index}`}
            type="button"
            onClick={() => jumpTo(index)}
            aria-current={index === path.length - 1 ? "page" : undefined}
          >
            {entry.label}
          </button>
        ))}
      </nav>

      <main className="workspace">
        <section className="graph-region">
          <GraphCanvas
            graph={currentGraph}
            selectedBlockId={selectedBlockId}
            onSelectBlock={setSelectedBlockId}
            onOpenGraph={openGraph}
          />
        </section>

        <section className="inspector-region">
          <div className="block-inspector">
            <div>
              <p className="eyebrow">Current level</p>
              <strong>{currentGraph.id}</strong>
            </div>

            {selectedBlock ? (
              <>
                <h2>{selectedBlock.label}</h2>
                <p className="block-id">{selectedBlock.id}</p>
                <div className="inspector-actions">
                  {selectedBlock.internal_graph_ref ? (
                    <button
                      type="button"
                      onClick={() =>
                        openGraph(
                          selectedBlock.internal_graph_ref as string,
                          selectedBlock,
                        )
                      }
                    >
                      Open internal graph
                    </button>
                  ) : null}
                  {selectedRouteGraphId ? (
                    <button
                      type="button"
                      onClick={() =>
                        openGraphWithLabel(
                          selectedRouteGraphId,
                          "Actual selected route",
                        )
                      }
                    >
                      Inspect actual route
                    </button>
                  ) : null}
                  <button
                    type="button"
                    className="secondary"
                    onClick={() => setSelectedBlockId(null)}
                  >
                    Clear selection
                  </button>
                </div>
              </>
            ) : (
              <p className="hint">
                Select a Block. Double-click an openable Block to descend one
                hierarchy level.
              </p>
            )}
          </div>

          <SourcePanel
            bundle={demoBundle}
            graph={currentGraph}
            selectedBlock={selectedBlock}
            onSelectBlock={setSelectedBlockId}
          />
        </section>
      </main>
    </div>
  );
}
