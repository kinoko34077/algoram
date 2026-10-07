import type { AlgoramGraph } from "./algoram";
import {
  prepareCanonicalGraphExport,
  type CanonicalGraphExport,
} from "./graphExport";
import { validateGraph } from "./graphValidation";

export function requestCanonicalGraphDownload(
  graph: AlgoramGraph,
): CanonicalGraphExport {
  const payload = prepareCanonicalGraphExport(graph, validateGraph);
  const blob = new Blob([payload.json], {
    type: "application/json;charset=utf-8",
  });
  const objectUrl = URL.createObjectURL(blob);
  const anchor = document.createElement("a");

  anchor.href = objectUrl;
  anchor.download = payload.filename;
  anchor.hidden = true;
  document.body.append(anchor);

  try {
    anchor.click();
  } finally {
    anchor.remove();
    window.setTimeout(() => URL.revokeObjectURL(objectUrl), 0);
  }

  return payload;
}
