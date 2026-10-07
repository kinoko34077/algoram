import { useEffect, useRef } from "react";
import { jaJP } from "./locales/ja-JP";

interface HeaderOverflowMenuProps {
  historyAvailable: boolean;
  canUndo: boolean;
  canRedo: boolean;
  inspectorOpen: boolean;
  onUndo: () => void;
  onRedo: () => void;
  onToggleInspector: () => void;
}

export function HeaderOverflowMenu({
  historyAvailable,
  canUndo,
  canRedo,
  inspectorOpen,
  onUndo,
  onRedo,
  onToggleInspector,
}: HeaderOverflowMenuProps) {
  const detailsRef = useRef<HTMLDetailsElement>(null);

  function closeMenu() {
    if (detailsRef.current) {
      detailsRef.current.open = false;
    }
  }

  useEffect(() => {
    function handleEscape(event: KeyboardEvent) {
      if (event.key === "Escape" && detailsRef.current?.open) {
        detailsRef.current.open = false;
      }
    }

    window.addEventListener("keydown", handleEscape);
    return () => window.removeEventListener("keydown", handleEscape);
  }, []);

  return (
    <details className="header-overflow-menu" ref={detailsRef}>
      <summary
        className="tertiary-action"
        aria-label={jaJP.headerOverflow.moreEditorActions}
      >
        {jaJP.headerOverflow.more}
      </summary>
      <div
        className="header-overflow-popover"
        role="group"
        aria-label={jaJP.headerOverflow.moreEditorActions}
      >
        {historyAvailable ? (
          <>
            <button
              type="button"
              className="tertiary-action"
              disabled={!canUndo}
              onClick={() => {
                closeMenu();
                onUndo();
              }}
            >
              {jaJP.common.actions.undo}
            </button>
            <button
              type="button"
              className="tertiary-action"
              disabled={!canRedo}
              onClick={() => {
                closeMenu();
                onRedo();
              }}
            >
              {jaJP.common.actions.redo}
            </button>
          </>
        ) : null}
        <button
          type="button"
          className="tertiary-action"
          aria-pressed={inspectorOpen}
          onClick={() => {
            closeMenu();
            onToggleInspector();
          }}
        >
          {inspectorOpen ? jaJP.headerOverflow.hideInspector : jaJP.headerOverflow.showInspector}
        </button>
      </div>
    </details>
  );
}
