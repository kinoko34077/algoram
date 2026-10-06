interface AnnotationPanelProps {
  blockId: string;
  value: string;
  onChange: (value: string) => void;
  onRemove: () => void;
}

export function AnnotationPanel({
  blockId,
  value,
  onChange,
  onRemove,
}: AnnotationPanelProps) {
  const fieldId = `annotation:${blockId}`;

  return (
    <section className="annotation-panel" aria-label="Local Block annotation">
      <div className="annotation-heading">
        <div>
          <p className="eyebrow">Local annotation</p>
          <strong>Editor-only note</strong>
        </div>
        <button
          type="button"
          className="tertiary-action"
          onClick={onRemove}
          disabled={value.length === 0}
        >
          Remove
        </button>
      </div>

      <label htmlFor={fieldId}>
        <span>Annotation for this Block</span>
        <textarea
          id={fieldId}
          value={value}
          onChange={(event) => onChange(event.target.value)}
          rows={4}
          placeholder="Add a local note…"
        />
      </label>

      <small>
        Stored only in editor state. Imported source and execution semantics are
        unchanged.
      </small>
    </section>
  );
}
