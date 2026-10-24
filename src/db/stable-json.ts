/** Lexicographic order on UTF-16 code units (JavaScript string order), not locale rules. */
export function compareUtf8Keys(left: string, right: string): number {
  if (left < right) {
    return -1;
  }
  if (left > right) {
    return 1;
  }
  return 0;
}

export function stableValue(value: unknown): unknown {
  if (Array.isArray(value)) {
    return value.map(stableValue);
  }
  if (value !== null && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value as Record<string, unknown>)
        .sort(([left], [right]) => compareUtf8Keys(left, right))
        .map(([key, child]) => [key, stableValue(child)]),
    );
  }
  return value;
}

export function deterministicJson(value: unknown): Buffer {
  return Buffer.from(`${JSON.stringify(stableValue(value), null, 2)}\n`, "utf8");
}

/** True when two JSON text columns carry the same logical value (key order ignored). */
export function semanticJsonEqual(left: string, right: string): boolean {
  try {
    const normalizedLeft = stableValue(JSON.parse(left) as unknown);
    const normalizedRight = stableValue(JSON.parse(right) as unknown);
    return JSON.stringify(normalizedLeft) === JSON.stringify(normalizedRight);
  } catch {
    return false;
  }
}
