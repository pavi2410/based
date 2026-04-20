/**
 * Terminal renderer for a single cell value.
 *
 * Lives in its own file because both `data-viewer` and a future
 * detached result viewer reach for the exact same "render null / bool
 * / number / object / long-string" logic. Keeping it colocated with
 * DataViewer blocked reuse and made the file harder to scan.
 *
 * The variants are intentionally simple and visual; semantic cell
 * rendering (clickable foreign-keys, rendered Markdown, etc.) is not
 * a Phase 2 goal.
 */
export function CellValue({ value }: { value: unknown }) {
  if (value === null || value === undefined) {
    return <span className="text-muted-foreground/60 italic">null</span>;
  }

  if (typeof value === "boolean") {
    return (
      <span
        className={
          value
            ? "text-emerald-600 dark:text-emerald-400"
            : "text-red-500 dark:text-red-400"
        }
      >
        {value.toString()}
      </span>
    );
  }

  if (typeof value === "number") {
    return (
      <span className="text-blue-600 dark:text-blue-400">
        {value.toLocaleString()}
      </span>
    );
  }

  if (typeof value === "object") {
    const json = JSON.stringify(value);
    return (
      <span
        className="text-amber-600 dark:text-amber-400 max-w-[200px] truncate inline-block align-bottom"
        title={json}
      >
        {json}
      </span>
    );
  }

  const strValue = String(value);
  if (strValue.length > 80) {
    return (
      <span
        className="max-w-[300px] truncate inline-block align-bottom"
        title={strValue}
      >
        {strValue}
      </span>
    );
  }

  return <span>{strValue}</span>;
}
