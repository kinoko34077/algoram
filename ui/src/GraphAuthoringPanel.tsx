import { useEffect, useState } from "react";
import type { AlgoramGraph } from "./algoram";
import type { GraphValidationIssue } from "./graphValidation";

interface GraphAuthoringPanelProps {
  graph: AlgoramGraph;
  editable: boolean;
  dirty: boolean;
  validationIssues: GraphValidationIssue[];
  onCommitLabel: (label: string) => void;
}

export function GraphAuthoringPanel({
  graph,
  editable,
  dirty,
  validationIssues,
  onCommitLabel,
}: GraphAuthoringPanelProps) {
  const [draftLabel, setDraftLabel] = useState(graph.label ?? "");

  useEffect(() => {
    setDraftLabel(graph.label ?? "");
  }, [graph.id, graph.label]);

  function commit() {
    onCommitLabel(draftLabel);
  }

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
              validationIssues.length === 0
                ? "state-chip valid"
                : "state-chip invalid"
            }
          >
            {validationIssues.length === 0
              ? "Valid"
              : \`\${validationIssues.length} issue\${validationIssues.length === 1 ? "" : "s"}\`}
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
              onChange={(event) => setDraftLabel(event.target.value)}
              onBlur={commit}
              onKeyDown={(event) => {
                if (event.key === "Escape") {
                  event.preventDefault();
                  setDraftLabel(graph.label ?? "");
                  event.currentTarget.blur();
                }
              }}
              aria-describedby="graph-label-help"
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

      {validationIssues.length > 0 ? (
        <div className="graph-validation" role="alert">
          <strong>Graph validation</strong>
          <ul>
            {validationIssues.slice(0, 4).map((issue, index) => (
              <li
                key={\`\${issue.code}:\${issue.connectionId ?? issue.blockId ?? index}\`}
              >
                {issue.message}
              </li>
            ))}
          </ul>
          {validationIssues.length > 4 ? (
            <p>{validationIssues.length - 4} more issues.</p>
          ) : null}
        </div>
      ) : null}
    </section>
  );
}
