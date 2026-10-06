export type BlockAnnotations = Readonly<Record<string, string>>;

export function getBlockAnnotation(
  annotations: BlockAnnotations,
  blockId: string,
): string {
  return annotations[blockId] ?? "";
}

export function setBlockAnnotation(
  annotations: BlockAnnotations,
  blockId: string,
  text: string,
): BlockAnnotations {
  if (text.length === 0) {
    return removeBlockAnnotation(annotations, blockId);
  }

  if (annotations[blockId] === text) {
    return annotations;
  }

  return {
    ...annotations,
    [blockId]: text,
  };
}

export function removeBlockAnnotation(
  annotations: BlockAnnotations,
  blockId: string,
): BlockAnnotations {
  if (!(blockId in annotations)) {
    return annotations;
  }

  const next = { ...annotations };
  delete next[blockId];
  return next;
}
