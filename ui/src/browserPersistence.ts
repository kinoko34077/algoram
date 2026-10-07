import type { AlgoramGraph } from "./algoram";
import type { PersistedEditorSession } from "./documentPersistence";

const DATABASE_NAME = "algoram-editor";
const DATABASE_VERSION = 1;
const GRAPH_STORE = "graphs";
const SESSION_STORE = "sessions";
const META_STORE = "meta";
const LAST_DOCUMENT_KEY = "last-document";

interface LastDocumentRecord {
  key: typeof LAST_DOCUMENT_KEY;
  graphId: string;
}

export interface LocalDocument {
  graph: AlgoramGraph;
  session: PersistedEditorSession | null;
}

function requestResult<T>(request: IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    request.addEventListener("success", () => resolve(request.result), {
      once: true,
    });
    request.addEventListener(
      "error",
      () => reject(request.error ?? new Error("IndexedDB request failed.")),
      { once: true },
    );
  });
}

function transactionComplete(transaction: IDBTransaction): Promise<void> {
  return new Promise((resolve, reject) => {
    transaction.addEventListener("complete", () => resolve(), { once: true });
    transaction.addEventListener(
      "abort",
      () => reject(transaction.error ?? new Error("IndexedDB transaction aborted.")),
      { once: true },
    );
    transaction.addEventListener(
      "error",
      () => reject(transaction.error ?? new Error("IndexedDB transaction failed.")),
      { once: true },
    );
  });
}

function openDatabase(): Promise<IDBDatabase> {
  if (!("indexedDB" in globalThis)) {
    return Promise.reject(new Error("This browser does not provide IndexedDB."));
  }

  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DATABASE_NAME, DATABASE_VERSION);

    request.addEventListener("upgradeneeded", () => {
      const database = request.result;
      if (!database.objectStoreNames.contains(GRAPH_STORE)) {
        database.createObjectStore(GRAPH_STORE);
      }
      if (!database.objectStoreNames.contains(SESSION_STORE)) {
        database.createObjectStore(SESSION_STORE);
      }
      if (!database.objectStoreNames.contains(META_STORE)) {
        database.createObjectStore(META_STORE, { keyPath: "key" });
      }
    });

    request.addEventListener("success", () => resolve(request.result), {
      once: true,
    });
    request.addEventListener(
      "error",
      () => reject(request.error ?? new Error("Could not open editor storage.")),
      { once: true },
    );
  });
}

export async function saveLocalDocument(
  graph: AlgoramGraph,
  session: PersistedEditorSession,
): Promise<void> {
  const database = await openDatabase();
  try {
    const transaction = database.transaction(
      [GRAPH_STORE, SESSION_STORE, META_STORE],
      "readwrite",
    );

    transaction.objectStore(GRAPH_STORE).put(structuredClone(graph), graph.id);
    transaction
      .objectStore(SESSION_STORE)
      .put(structuredClone(session), graph.id);
    transaction.objectStore(META_STORE).put({
      key: LAST_DOCUMENT_KEY,
      graphId: graph.id,
    } satisfies LastDocumentRecord);

    await transactionComplete(transaction);
  } finally {
    database.close();
  }
}

export async function loadLastLocalDocument(): Promise<LocalDocument | null> {
  const database = await openDatabase();
  try {
    const metaTransaction = database.transaction(META_STORE, "readonly");
    const meta = (await requestResult(
      metaTransaction.objectStore(META_STORE).get(LAST_DOCUMENT_KEY),
    )) as LastDocumentRecord | undefined;

    if (!meta?.graphId) {
      return null;
    }

    const documentTransaction = database.transaction(
      [GRAPH_STORE, SESSION_STORE],
      "readonly",
    );
    const graphRequest = documentTransaction
      .objectStore(GRAPH_STORE)
      .get(meta.graphId);
    const sessionRequest = documentTransaction
      .objectStore(SESSION_STORE)
      .get(meta.graphId);

    const [graph, session] = await Promise.all([
      requestResult(graphRequest) as Promise<AlgoramGraph | undefined>,
      requestResult(sessionRequest) as Promise<
        PersistedEditorSession | undefined
      >,
    ]);

    if (!graph) {
      return null;
    }

    return {
      graph: structuredClone(graph),
      session: session ? structuredClone(session) : null,
    };
  } finally {
    database.close();
  }
}
