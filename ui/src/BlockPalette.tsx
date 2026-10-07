import { useMemo, useState } from "react";
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
      aria-label="Reusable Block palette"
      onKeyDown={(event) => {
        if (event.key === "Escape") {
          setQuery("");
          setSelectedDefinitionRef(null);
        }
      }}
    >
      <div className="palette-heading">
        <div>
          <p className="eyebrow">Library</p>
          <strong>Reusable Blocks</strong>
        </div>
        <span>{templates.length}</span>
      </div>

      <label className="palette-search">
        Find Block
        <input
          type="search"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder="Search definitions"
        />
      </label>

      <ul className="palette-list" aria-label="Reusable Block definitions">
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
                  {inputs} in · {outputs} out
                </small>
              </button>
            </li>
          );
        })}
      </ul>

      {visibleTemplates.length === 0 ? (
        <p className="palette-empty">No reusable Blocks match.</p>
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
          Add Block
        </button>
      </div>
    </aside>
  );
}
