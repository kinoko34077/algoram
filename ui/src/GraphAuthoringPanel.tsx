import { useEffect, useRef, useState } from "react";
import type { AlgoramGraph } from "./algoram";
import type { GraphValidationIssue } from "./graphValidation";

interface GraphAuthoringPanelProps {
  graph: AlgoramGraph;
  editable: boolean;
  dirty: boolean;
  validationIssues: GraphValidationIssue[];
  onCommitLabel: (label: string) => GraphValidationIssue[];
}

export function GraphAuthoringPanel({
  graph,
  editable,
  dirty,
  validationIssues,
  onCommitLabel,
}: GraphAuthoringPanelProps) {
  const [draftLabel, setDraftLabel] = useState(graph.label ?? "");
  const [commitIssues, setCommitIssues] = useState<GraphValidationIssue[]>([]);
  const skipBlurCommit = useRef(false);

  useEffect(() => {
    skipBlurCommit.current = false;
    setDraftLabel(graph.label ?? "");
    setCommitIssues([]);
  }, [graph.id, graph.label]);

  function commit() {
    setCommitIssues(onCommitLabel(draftLabel));
  }

  const displayedIssues =
    commitIssues.length > 0 ? commitIssues : validationIssues;

  return (
    <section className="graph-authoring-panel" aria-label="Graph properties">
      <div className="panel-heading-row">
        <div>
          <p className="eyebrow">Graph</p>
          <strong>{graph.id}</strong>
        </div>
        <div className="graph-state" aria-live="polite">
          <span className={editable ? "state-chip editable" : "state-chip"}>
            {editable ? "Editable" : "Read only"}
          </span>
          {editable ? (
            <span className={dirty ? "state-chip modified" : "state-chip"}>
              {dirty ? "Modified" : "Clean"}
            </span>
          ) : null}
          <span
            className={
              displayedIssues.length === 0
                ? "state-chip valid"
                : "state-chip invalid"
            }
          >
            {displayedIssues.length === 0
              ? "Valid"
              : `${displayedIssues.length} issue${displayedIssues.length === 1 ? "" : "s"}`}
          </span>
        </div>
      </div>

      {editable ? (
        <form
          className="graph-property-form"
          onSubmit={(event) => {
            event.preventDefault();
            commit();
          }}
        >
          <label>
            Label
            <input
              value={draftLabel}
              onChange={(event) => {
                setDraftLabel(event.target.value);
                setCommitIssues([]);
              }}
              onBlur={() => {
                if (skipBlurCommit.current) {
                  skipBlurCommit.current = false;
                  return;
                }
                commit();
              }}
              onKeyDown={(event) => {
                if (event.key === "Escape") {
                  event.preventDefault();
                  skipBlurCommit.current = true;
                  setDraftLabel(graph.label ?? "");
                  setCommitIssues([]);
                  event.currentTarget.blur();
                }
              }}
              aria-invalid={displayedIssues.length > 0}
              aria-describedby={
                displayedIssues.length > 0
                  ? "graph-label-help graph-validation"
                  : "graph-label-help"
              }
            />
          </label>
          <p id="graph-label-help" className="compact-hint">
            Enter or leave the field to commit. Esc cancels the current text
            edit. Canonical changes support Undo/Redo.
          </p>
        </form>
      ) : (
        <p className="compact-hint">
          This Graph is a derived/source-backed view. Canonical editing is
          disabled; presentation state remains local to the editor.
        </p>
      )}

      {displayedIssues.length > 0 ? (
        <div id="graph-validation" className="graph-validation" role="alert">
          <strong>Graph validation</strong>
          <ul>
            {displayedIssues.slice(0, 4).map((issue, index) => (
              <li
                key={`${issue.code}:${issue.connectionId ?? issue.blockId ?? index}`}
              >
                {issue.message}
              </li>
            ))}
          </ul>
          {displayedIssues.length > 4 ? (
            <p>{displayedIssues.length - 4} more issues.</p>
          ) : null}
        </div>
      ) : null}
    </section>
  );
}
