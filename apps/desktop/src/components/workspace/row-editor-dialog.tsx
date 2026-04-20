/**
 * Row editor dialog for inserting or editing a row / document.
 *
 * Modes:
 *  - `insert`: all fields start empty, all fields participate in the
 *    final VALUES / document payload.
 *  - `edit`: fields start with the existing row's values, PK columns
 *    are locked (can't be changed), and only *modified* fields end up
 *    in the `changes` payload so the SQL UPDATE is minimal.
 *
 * The editor is intentionally JSON-typed: every input is a textarea
 * that parses as JSON on submit (with a few conveniences — unquoted
 * strings and empty fields both map to JSON primitives). This is
 * the simplest way to cover MongoDB's nested documents alongside
 * scalar SQL columns without building a dedicated per-type editor.
 */
import { useState, useMemo } from "react";
import { KeyIcon, Loader2Icon } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import type { ColumnDescription, TableDescription } from "@/types/project";
import { useRowMutations, type RowMap } from "@/hooks/use-row-mutations";

export type EditorMode =
  | { kind: "insert" }
  | { kind: "edit"; originalRow: RowMap; pk: RowMap };

interface Props {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  selectedTable: string;
  description: TableDescription;
  mode: EditorMode;
}

export function RowEditorDialog({
  open,
  onOpenChange,
  selectedTable,
  description,
  mode,
}: Props) {
  const mut = useRowMutations(selectedTable);
  const [submitting, setSubmitting] = useState(false);

  // Initial text values per column. Edit mode echoes the existing row;
  // insert mode starts blank.
  const initialValues = useMemo<Record<string, string>>(() => {
    const out: Record<string, string> = {};
    for (const c of description.columns) {
      if (mode.kind === "edit") {
        const v = mode.originalRow[c.name];
        out[c.name] = v === null || v === undefined ? "" : stringify(v);
      } else {
        out[c.name] = "";
      }
    }
    return out;
  }, [description.columns, mode]);

  const [values, setValues] = useState<Record<string, string>>(initialValues);

  const isEditing = mode.kind === "edit";
  const pkColumns = useMemo(
    () => description.columns.filter((c) => c.isPrimaryKey),
    [description.columns],
  );

  const handleSubmit = async () => {
    try {
      const parsed = parseValues(description.columns, values);

      if (mode.kind === "insert") {
        setSubmitting(true);
        await mut.insertRow(parsed);
        toast.success(`Inserted into ${selectedTable}`);
      } else {
        const changes: RowMap = {};
        for (const [k, v] of Object.entries(parsed)) {
          const prev = mode.originalRow[k] ?? null;
          if (!deepEqual(v, prev)) {
            changes[k] = v;
          }
        }
        if (Object.keys(changes).length === 0) {
          toast.info("No changes to save");
          onOpenChange(false);
          return;
        }
        setSubmitting(true);
        await mut.updateRow({
          pk: mode.pk,
          changes,
          originalRow: mode.originalRow,
        });
        toast.success(`Updated row in ${selectedTable}`);
      }
      onOpenChange(false);
    } catch (e) {
      toast.error(
        e instanceof Error ? e.message : `Failed to save: ${String(e)}`,
      );
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl max-h-[80vh] flex flex-col">
        <DialogHeader>
          <DialogTitle className="text-sm">
            {isEditing ? "Edit row" : "Insert row"}
            <span className="ml-2 font-mono text-muted-foreground font-normal">
              {selectedTable}
            </span>
          </DialogTitle>
          <DialogDescription className="text-xs">
            {isEditing
              ? "Primary-key columns are locked. Only modified fields are sent to the database."
              : "Fields left blank become NULL. Values are parsed as JSON; plain text without quotes becomes a string."}
          </DialogDescription>
        </DialogHeader>

        <div className="flex-1 overflow-auto space-y-2 pr-1">
          {description.columns.length === 0 ? (
            <p className="text-xs text-muted-foreground italic">
              No columns reported for this {description.kind}.
            </p>
          ) : (
            description.columns.map((col) => (
              <Field
                key={col.name}
                col={col}
                value={values[col.name] ?? ""}
                onChange={(v) => setValues((s) => ({ ...s, [col.name]: v }))}
                locked={isEditing && col.isPrimaryKey}
              />
            ))
          )}
        </div>

        <DialogFooter>
          <div className="flex-1 text-[11px] text-muted-foreground">
            {pkColumns.length === 0 && (
              <span>
                Warning: no primary key. Edits and deletes will be disabled
                after insert.
              </span>
            )}
          </div>
          <Button
            variant="outline"
            size="sm"
            className="h-7 text-xs"
            onClick={() => onOpenChange(false)}
            disabled={submitting}
          >
            Cancel
          </Button>
          <Button
            size="sm"
            className="h-7 text-xs"
            onClick={handleSubmit}
            disabled={submitting || description.columns.length === 0}
          >
            {submitting ? (
              <Loader2Icon className="size-3 animate-spin mr-1.5" />
            ) : null}
            {isEditing ? "Save" : "Insert"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function Field({
  col,
  value,
  onChange,
  locked,
}: {
  col: ColumnDescription;
  value: string;
  onChange: (v: string) => void;
  locked: boolean;
}) {
  return (
    <div className="grid grid-cols-[140px_1fr] gap-2 items-start">
      <Label
        htmlFor={`field-${col.name}`}
        className="text-xs font-mono pt-1 flex items-center gap-1 min-w-0"
      >
        {col.isPrimaryKey ? (
          <KeyIcon className="size-3 text-amber-500 shrink-0" />
        ) : null}
        <span className="truncate" title={col.name}>
          {col.name}
        </span>
      </Label>
      <div className="flex flex-col gap-0.5 min-w-0">
        <Textarea
          id={`field-${col.name}`}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          disabled={locked}
          rows={1}
          className="text-xs font-mono min-h-[28px] resize-y"
          placeholder={col.nullable ? "NULL" : undefined}
        />
        <span className="text-[10px] text-muted-foreground truncate">
          {col.dataType}
          {col.nullable ? "" : " · NOT NULL"}
          {col.default ? ` · default: ${col.default}` : ""}
        </span>
      </div>
    </div>
  );
}

// ----- value parsing --------------------------------------------------------

function stringify(v: unknown): string {
  if (typeof v === "string") return v;
  try {
    return JSON.stringify(v);
  } catch {
    return String(v);
  }
}

/**
 * Parse a raw string cell value using a pragmatic superset of JSON:
 *  - empty string → null
 *  - valid JSON (number, bool, object, array, quoted string) → parsed
 *  - anything else → the raw string
 *
 * This keeps basic SQL cell editing (type "42" → number, "hello" →
 * string) frictionless while still supporting nested MongoDB values
 * if the user types real JSON.
 */
function parseCell(raw: string): unknown {
  const s = raw.trim();
  if (s === "") return null;
  try {
    return JSON.parse(s);
  } catch {
    return raw;
  }
}

function parseValues(
  cols: ColumnDescription[],
  raw: Record<string, string>,
): RowMap {
  const out: RowMap = {};
  for (const c of cols) {
    out[c.name] = parseCell(raw[c.name] ?? "");
  }
  return out;
}

function deepEqual(a: unknown, b: unknown): boolean {
  if (a === b) return true;
  if (a === null || b === null) return false;
  if (typeof a !== typeof b) return false;
  if (typeof a === "object") {
    try {
      return JSON.stringify(a) === JSON.stringify(b);
    } catch {
      return false;
    }
  }
  return false;
}
