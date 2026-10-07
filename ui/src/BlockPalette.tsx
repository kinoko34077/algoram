import { useMemo, useState } from "react";
import { jaJP } from "./locales/ja-JP";
import type { ReusableBlockTemplate } from "./blockAuthoring";

interface BlockPaletteProps {
  templates: ReusableBlockTemplate[];
  onAdd: (template: ReusableBlockTemplate) => void;
}

export function BlockPalette({ templates, onAdd }: BlockPaletteProps) {
  const [query, setQuery] = useState("");
  const [selectedDefinitionRef, setSelectedDefinitionRef] = useState<
    string | null
  >(templates[0]?.definitionRef ?? null);

  const visibleTemplates = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) {
      return templates;
    }

    return templates.filter((template) =>
      [template.label, template.definitionRef]
        .join(" ")
        .toLowerCase()
        .includes(needle),
    );
  }, [query, templates]);

  const selected =
    visibleTemplates.find(
      (template) => template.definitionRef === selectedDefinitionRef,
    ) ??
    visibleTemplates[0] ??
    null;

  return (
    <aside
      className="block-palette"
      aria-label={jaJP.authoring.blockPalette.accessibilityLabel}
      onKeyDown={(event) => {
        if (event.key === "Escape") {
          setQuery("");
          setSelectedDefinitionRef(null);
        }
      }}
    >
      <div className="palette-heading">
        <div>
          <p className="eyebrow">{jaJP.authoring.blockPalette.library}</p>
          <strong>{jaJP.authoring.blockPalette.reusableBlocks}</strong>
        </div>
        <span>{templates.length}</span>
      </div>

      <label className="palette-search">
        {jaJP.authoring.blockPalette.findBlock}
        <input
          type="search"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder={jaJP.authoring.blockPalette.searchDefinitions}
        />
      </label>

      <ul className="palette-list" aria-label={jaJP.authoring.blockPalette.reusableBlockDefinitions}>
        {visibleTemplates.map((template) => {
          const selectedRow =
            selected?.definitionRef === template.definitionRef;
          const inputs = (template.block.ports ?? []).filter(
            (port) => port.direction === "in",
          ).length;
          const outputs = (template.block.ports ?? []).filter(
            (port) => port.direction === "out",
          ).length;

          return (
            <li key={template.definitionRef}>
              <button
                type="button"
                className={
                  selectedRow ? "palette-item selected" : "palette-item"
                }
                aria-pressed={selectedRow}
                onClick={() =>
                  setSelectedDefinitionRef(template.definitionRef)
                }
              >
                <strong>{template.label}</strong>
                <span>{template.definitionRef}</span>
                <small>
                  {jaJP.authoring.blockPalette.ioSummary
                    .replace("{inputs}", String(inputs))
                    .replace("{outputs}", String(outputs))}
                </small>
              </button>
            </li>
          );
        })}
      </ul>

      {visibleTemplates.length === 0 ? (
        <p className="palette-empty">{jaJP.authoring.blockPalette.noMatches}</p>
      ) : null}

      <div className="palette-actions">
        <button
          type="button"
          className="secondary-action"
          disabled={!selected}
          onClick={() => {
            if (selected) {
              onAdd(selected);
            }
          }}
        >
          {jaJP.authoring.blockPalette.addBlock}
        </button>
      </div>
    </aside>
  );
}
