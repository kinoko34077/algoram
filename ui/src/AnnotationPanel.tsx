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
    <section className="annotation-panel" aria-label={jaJP.authoring.annotation.inputLabel}>
      <div className="annotation-heading">
        <div>
          <p className="eyebrow">{jaJP.authoring.annotation.localAnnotation}</p>
          <strong>{jaJP.authoring.annotation.editorOnlyNote}</strong>
        </div>
        <button
          type="button"
          className="danger-action"
          onClick={onRemove}
          disabled={value.length === 0}
        >
          {jaJP.authoring.annotation.remove}
        </button>
      </div>

      <label htmlFor={fieldId}>
        <span>{jaJP.authoring.annotation.annotationForBlock}</span>
        <textarea
          id={fieldId}
          value={value}
          onChange={(event) => onChange(event.target.value)}
          rows={4}
          placeholder={jaJP.authoring.annotation.placeholder}
        />
      </label>

      <small>
        Stored only in editor state. Imported source and execution semantics are
        unchanged.
      </small>
    </section>
  );
}
