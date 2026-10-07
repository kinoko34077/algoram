export type EditorPerformanceMark =
  | "presentation-commit"
  | "node-drag-start"
  | "node-drag-stop";

export function markEditorPerformance(name: EditorPerformanceMark): void {
  if (!import.meta.env.DEV || typeof performance === "undefined") {
    return;
  }

  performance.mark(`algoram:${name}`);
}
