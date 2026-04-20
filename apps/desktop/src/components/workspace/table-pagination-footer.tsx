/**
 * Paginator strip rendered at the bottom of the DataViewer data grid.
 *
 * Separated from the DataViewer so the viewer doesn't have to own
 * both "what page to show" and "how to render the page control".
 * The parent keeps owning the `page` state so this component stays
 * pure; we don't introduce a second source of truth.
 */
import {
  ChevronLeftIcon,
  ChevronRightIcon,
  ChevronsLeftIcon,
  ChevronsRightIcon,
} from "lucide-react";
import { Button } from "@/components/ui/button";

export function TablePaginationFooter({
  page,
  totalPages,
  totalCount,
  pageSize,
  onPageChange,
}: {
  page: number;
  totalPages: number;
  totalCount: number;
  pageSize: number;
  onPageChange: (nextPage: number) => void;
}) {
  if (totalPages <= 1) return null;

  const startRow = page * pageSize + 1;
  const endRow = Math.min((page + 1) * pageSize, totalCount);

  return (
    <div className="flex items-center justify-between px-3 py-1.5 border-t bg-muted/20 text-xs">
      <span className="text-muted-foreground tabular-nums">
        {startRow.toLocaleString()}–{endRow.toLocaleString()} of{" "}
        {totalCount.toLocaleString()}
      </span>
      <div className="flex items-center gap-0.5">
        <Button
          variant="ghost"
          size="icon"
          className="size-6"
          onClick={() => onPageChange(0)}
          disabled={page === 0}
        >
          <ChevronsLeftIcon className="size-3.5" />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          className="size-6"
          onClick={() => onPageChange(Math.max(0, page - 1))}
          disabled={page === 0}
        >
          <ChevronLeftIcon className="size-3.5" />
        </Button>
        <span className="px-2 text-muted-foreground tabular-nums min-w-[60px] text-center">
          {page + 1} / {totalPages}
        </span>
        <Button
          variant="ghost"
          size="icon"
          className="size-6"
          onClick={() => onPageChange(Math.min(totalPages - 1, page + 1))}
          disabled={page >= totalPages - 1}
        >
          <ChevronRightIcon className="size-3.5" />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          className="size-6"
          onClick={() => onPageChange(totalPages - 1)}
          disabled={page >= totalPages - 1}
        >
          <ChevronsRightIcon className="size-3.5" />
        </Button>
      </div>
    </div>
  );
}
