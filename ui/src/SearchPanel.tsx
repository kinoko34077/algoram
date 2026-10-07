import { useMemo, useState } from "react";
import { jaJP } from "./locales/ja-JP";
import {
  listSourceLanguages,
  searchNavigation,
  type NavigationIndex,
  type NavigationRecord,
} from "./navigation";

interface SearchPanelProps {
  index: NavigationIndex;
  status?: string | null;
  onSelectResult: (record: NavigationRecord) => void;
}

const RESULT_LIMIT = 12;

export function SearchPanel({
  index,
  status,
  onSelectResult,
}: SearchPanelProps) {
  const [query, setQuery] = useState("");
  const [language, setLanguage] = useState("");

  const languages = useMemo(() => listSourceLanguages(index), [index]);
  const active = query.trim().length > 0 || language.length > 0;
  const results = useMemo(
    () =>
      active
        ? searchNavigation(index, query, {
            ...(language ? { language } : {}),
          })
        : [],
    [active, index, language, query],
  );
  const visibleResults = results.slice(0, RESULT_LIMIT);

  function clear() {
    setQuery("");
    setLanguage("");
  }

  return (
    <section className="search-panel" aria-label={jaJP.authoring.search.graphSearch}>
      <div className="search-controls">
        <label>
          <span>{jaJP.authoring.search.findBlock}</span>
          <input
            type="search"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Escape" && active) {
                event.preventDefault();
                clear();
              }
            }}
            placeholder={jaJP.authoring.search.placeholder}
          />
        </label>

        <label>
          <span>{jaJP.authoring.search.language}</span>
          <select
            value={language}
            onChange={(event) => setLanguage(event.target.value)}
          >
            <option value="">{jaJP.authoring.search.all}</option>
            {languages.map((item) => (
              <option key={item} value={item}>
                {item}
              </option>
            ))}
          </select>
        </label>

        <button
          type="button"
          className="tertiary-action"
          onClick={clear}
          disabled={!active}
        >
          {jaJP.authoring.search.clear}
        </button>
      </div>

      {status ? (
        <p className="search-status" role="status" aria-live="polite">
          {status}
        </p>
      ) : null}

      {active ? (
        <div className="search-results">
          <div className="search-summary">
            <strong>{results.length}</strong>
            <span>{jaJP.authoring.search.matches}</span>
            {results.length > RESULT_LIMIT ? (
              <small>{jaJP.authoring.search.firstResults.replace("{limit}", String(RESULT_LIMIT))}</small>
            ) : null}
          </div>

          {visibleResults.length > 0 ? (
            <ul className="search-result-list">
              {visibleResults.map((record) => (
                <li key={`${record.graphId}:${record.blockId}`}>
                  <button
                    type="button"
                    className="search-result"
                    onClick={() => onSelectResult(record)}
                  >
                    <strong>{record.label}</strong>
                    <span>{record.graphLabel ?? record.graphId}</span>
                    <small>
                      {record.sourceOrigin ?? record.blockId}
                      {record.sourceLanguage
                        ? ` · ${record.sourceLanguage}`
                        : ""}
                    </small>
                  </button>
                </li>
              ))}
            </ul>
          ) : (
            <p className="search-empty" role="status">
              {jaJP.authoring.search.noMatchingBlocks}
            </p>
          )}
        </div>
      ) : null}
    </section>
  );
}
