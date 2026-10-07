import { useEffect, useRef } from "react";

export interface FileOperationStatus {
  kind: "success" | "info" | "error";
  message: string;
}

interface FileMenuProps {
  canOpen: boolean;
  canSaveLocal: boolean;
  status: FileOperationStatus | null;
  onOpenFile: (file: File) => void | Promise<void>;
  onSaveLocal: () => void | Promise<void>;
  onExport: () => void;
}

export function FileMenu({
  canOpen,
  canSaveLocal,
  status,
  onOpenFile,
  onSaveLocal,
  onExport,
}: FileMenuProps) {
  const detailsRef = useRef<HTMLDetailsElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  function closeMenu() {
    if (detailsRef.current) {
      detailsRef.current.open = false;
    }
  }

  function chooseFile() {
    closeMenu();
    inputRef.current?.click();
  }

  useEffect(() => {
    function handleShortcut(event: KeyboardEvent) {
      if (!(event.metaKey || event.ctrlKey) || event.altKey || event.isComposing) {
        return;
      }

      const key = event.key.toLowerCase();
      if (key === "o" && canOpen) {
        event.preventDefault();
        chooseFile();
      } else if (key === "s" && canSaveLocal) {
        event.preventDefault();
        void onSaveLocal();
      }
    }

    function handleEscape(event: KeyboardEvent) {
      if (event.key === "Escape" && detailsRef.current?.open) {
        detailsRef.current.open = false;
      }
    }

    window.addEventListener("keydown", handleShortcut);
    window.addEventListener("keydown", handleEscape);
    return () => {
      window.removeEventListener("keydown", handleShortcut);
      window.removeEventListener("keydown", handleEscape);
    };
  }, [canOpen, canSaveLocal, onSaveLocal]);

  return (
    <div className="file-menu-shell">
      <details className="file-menu" ref={detailsRef}>
        <summary className="tertiary-action">File</summary>
        <div className="file-menu-popover" role="group" aria-label="File operations">
          <button
            type="button"
            className="tertiary-action"
            disabled={!canOpen}
            onClick={chooseFile}
          >
            Open
          </button>
          <button
            type="button"
            className="tertiary-action"
            disabled={!canSaveLocal}
            onClick={() => {
              closeMenu();
              void onSaveLocal();
            }}
          >
            Save local
          </button>
          <button
            type="button"
            className="tertiary-action"
            onClick={() => {
              closeMenu();
              onExport();
            }}
          >
            Export
          </button>
          <small>Ctrl/Cmd+O · Ctrl/Cmd+S</small>
        </div>
      </details>

      <input
        ref={inputRef}
        className="visually-hidden-file-input"
        type="file"
        accept=".algoram.json,application/json"
        tabIndex={-1}
        aria-hidden="true"
        onChange={(event) => {
          const file = event.currentTarget.files?.[0];
          event.currentTarget.value = "";
          if (file) {
            void onOpenFile(file);
          }
        }}
      />

      {status ? (
        <span
          className={
            status.kind === "error"
              ? "file-operation-status error-text"
              : "file-operation-status"
          }
          role={status.kind === "error" ? "alert" : "status"}
          title={status.message}
        >
          {status.message}
        </span>
      ) : null}
    </div>
  );
}
