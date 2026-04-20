import {
  CalendarIcon,
  HashIcon,
  type LucideIcon,
  TextIcon,
} from "lucide-react";
import type { ColumnDataType } from "@/components/data-table-filter/core/types";

/**
 * Map database column types to filter data types
 */
export function dbTypeToFilterType(dbType: string): ColumnDataType {
  const normalized = dbType.toUpperCase();

  // Number types
  if (
    normalized.includes("INT") ||
    normalized.includes("REAL") ||
    normalized.includes("FLOAT") ||
    normalized.includes("DOUBLE") ||
    normalized.includes("DECIMAL") ||
    normalized.includes("NUMERIC") ||
    normalized.includes("SERIAL") ||
    normalized.includes("BIGSERIAL") ||
    normalized.includes("SMALLSERIAL")
  ) {
    return "number";
  }

  // Date types
  if (
    normalized.includes("DATE") ||
    normalized.includes("TIME") ||
    normalized.includes("TIMESTAMP")
  ) {
    return "date";
  }

  // Default to text for everything else (VARCHAR, TEXT, CHAR, BLOB, etc.)
  return "text";
}

/**
 * Get icon for filter type
 */
export function getFilterTypeIcon(type: ColumnDataType): LucideIcon {
  switch (type) {
    case "number":
      return HashIcon;
    case "date":
      return CalendarIcon;
    default:
      return TextIcon;
  }
}

/**
 * Filter state type for backend
 */
export interface FilterParam {
  columnId: string;
  type: string;
  operator: string;
  values: (string | number | boolean | null)[];
}
