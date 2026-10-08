import { useCallback, useState } from "react";
import { jaJP } from "./locales/ja-JP";

const PREFERENCE_KEY = "algoram.ui.first-use-guide-dismissed.v1";

function wasDismissed(): boolean {
  try {
    return window.localStorage.getItem(PREFERENCE_KEY) === "1";
  } catch {
    return false;
  }
}

/** This boolean preference never contains Graph data, runtime grants, or credentials. */
export function useFirstUseGuide() {
  const [visible, setVisible] = useState(() => !wasDismissed());

  const dismiss = useCallback(() => {
    setVisible(false);
    try {
      window.localStorage.setItem(PREFERENCE_KEY, "1");
    } catch {
      // Storage may be disabled; the current-session dismissal still works.
    }
  }, []);

  const reopen = useCallback(() => setVisible(true), []);

  return { visible, dismiss, reopen };
}

export function FirstUseGuide({ onDismiss }: { onDismiss: () => void }) {
  const copy = jaJP.authoring.canvas.firstUse;
  return (
    <section className="canvas-first-use" aria-label={copy.title}>
      <div className="canvas-first-use-heading">
        <strong>{copy.title}</strong>
        <button
          type="button"
          className="tertiary-action"
          onClick={onDismiss}
          aria-label={copy.dismiss}
          title={copy.dismiss}
        >
          ×
        </button>
      </div>
      <ol>
        <li>{copy.add}</li>
        <li>{copy.connect}</li>
        <li>{copy.execute}</li>
      </ol>
    </section>
  );
}
