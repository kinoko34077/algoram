import { useEffect, useRef, useState } from "react";
import { jaJP } from "./locales/ja-JP";
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
    <section className="graph-authoring-panel" aria-label={jaJP.authoring.graphProperties.properties}>
      <div className="panel-heading-row">
        <div>
          <p className="eyebrow">{jaJP.authoring.graphProperties.graph}</p>
          <strong>{graph.id}</strong>
        </div>
        <div className="graph-state" aria-live="polite">
          <span className={editable ? "state-chip editable" : "state-chip"}>
            {editable ? jaJP.authoring.graphProperties.editable : jaJP.authoring.graphProperties.readOnly}
          </span>
          {editable ? (
            <span className={dirty ? "state-chip modified" : "state-chip"}>
              {dirty ? jaJP.authoring.graphProperties.modified : jaJP.authoring.graphProperties.clean}
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
              ? jaJP.authoring.graphProperties.valid
              : jaJP.authoring.graphProperties.issueCount.replace("{count}", String(displayedIssues.length))}
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
            {jaJP.authoring.graphProperties.label}
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
            {jaJP.authoring.graphProperties.editHint}
          </p>
        </form>
      ) : (
        <p className="compact-hint">
          {jaJP.authoring.graphProperties.derivedReadOnlyHint}
        </p>
      )}

      {displayedIssues.length > 0 ? (
        <div id="graph-validation" className="graph-validation" role="alert">
          <strong>{jaJP.authoring.graphProperties.validation}</strong>
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
            <p>{jaJP.authoring.graphProperties.moreIssues.replace("{count}", String(displayedIssues.length - 4))}</p>
          ) : null}
        </div>
      ) : null}
    </section>
  );
}
