import { useEffect, useId, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import { GLOSS_HOLD_MS, hasMovedBeyondGlossSlop } from "./glossGesture";

interface GlossableProps {
  shortLabel: string;
  fullLabel: string;
}

/**
 * Shared gloss primitive. No Graph mutation, no drag prevention.
 * Native hover, keyboard focus and actual touch-hold share the same text.
 */
export function Glossable({ shortLabel, fullLabel }: GlossableProps) {
  const glossId = useId();
  const hasGloss = Boolean(fullLabel && fullLabel !== shortLabel);
  const [held, setHeld] = useState(false);
  const [dismissed, setDismissed] = useState(false);
  const pending = useRef<{
    id: number;
    x: number;
    y: number;
    timer: ReturnType<typeof setTimeout>;
  } | null>(null);

  const clearPending = () => {
    if (pending.current) {
      clearTimeout(pending.current.timer);
      pending.current = null;
    }
  };

  useEffect(() => () => {
    if (pending.current) clearTimeout(pending.current.timer);
  }, []);

  const start = (event: ReactPointerEvent<HTMLSpanElement>) => {
    if (!hasGloss || event.pointerType !== "touch") return;
    clearPending();
    setHeld(false);
    setDismissed(false);
    const id = event.pointerId;
    const x = event.clientX;
    const y = event.clientY;
    pending.current = {
      id, x, y,
      timer: setTimeout(() => {
        if (pending.current?.id === id) {
          setHeld(true);
          pending.current = null;
        }
      }, GLOSS_HOLD_MS),
    };
  };

  const move = (event: ReactPointerEvent<HTMLSpanElement>) => {
    const press = pending.current;
    if (press && press.id === event.pointerId &&
        hasMovedBeyondGlossSlop(press.x, press.y, event.clientX, event.clientY)) {
      clearPending();
      setHeld(false);
    }
  };

  const finish = () => {
    clearPending();
    setHeld(false);
  };

  return (
    <span
      className={["glossable", held ? "held" : "", dismissed ? "dismissed" : ""].filter(Boolean).join(" ")}
      tabIndex={hasGloss ? 0 : -1}
      aria-label={hasGloss ? shortLabel : undefined}
      aria-describedby={hasGloss ? glossId : undefined}
      onPointerDown={start}
      onPointerMove={move}
      onPointerUp={finish}
      onPointerCancel={finish}
      onPointerLeave={finish}
      onFocus={() => setDismissed(false)}
      onBlur={() => {
        setDismissed(false);
        finish();
      }}
      onKeyDown={(event) => {
        if (event.key === "Escape" && hasGloss) {
          event.stopPropagation();
          setDismissed(true);
          finish();
        }
      }}
    >
      <span className="gloss-label">{shortLabel}</span>
      {hasGloss ? (
        <span className="gloss-hint" id={glossId} role="tooltip">
          {fullLabel}
        </span>
      ) : null}
    </span>
  );
}
