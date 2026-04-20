/**
 * Export helpers for the browse grid.
 *
 * We emit plain CSV and JSON from the already-loaded page of rows.
 * Large exports (beyond the current page) aren't a v1 goal — the
 * safer long-term path is an `export_*` Tauri command that streams
 * directly from the connection. Until then, "export" means "save
 * what you see".
 *
 * We trigger the download via an anchor + blob URL rather than a
 * filesystem plugin so the app keeps working without plugin-fs
 * capabilities. The browser runtime inside Tauri handles the
 * platform-native save dialog.
 */

export interface ExportRow {
  [column: string]: unknown;
}

export function exportAsCsv(
  suggestedName: string,
  columns: string[],
  rows: ExportRow[],
): void {
  download(`${suggestedName}.csv`, toCsv(columns, rows), "text/csv");
}

export function exportAsJson(
  suggestedName: string,
  columns: string[],
  rows: ExportRow[],
): void {
  const objRows = rows.map((r) => {
    const o: ExportRow = {};
    for (const c of columns) o[c] = r[c];
    return o;
  });
  download(
    `${suggestedName}.json`,
    JSON.stringify(objRows, null, 2),
    "application/json",
  );
}

function download(filename: string, text: string, mime: string): void {
  const blob = new Blob([text], { type: mime });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  // Release the object URL after the click has initiated the download.
  setTimeout(() => URL.revokeObjectURL(url), 0);
}

function toCsv(columns: string[], rows: ExportRow[]): string {
  const lines: string[] = [];
  lines.push(columns.map(csvField).join(","));
  for (const row of rows) {
    lines.push(columns.map((c) => csvField(row[c])).join(","));
  }
  // CSV convention: trailing newline.
  return `${lines.join("\n")}\n`;
}

function csvField(value: unknown): string {
  if (value === null || value === undefined) return "";
  let s: string;
  if (typeof value === "string") {
    s = value;
  } else if (typeof value === "object") {
    try {
      s = JSON.stringify(value);
    } catch {
      s = String(value);
    }
  } else {
    s = String(value);
  }
  // Quote fields that contain delimiter, quote, or newline.
  const needsQuote = /[",\n\r]/.test(s);
  if (!needsQuote) return s;
  return `"${s.replace(/"/g, '""')}"`;
}
