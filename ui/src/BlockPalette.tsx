import { useEffect, useMemo, useRef, useState } from "react";
import { jaJP } from "./locales/ja-JP";
import type { AlgoramPort } from "./algoram";
import type { ReusableBlockTemplate } from "./blockAuthoring";
import { projectBlockDisplay } from "./blockDisplay";
import { discoverConnectionStatus } from "./connectionDiscovery";

export interface CapabilityAddSource {
  blockId: string;
  blockLabel: string;
  outputs: AlgoramPort[];
  sourcePortId: string | null;
}

interface BlockPaletteProps {
  templates: ReusableBlockTemplate[];
  context?: CapabilityAddSource | null;
  onSourcePortChange?: (portId: string) => void;
  onAdd: (template: ReusableBlockTemplate) => void;
  onClose?: () => void;
}

/**
 * One searchable discovery surface for global + and selected-output +.
 * Connection statuses are explicitly exploratory and never route proof.
 */
export function BlockPalette({
  templates,
  context = null,
  onSourcePortChange,
  onAdd,
  onClose,
}: BlockPaletteProps) {
  const [query, setQuery] = useState("");
  const searchRef = useRef<HTMLInputElement>(null);
  const sourcePort = context?.outputs.find((port) => port.id === context.sourcePortId);
  const visibleTemplates = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return templates;
    return templates.filter((template) => {
      const display = projectBlockDisplay(template.block);
      return [display.shortLabel, display.fullLabel, template.definitionRef]
        .join(" ")
        .toLowerCase()
        .includes(needle);
    });
  }, [query, templates]);

  useEffect(() => {
    searchRef.current?.focus();
  }, [context?.blockId]);

  return (
    <aside
      className="block-palette"
      aria-label={jaJP.authoring.blockPalette.accessibilityLabel}
      onKeyDown={(event) => {
        if (event.key !== "Escape") return;
        event.stopPropagation();
        if (query) setQuery("");
        else onClose?.();
      }}
    >
      <div className="palette-heading">
        <div>
          <strong>
            {context
              ? jaJP.authoring.blockPalette.contextualAdd
              : jaJP.authoring.blockPalette.globalAdd}
          </strong>
          {context ? (
            <p className="compact-hint">
              {jaJP.authoring.blockPalette.sourceContext.replace("{name}", context.blockLabel)}
            </p>
          ) : null}
        </div>
        <button
          type="button"
          className="tertiary-action palette-close"
          aria-label={jaJP.authoring.blockPalette.searchClose}
          onClick={onClose}
        >
          ×
        </button>
      </div>

      {context && context.outputs.length > 1 ? (
        <label className="palette-source-port">
          <span>{jaJP.authoring.blockPalette.outputChoice}</span>
          <select
            value={context.sourcePortId ?? ""}
            onChange={(event) => onSourcePortChange?.(event.target.value)}
          >
            <option value="">{jaJP.authoring.blockPalette.outputChoice}</option>
            {context.outputs.map((port) => (
              <option key={port.id} value={port.id}>
                {port.id} · {port.channel}
              </option>
            ))}
          </select>
        </label>
      ) : null}

      <label className="palette-search">
        {jaJP.authoring.blockPalette.findBlock}
        <input
          ref={searchRef}
          type="search"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder={jaJP.authoring.blockPalette.searchDefinitions}
        />
      </label>

      <ul className="palette-list" aria-label={jaJP.authoring.blockPalette.reusableBlockDefinitions}>
        {visibleTemplates.map((template) => {
          const display = projectBlockDisplay(template.block);
          const status = context ? discoverConnectionStatus(sourcePort, template.block) : null;

          return (
            <li key={template.definitionRef}>
              <button
                type="button"
                className="palette-item"
                data-connection-discovery={status ?? "global"}
                disabled={Boolean(context && !sourcePort)}
                draggable={!context || Boolean(sourcePort)}
                onDragStart={(event) => {
                  event.dataTransfer.effectAllowed = "copy";
                  event.dataTransfer.setData(
                    "application/x-algoram-block-template",
                    template.definitionRef,
                  );
                }}
                onClick={() => onAdd(template)}
              >
                <strong>{display.shortLabel}</strong>
                {display.shortLabel !== display.fullLabel ? (
                  <span>{display.fullLabel}</span>
                ) : null}
                {status ? (
                  <small>{jaJP.authoring.blockPalette.routeStatus[status]}</small>
                ) : (
                  <small>
                    {jaJP.authoring.blockPalette.ioSummary
                      .replace("{inputs}", String((template.block.ports ?? []).filter((p) => p.direction === "in").length))
                      .replace("{outputs}", String((template.block.ports ?? []).filter((p) => p.direction === "out").length))}
                  </small>
                )}
              </button>
            </li>
          );
        })}
      </ul>
      {visibleTemplates.length === 0 ? (
        <p className="palette-empty">{jaJP.authoring.blockPalette.noMatches}</p>
      ) : null}
    </aside>
  );
}
