import { useEffect, useRef, type Dispatch, type SetStateAction } from "react";

export interface BrowserGraphPathEntry {
  graphId: string;
  label: string;
}

interface StoredGraphNavigation {
  rootGraphId: string;
  path: BrowserGraphPathEntry[];
}

const HISTORY_KEY = "__algoramGraphNavigationV1";
const MAX_PATH_LENGTH = 256;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function readGraphHistoryState(
  state: unknown,
  rootGraphId: string,
  availableGraphs: Record<string, unknown>,
): BrowserGraphPathEntry[] | null {
  if (!isRecord(state) || !isRecord(state[HISTORY_KEY])) return null;
  const stored = state[HISTORY_KEY] as Record<string, unknown>;
  if (stored.rootGraphId !== rootGraphId || !Array.isArray(stored.path)) return null;
  if (stored.path.length === 0 || stored.path.length > MAX_PATH_LENGTH) return null;

  const path: BrowserGraphPathEntry[] = [];
  for (const entry of stored.path) {
    if (!isRecord(entry) || typeof entry.graphId !== "string" ||
        typeof entry.label !== "string" || entry.label.length > 1024 ||
        !Object.hasOwn(availableGraphs, entry.graphId)) {
      return null;
    }
    path.push({ graphId: entry.graphId, label: entry.label });
  }
  return path[0]?.graphId === rootGraphId ? path : null;
}

export function makeGraphHistoryState(
  previous: unknown,
  rootGraphId: string,
  path: readonly BrowserGraphPathEntry[],
): Record<string, unknown> {
  const inherited = isRecord(previous) ? previous : {};
  return {
    ...inherited,
    [HISTORY_KEY]: {
      rootGraphId,
      path: path.map(({ graphId, label }) => ({ graphId, label })),
    } satisfies StoredGraphNavigation,
  };
}

function sameGraphPath(
  left: readonly BrowserGraphPathEntry[],
  right: readonly BrowserGraphPathEntry[],
): boolean {
  return left.length === right.length &&
    left.every((entry, index) => entry.graphId === right[index]?.graphId);
}

interface GraphHistoryOptions {
  rootGraphId: string;
  path: BrowserGraphPathEntry[];
  setPath: Dispatch<SetStateAction<BrowserGraphPathEntry[]>>;
  availableGraphs: Record<string, unknown>;
  ready: boolean;
}

/** History contains only navigation labels/IDs, never canonical Graph state or host grants. */
export function useBrowserGraphHistory({
  rootGraphId,
  path,
  setPath,
  availableGraphs,
  ready,
}: GraphHistoryOptions): void {
  const initializedRoot = useRef<string | null>(null);

  useEffect(() => {
    if (!ready) return;

    const active = readGraphHistoryState(
      window.history.state,
      rootGraphId,
      availableGraphs,
    );
    const firstForRoot = initializedRoot.current !== rootGraphId;
    initializedRoot.current = rootGraphId;
    const nextState = makeGraphHistoryState(window.history.state, rootGraphId, path);

    if (firstForRoot || !active || sameGraphPath(active, path)) {
      if (!active || !sameGraphPath(active, path) ||
          active.some((item, index) => item.label !== path[index]?.label)) {
        window.history.replaceState(nextState, "", window.location.href);
      }
      return;
    }

    // Only a user-visible Graph path transition receives a new history entry.
    window.history.pushState(nextState, "", window.location.href);
  }, [ready, rootGraphId, path, availableGraphs]);

  useEffect(() => {
    if (!ready) return;
    function handlePopstate(event: PopStateEvent) {
      const restored = readGraphHistoryState(
        event.state,
        rootGraphId,
        availableGraphs,
      );
      const root = availableGraphs[rootGraphId];
      const fallbackLabel =
        isRecord(root) && typeof root.label === "string" && root.label.trim()
          ? root.label
          : rootGraphId;
      setPath(restored ?? [{ graphId: rootGraphId, label: fallbackLabel }]);
    }
    window.addEventListener("popstate", handlePopstate);
    return () => window.removeEventListener("popstate", handlePopstate);
  }, [ready, rootGraphId, availableGraphs, setPath]);
}
