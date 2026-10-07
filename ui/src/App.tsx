import { useCallback, useMemo, useState } from "react";
import type { AlgoramBlock } from "./algoram";
import {
  getBlockAnnotation,
  removeBlockAnnotation,
  setBlockAnnotation,
  type BlockAnnotations,
} from "./annotations";
import { AnnotationPanel } from "./AnnotationPanel";
import { demoBundle } from "./fixture";
import { DraftLinkPanel } from "./DraftLinkPanel";
import { GraphCanvas } from "./GraphCanvas";
import {
  buildNavigationIndex,
  resolveBlockPath,
  type NavigationRecord,
} from "./navigation";
import {
  addDraftLink,
  EMPTY_GRAPH_PRESENTATION,
  validateDraftLink,
  type DraftLink,
  type GraphPresentationState,
} from "./presentation";
import { markEditorPerformance } from "./perfMarks";
import { SearchPanel } from "./SearchPanel";
import { SourcePanel } from "./SourcePanel";

interface Breadcrumb {
  graphId: string;
  label: string;
}

type PresentationByGraph = Record<string, GraphPresentationState>;
type SelectionByGraph = Record<string, string | null>;

interface GraphFocusRequest {
  graphId: string;
  blockId: string;
  revision: number;
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
  const [selectionByGraph, setSelectionByGraph] = useState<SelectionByGraph>(
    {},
  );
  const [presentationByGraph, setPresentationByGraph] =
    useState<PresentationByGraph>({});
  const [navigationStatus, setNavigationStatus] = useState<string | null>(null);
  const [annotations, setAnnotations] = useState<BlockAnnotations>({});
  const [inspectorOpen, setInspectorOpen] = useState(true);
  const [focusRequest, setFocusRequest] = useState<GraphFocusRequest | null>(
    null,
  );

  const navigationIndex = useMemo(
    () => buildNavigationIndex(demoBundle),
    [],
  );

  const currentGraphId = path.at(-1)?.graphId ?? demoBundle.rootGraphId;
  const currentGraph = demoBundle.graphs[currentGraphId];

  if (!currentGraph) {
    throw new Error(`Missing graph fixture: ${currentGraphId}`);
  }

  const selectedBlockId = selectionByGraph[currentGraphId] ?? null;
  const currentPresentation =
    presentationByGraph[currentGraphId] ?? EMPTY_GRAPH_PRESENTATION;

  const selectedBlock = useMemo(
    () =>
      currentGraph.blocks.find((block) => block.id === selectedBlockId) ?? null,
    [currentGraph, selectedBlockId],
  );

  const selectedRouteGraphId =
    selectedBlockId === null
      ? undefined
      : demoBundle.routeInspections?.[selectedBlockId];

  const selectedAnnotation =
    selectedBlock === null
      ? ""
      : getBlockAnnotation(annotations, selectedBlock.id);

  const updatePresentation = useCallback(
    (
      graphId: string,
      update: (current: GraphPresentationState) => GraphPresentationState,
    ) => {
      markEditorPerformance("presentation-commit");
      setPresentationByGraph((current) => {
        const previous = current[graphId] ?? EMPTY_GRAPH_PRESENTATION;
        const next = update(previous);
        if (next === previous) {
          return current;
        }

        return {
          ...current,
          [graphId]: next,
        };
      });
    },
    [],
  );

  const selectBlock = useCallback(
    (blockId: string | null) => {
      setSelectionByGraph((current) => {
        const previous = current[currentGraphId] ?? null;
        if (previous === blockId) {
          return current;
        }

        return {
          ...current,
          [currentGraphId]: blockId,
        };
      });
    },
    [currentGraphId],
  );

