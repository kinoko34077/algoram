import { useEffect, useRef } from "react";

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
        aria-label="More editor actions"
      >
        More
      </summary>
      <div
        className="header-overflow-popover"
        role="group"
        aria-label="More editor actions"
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
              Undo
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
              Redo
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
          {inspectorOpen ? "Hide inspector" : "Show inspector"}
        </button>
      </div>
    </details>
  );
}
