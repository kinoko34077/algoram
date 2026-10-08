import { useEffect, useMemo, useRef, useState } from "react";
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
  open: boolean;
  onClose: () => void;
  onSelectResult: (record: NavigationRecord) => void;
}

const RESULT_LIMIT = 12;

export function SearchPanel({
  index,
  status,
  open,
  onClose,
  onSelectResult,
}: SearchPanelProps) {
  const [query, setQuery] = useState("");
  const [language, setLanguage] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);
  const resultButtonsRef = useRef<Array<HTMLButtonElement | null>>([]);
  useEffect(() => {
    if (open) inputRef.current?.focus();
  }, [open]);

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
    <section className="search-panel" aria-label={jaJP.authoring.search.graphSearch} onKeyDown={(event) => {
      if (event.key === "Escape" && !event.nativeEvent.isComposing) {
        event.preventDefault();
        event.stopPropagation();
        onClose();
      }
    }}>
      <div className="search-panel-heading"><strong>{jaJP.authoring.search.graphSearch}</strong><button type="button" className="tertiary-action" onClick={onClose}>{jaJP.common.actions.close}</button></div>
      <div className="search-controls">
        <label>
          <span>{jaJP.authoring.search.findBlock}</span>
          <input
            ref={inputRef}
            onKeyDown={(event) => {
              if (event.nativeEvent.isComposing || visibleResults.length === 0) return;
              if (event.key === "ArrowDown" || event.key === "ArrowUp") {
                event.preventDefault();
                resultButtonsRef.current[event.key === "ArrowDown" ? 0 : visibleResults.length - 1]?.focus();
              }
            }}
            type="search"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
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
              {visibleResults.map((record, index) => (
                <li key={`${record.graphId}:${record.blockId}`}>
                  <button
                    type="button"
                    className="search-result"
                    ref={(element) => { resultButtonsRef.current[index] = element; }}
                    onKeyDown={(event) => {
                      if (event.nativeEvent.isComposing) return;
                      if (event.key === "ArrowDown") {
                        event.preventDefault();
                        resultButtonsRef.current[Math.min(index + 1, visibleResults.length - 1)]?.focus();
                      } else if (event.key === "ArrowUp") {
                        event.preventDefault();
                        if (index === 0) inputRef.current?.focus();
                        else resultButtonsRef.current[index - 1]?.focus();
                      }
                    }}
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