  const updateCurrentPresentation = useCallback(
    (update: (current: GraphPresentationState) => GraphPresentationState) =>
      updatePresentation(currentGraphId, update),
    [currentGraphId, updatePresentation],
  );

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
    setNavigationStatus(null);
  }

  function openGraph(graphId: string, viaBlock: AlgoramBlock) {
    openGraphWithLabel(graphId, viaBlock.label);
  }

  function jumpTo(index: number) {
    setPath((current) => current.slice(0, index + 1));
    setNavigationStatus(null);
  }

  function updateSelectedAnnotation(value: string) {
    if (!selectedBlock) {
      return;
    }

    setAnnotations((current) =>
      setBlockAnnotation(current, selectedBlock.id, value),
    );
  }

  function removeSelectedAnnotation() {
    if (!selectedBlock || selectedAnnotation.length === 0) {
      return;
    }

    if (!window.confirm("Clear this local note? This cannot be undone.")) {
      return;
    }

    setAnnotations((current) =>
      removeBlockAnnotation(current, selectedBlock.id),
    );
  }

  function jumpToSearchResult(record: NavigationRecord) {
    const resolvedPath = resolveBlockPath(navigationIndex, record);
    if (!resolvedPath) {
      setNavigationStatus(
        `Cannot reach ${record.label} from the loaded root hierarchy.`,
      );
      return;
    }

    setPath(
      resolvedPath.map((entry) => ({
        graphId: entry.graphId,
        label: entry.label,
      })),
    );
    setSelectionByGraph((current) => ({
      ...current,
      [record.graphId]: record.blockId,
    }));
    setFocusRequest((current) => ({
      graphId: record.graphId,
      blockId: record.blockId,
      revision: (current?.revision ?? 0) + 1,
    }));
    setNavigationStatus(null);
  }

  function addDraftFromInspector(link: DraftLink): string | null {
    const error = validateDraftLink(currentGraph, currentPresentation, link);
    if (error) {
      return error;
    }

    updatePresentation(currentGraphId, (current) =>
      addDraftLink(currentGraph, current, link),
    );
    return null;
  }

  return (
    <div className="app-shell">
      <header className="app-header">
        <div className="title-block">
          <p className="eyebrow">ALGOram</p>
          <h1>{currentGraph.label ?? currentGraph.id}</h1>
        </div>

        <div className="header-actions">
          <div className="header-stat" aria-label="Visible Blocks">
            <strong>{currentGraph.blocks.length}</strong>
            <span>Blocks</span>
          </div>
          <button
            type="button"
            className="tertiary-action"
            aria-controls="inspector-panel"
            aria-expanded={inspectorOpen}
            aria-pressed={inspectorOpen}
            onClick={() => setInspectorOpen((open) => !open)}
          >
            Inspector
          </button>
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

      <SearchPanel
        index={navigationIndex}
        status={navigationStatus}
        onSelectResult={jumpToSearchResult}
      />

      <main
        className={
          inspectorOpen ? "workspace inspector-open" : "workspace inspector-closed"
        }
      >
        <section className="graph-region" aria-label="Graph editor">
          <GraphCanvas
            graph={currentGraph}
            selectedBlockId={selectedBlockId}
            focusRequest={
              focusRequest?.graphId === currentGraphId
                ? {
                    blockId: focusRequest.blockId,
                    revision: focusRequest.revision,
                  }
                : null
            }
            presentation={currentPresentation}
            onPresentationChange={updateCurrentPresentation}
            onSelectBlock={selectBlock}
            onOpenGraph={openGraph}
          />
        </section>

        {inspectorOpen ? (
          <aside
            id="inspector-panel"
            className="inspector-region"
            aria-label="Block inspector"
          >
            <section className="block-inspector">
              <div className="panel-heading-row">
                <div>
                  <p className="eyebrow">Current level</p>
                  <strong>{currentGraph.id}</strong>
                </div>
                {selectedBlock ? (
                  <button
                    type="button"
                    className="tertiary-action"
                    onClick={() => selectBlock(null)}
                  >
                    Clear selection
                  </button>
                ) : null}
              </div>

              {selectedBlock ? (
                <>
                  <h2>{selectedBlock.label}</h2>
                  <p className="block-id">{selectedBlock.id}</p>

                  <div className="inspector-actions">
                    {selectedBlock.internal_graph_ref ? (
                      <button
                        type="button"
                        className="secondary-action"
                        onClick={() =>
                          openGraph(
                            selectedBlock.internal_graph_ref as string,
                            selectedBlock,
                          )
                        }
                      >
                        Open graph
                      </button>
                    ) : null}
                    {selectedRouteGraphId ? (
                      <button
                        type="button"
                        className="secondary-action"
                        onClick={() =>
                          openGraphWithLabel(
                            selectedRouteGraphId,
                            "Actual selected route",
                          )
                        }
                      >
                        Route
                      </button>
                    ) : null}
                  </div>

                  <DraftLinkPanel
                    graph={currentGraph}
                    selectedBlock={selectedBlock}
                    presentation={currentPresentation}
                    onAdd={addDraftFromInspector}
                  />

                  <AnnotationPanel
                    blockId={selectedBlock.id}
                    value={selectedAnnotation}
                    onChange={updateSelectedAnnotation}
                    onRemove={removeSelectedAnnotation}
                  />
                </>
              ) : (
                <p className="hint">
                  Select a Block to inspect properties, source, or create a
                  presentation-only draft link.
                </p>
              )}
            </section>

            <SourcePanel
              bundle={demoBundle}
              graph={currentGraph}
              selectedBlock={selectedBlock}
              onSelectBlock={selectBlock}
            />
          </aside>
        ) : null}
      </main>
    </div>
  );
}
