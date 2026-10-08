import type { AlgoramBlock } from "./algoram.ts";
import { demoJa } from "./locales/ja-JP/demo.ts";

/**
 * UI-only vocabulary. These keys are the complete, known fixture labels;
 * no native ID, contract, source anchor, or Graph data is translated.
 * Unknown labels stay verbatim rather than guessing their meaning.
 */
const shortNames: Readonly<Record<string, string>> = {
  [demoJa.graphPythonC]: "Py→C ×2",
  [demoJa.graphPythonRust]: "Py→Rust ×3",
  [demoJa.pythonInputBoundary]: "入力",
  [demoJa.pythonCtypesInputBoundary]: "入力",
  [demoJa.pythonResultBoundary]: "結果",
  [demoJa.pythonCtypesResultBoundary]: "結果",
  [demoJa.buildCSharedLibrary]: "Cビルド",
  [demoJa.buildRustCdylib]: "Rustビルド",
  [demoJa.callCThroughCtypes]: "C関数呼出",
  [demoJa.invokeRustExternC]: "Rust関数呼出",
};

export interface BlockGeometry {
  width: number;
  height: number;
}

export interface BlockDisplayProjection {
  shortLabel: string;
  fullLabel: string;
  gloss: string;
  geometry: BlockGeometry;
}

/**
 * Deterministic in both the ELK worker request and React Flow. Width is
 * based on visible glyphs, not implementation IDs or the internal graph.
 * A conservative cap avoids enormous nodes for arbitrary source labels.
 */
function approximateLabelWidth(label: string): number {
  const glyphUnits = [...label].reduce(
    (sum, char) => sum + (char.codePointAt(0)! <= 0x7f ? 0.62 : 1),
    0,
  );
  return Math.ceil(glyphUnits * 11 + 30);
}

export function projectBlockDisplay(block: AlgoramBlock): BlockDisplayProjection {
  const fullLabel = block.label;
  const shortLabel = shortNames[fullLabel] ?? fullLabel;
  const ports = block.ports ?? [];
  const inputs = ports.filter((port) => port.direction === "in").length;
  const outputs = ports.length - inputs;
  const rows = Math.max(inputs, outputs, 1);

  return {
    shortLabel,
    fullLabel,
    gloss: shortLabel === fullLabel ? fullLabel : fullLabel,
    geometry: {
      width: Math.max(72, Math.min(300, approximateLabelWidth(shortLabel))),
      height: Math.max(56, 32 + rows * 30),
    },
  };
}

export function getBlockGeometry(block: AlgoramBlock): BlockGeometry {
  return projectBlockDisplay(block).geometry;
}

/** Handle centres follow the *same* actual model height as ELK/React Flow. */
export function portHandleTop(
  index: number,
  count: number,
  height: number,
): number {
  if (count <= 0 || index < 0 || index >= count) {
    throw new RangeError("Invalid visible port index");
  }
  return height / 2 + (index - (count - 1) / 2) * 28;
}
