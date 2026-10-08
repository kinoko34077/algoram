import { demoJa } from "./demo.ts";

/** UI-only shorthand; keys are exact localized full labels, not native IDs. */
export const shortBlockNamesJa: Readonly<Record<string, string>> = {
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
