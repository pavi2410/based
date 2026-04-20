/**
 * Cell detail side panel.
 *
 * Shows the full, unstyled value of a cell the user clicked on.
 * Strings stay plain; objects/arrays get a readable JSON pretty-print.
 * Blobs show their size marker and nothing else.
 *
 * This is the "long TEXT column that the grid truncates" escape hatch
 * — a staple of every DataGrip-style client.
 */
import { XIcon, CopyIcon, CheckIcon } from "lucide-react";
import { useState } from "react";
import { Button } from "@/components/ui/button";

interface Props {
  columnId: string;
  value: unknown;
  onClose: () => void;
}

export function CellDetailPanel({ columnId, value, onClose }: Props) {
  const [copied, setCopied] = useState(false);
  const pretty = formatValue(value);
  const isNull = value === null || value === undefined;

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(pretty);
      setCopied(true);
      setTimeout(() => setCopied(false), 1200);
    } catch {
      // No-op: clipboard may be blocked in restricted envs. The copy
      // button visibly flashes only on success, so failure is silent.
    }
  };

  return (
    <div className="flex flex-col h-full border-t bg-background">
      <div className="flex items-center justify-between px-3 py-1 border-b bg-muted/20">
        <div className="flex items-center gap-2 min-w-0">
          <span className="text-[10px] uppercase tracking-wide text-muted-foreground">
            Cell
          </span>
          <span className="text-xs font-mono truncate" title={columnId}>
            {columnId}
          </span>
          <span className="text-[10px] text-muted-foreground shrink-0">
            {describeType(value)}
          </span>
        </div>
        <div className="flex items-center gap-0.5">
          <Button
            variant="ghost"
            size="sm"
            className="h-6 px-2 text-[11px]"
            onClick={copy}
            disabled={isNull}
          >
            {copied ? (
              <CheckIcon className="size-3 mr-1" />
            ) : (
              <CopyIcon className="size-3 mr-1" />
            )}
            {copied ? "Copied" : "Copy"}
          </Button>
          <Button
            variant="ghost"
            size="icon"
            className="size-6"
            onClick={onClose}
            aria-label="Close cell detail"
          >
            <XIcon className="size-3.5" />
          </Button>
        </div>
      </div>
      <div className="flex-1 overflow-auto p-2">
        {isNull ? (
          <span className="text-xs italic text-muted-foreground">null</span>
        ) : (
          <pre className="text-xs font-mono whitespace-pre-wrap wrap-break-word">
            {pretty}
          </pre>
        )}
      </div>
    </div>
  );
}

function describeType(value: unknown): string {
  if (value === null || value === undefined) return "null";
  if (Array.isArray(value)) return `array (${value.length})`;
  if (typeof value === "object") return "object";
  return typeof value;
}

function formatValue(value: unknown): string {
  if (value === null || value === undefined) return "";
  if (typeof value === "string") return value;
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}
