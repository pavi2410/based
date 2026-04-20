/**
 * Schema Inspector panel.
 *
 * Rendered in place of the data grid when the user switches a table
 * tab to the "Structure" view. Shows columns (with types / nullability
 * / defaults / PK flags), indexes, and foreign keys, backed by the
 * engine-specific `describe_*` commands.
 *
 * The panel is intentionally self-contained: it takes the current
 * connection context via `useConnection()` and the selected table
 * identifier via props so it can be reused outside the DataViewer
 * later (e.g. as a detached inspector window, see Phase 1 pop-out
 * todo).
 */
import { useQuery } from "@tanstack/react-query";
import {
  Loader2Icon,
  KeyIcon,
  LinkIcon,
  ColumnsIcon,
  RefreshCwIcon,
  RowsIcon,
} from "lucide-react";
import { cmd } from "@/commands";
import { Button } from "@/components/ui/button";
import { queryKeys } from "@/lib/query-keys";
import { useWorkspace } from "@/hooks/use-workspace";
import type { TableDescription } from "@/types/project";

export function SchemaInspector({ selectedTable }: { selectedTable: string }) {
  const { connKey, projectPath, selectedSchema, engine } = useWorkspace();

  const descriptionQuery = useQuery({
    queryKey: queryKeys.conn.tableDescribe(
      projectPath,
      connKey,
      engine,
      selectedSchema,
      selectedTable,
    ),
    queryFn: async (): Promise<TableDescription> => {
      switch (engine) {
        case "sqlite":
          return await cmd.describeSqliteTable(
            projectPath,
            connKey,
            selectedTable,
          );
        case "postgres":
          return await cmd.describePostgresTable(
            projectPath,
            connKey,
            selectedSchema || "public",
            selectedTable,
          );
        case "mongodb":
          return await cmd.describeMongodbCollection(
            projectPath,
            connKey,
            selectedTable,
          );
        default:
          throw new Error(`Unsupported engine: ${engine}`);
      }
    },
  });

  if (descriptionQuery.status === "pending") {
    return (
      <div className="flex items-center justify-center h-full">
        <Loader2Icon className="size-5 animate-spin text-muted-foreground" />
      </div>
    );
  }

  if (descriptionQuery.status === "error") {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="flex flex-col items-center gap-3 max-w-sm">
          <h2 className="text-sm font-medium text-destructive">
            Failed to describe {selectedTable}
          </h2>
          <p className="text-xs text-muted-foreground text-center">
            {descriptionQuery.error instanceof Error
              ? descriptionQuery.error.message
              : String(descriptionQuery.error)}
          </p>
          <Button
            variant="outline"
            size="sm"
            className="h-7 text-xs"
            onClick={() => descriptionQuery.refetch()}
          >
            <RefreshCwIcon className="size-3 mr-1.5" />
            Retry
          </Button>
        </div>
      </div>
    );
  }

  const desc = descriptionQuery.data;

  return (
    <div className="h-full overflow-auto text-xs">
      <div className="px-3 py-2 border-b bg-muted/10 flex items-center gap-3">
        <span className="text-muted-foreground">Type:</span>
        <span className="font-medium">{desc.kind}</span>
        {desc.rowCount !== null && (
          <>
            <span className="text-muted-foreground">Rows:</span>
            <span className="font-medium tabular-nums">
              {desc.rowCount.toLocaleString()}
              {engine !== "sqlite" && (
                <span className="text-muted-foreground ml-1">(est.)</span>
              )}
            </span>
          </>
        )}
      </div>

      <Section
        title="Columns"
        icon={<ColumnsIcon className="size-3.5" />}
        count={desc.columns.length}
      >
        {desc.columns.length === 0 ? (
          <EmptyState>No columns reported.</EmptyState>
        ) : (
          <table className="w-full border-collapse">
            <thead>
              <tr className="text-left text-[10px] uppercase tracking-wide text-muted-foreground">
                <Th className="w-6 pl-3" />
                <Th>Name</Th>
                <Th>Type</Th>
                <Th className="w-20">Nullable</Th>
                <Th>Default</Th>
              </tr>
            </thead>
            <tbody>
              {desc.columns.map((col) => (
                <tr
                  key={col.name}
                  className="border-t hover:bg-muted/30 transition-colors"
                >
                  <Td className="pl-3 text-muted-foreground">
                    {col.isPrimaryKey ? (
                      <KeyIcon
                        className="size-3 text-amber-500"
                        aria-label="Primary key"
                      />
                    ) : null}
                  </Td>
                  <Td className="font-mono">{col.name}</Td>
                  <Td className="text-muted-foreground">{col.dataType}</Td>
                  <Td>
                    {col.nullable ? (
                      <span className="text-muted-foreground">yes</span>
                    ) : (
                      <span className="text-foreground">NOT NULL</span>
                    )}
                  </Td>
                  <Td className="font-mono text-muted-foreground">
                    {col.default ?? ""}
                  </Td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </Section>

      <Section
        title="Indexes"
        icon={<RowsIcon className="size-3.5" />}
        count={desc.indexes.length}
      >
        {desc.indexes.length === 0 ? (
          <EmptyState>No indexes.</EmptyState>
        ) : (
          <table className="w-full border-collapse">
            <thead>
              <tr className="text-left text-[10px] uppercase tracking-wide text-muted-foreground">
                <Th className="pl-3">Name</Th>
                <Th>Columns</Th>
                <Th className="w-20">Unique</Th>
                <Th className="w-20">Primary</Th>
              </tr>
            </thead>
            <tbody>
              {desc.indexes.map((idx) => (
                <tr
                  key={idx.name}
                  className="border-t hover:bg-muted/30 transition-colors"
                >
                  <Td className="pl-3 font-mono">{idx.name}</Td>
                  <Td className="font-mono text-muted-foreground">
                    {idx.columns.join(", ")}
                  </Td>
                  <Td>{idx.unique ? "yes" : ""}</Td>
                  <Td>{idx.primary ? "yes" : ""}</Td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </Section>

      <Section
        title="Foreign Keys"
        icon={<LinkIcon className="size-3.5" />}
        count={desc.foreignKeys.length}
      >
        {desc.foreignKeys.length === 0 ? (
          <EmptyState>No foreign keys.</EmptyState>
        ) : (
          <table className="w-full border-collapse">
            <thead>
              <tr className="text-left text-[10px] uppercase tracking-wide text-muted-foreground">
                <Th className="pl-3">Name</Th>
                <Th>Columns</Th>
                <Th>References</Th>
              </tr>
            </thead>
            <tbody>
              {desc.foreignKeys.map((fk, i) => (
                <tr
                  key={fk.name ?? `fk-${i}`}
                  className="border-t hover:bg-muted/30 transition-colors"
                >
                  <Td className="pl-3 font-mono text-muted-foreground">
                    {fk.name ?? ""}
                  </Td>
                  <Td className="font-mono">{fk.columns.join(", ")}</Td>
                  <Td className="font-mono text-muted-foreground">
                    {fk.referencedSchema ? `${fk.referencedSchema}.` : ""}
                    {fk.referencedTable}({fk.referencedColumns.join(", ")})
                  </Td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </Section>
    </div>
  );
}

function Section({
  title,
  icon,
  count,
  children,
}: {
  title: string;
  icon: React.ReactNode;
  count: number;
  children: React.ReactNode;
}) {
  return (
    <div className="border-b">
      <div className="flex items-center gap-1.5 px-3 h-7 bg-muted/20">
        <span className="text-muted-foreground">{icon}</span>
        <h3 className="text-xs font-semibold">{title}</h3>
        <span className="text-[10px] text-muted-foreground tabular-nums">
          {count}
        </span>
      </div>
      {children}
    </div>
  );
}

function Th({
  children,
  className = "",
}: {
  children?: React.ReactNode;
  className?: string;
}) {
  return <th className={`px-2 py-1.5 font-medium ${className}`}>{children}</th>;
}

function Td({
  children,
  className = "",
}: {
  children?: React.ReactNode;
  className?: string;
}) {
  return <td className={`px-2 py-1 ${className}`}>{children}</td>;
}

function EmptyState({ children }: { children: React.ReactNode }) {
  return (
    <div className="px-3 py-2 text-[11px] text-muted-foreground italic">
      {children}
    </div>
  );
}
