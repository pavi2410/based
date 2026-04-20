/**
 * `useRowMutations` — engine-agnostic CRUD for the currently-selected
 * table/collection.
 *
 * Responsibilities:
 *  - Pick the right `update_*` / `insert_*` / `delete_*` command for
 *    the active engine.
 *  - Invalidate the browse query so the grid re-renders after a
 *    successful mutation.
 *  - Record an inverse operation on the in-session undo stack so
 *    `undo()` can replay it.
 *
 * The hook deliberately does NOT know which columns are the primary
 * key; the caller (an editor dialog or context menu) is expected to
 * build the `pk` payload from the fetched `TableDescription`. This
 * keeps the mutation API symmetric across engines and lets the UI
 * show a "this row can't be edited" state early when a PK isn't
 * available.
 */
import { useQueryClient } from "@tanstack/react-query";
import { useCallback } from "react";
import { cmd } from "@/commands";
import type { JsonValue } from "@/bindings";
import { queryKeys } from "@/lib/query-keys";
import { useConnection } from "@/routes/project.$projectId/conn.$connKey";
import {
  $undoStack,
  pushUndoEntry,
  type UndoEntry,
} from "@/stores/row-mutations-store";

export type RowMap = Record<string, unknown>;

/**
 * The generated Rust bindings type each payload as
 * `Partial<{ [key in string]: JsonValue }>`. `RowMap` uses `unknown`
 * so the UI doesn't have to enforce JsonValue at every call site;
 * we pass it through `asJson()` at the IPC boundary. Values that
 * aren't JSON-serialisable (e.g. Date) will surface as a clear error
 * when the Rust side deserialises.
 */
function asJson(row: RowMap): Partial<{ [key in string]: JsonValue }> {
  return row as Partial<{ [key in string]: JsonValue }>;
}

export interface RowMutations {
  canUndo: boolean;
  updateRow: (args: {
    pk: RowMap;
    changes: RowMap;
    originalRow: RowMap;
  }) => Promise<void>;
  insertRow: (values: RowMap) => Promise<void>;
  deleteRow: (args: { pk: RowMap; originalRow: RowMap }) => Promise<void>;
  undo: () => Promise<void>;
}

export function useRowMutations(selectedTable: string | null): RowMutations {
  const { connKey, connectionConfig, projectPath, selectedSchema } =
    useConnection();
  const queryClient = useQueryClient();
  const engine = connectionConfig.engine;

  const invalidate = useCallback(() => {
    // Both the browse query and the describe query are affected — we
    // refresh both so row counts / sampled columns stay fresh on edit.
    queryClient.invalidateQueries({
      queryKey: queryKeys.conn.tableDataAll(projectPath, connKey),
    });
    queryClient.invalidateQueries({
      queryKey: queryKeys.conn.tableDescribeAll(projectPath, connKey),
    });
  }, [queryClient, projectPath, connKey]);

  const updateRow = useCallback(
    async ({
      pk,
      changes,
      originalRow,
    }: {
      pk: RowMap;
      changes: RowMap;
      originalRow: RowMap;
    }) => {
      if (!selectedTable) throw new Error("No table selected");
      await dispatchUpdate(engine, {
        projectPath,
        connKey,
        schema: selectedSchema,
        table: selectedTable,
        pk,
        changes,
      });

      const inverseChanges: RowMap = {};
      for (const k of Object.keys(changes)) {
        inverseChanges[k] = originalRow[k] ?? null;
      }
      pushUndoEntry(
        buildUpdateUndo(engine, {
          projectPath,
          connKey,
          schema: selectedSchema,
          table: selectedTable,
          pk,
          changes: inverseChanges,
          label: `Update row in ${selectedTable}`,
        }),
      );
      invalidate();
    },
    [engine, projectPath, connKey, selectedSchema, selectedTable, invalidate],
  );

  const insertRow = useCallback(
    async (values: RowMap) => {
      if (!selectedTable) throw new Error("No table selected");
      await dispatchInsert(engine, {
        projectPath,
        connKey,
        schema: selectedSchema,
        table: selectedTable,
        values,
      });
      // Inverse for insert is delete-by-pk — but we don't know the
      // auto-generated PK without re-reading. Skip undo for inserts
      // rather than fake one; the toast copy flags this clearly.
      invalidate();
    },
    [engine, projectPath, connKey, selectedSchema, selectedTable, invalidate],
  );

  const deleteRow = useCallback(
    async ({ pk, originalRow }: { pk: RowMap; originalRow: RowMap }) => {
      if (!selectedTable) throw new Error("No table selected");
      await dispatchDelete(engine, {
        projectPath,
        connKey,
        schema: selectedSchema,
        table: selectedTable,
        pk,
      });
      pushUndoEntry(
        buildDeleteUndo(engine, {
          projectPath,
          connKey,
          schema: selectedSchema,
          table: selectedTable,
          values: originalRow,
          label: `Delete row from ${selectedTable}`,
        }),
      );
      invalidate();
    },
    [engine, projectPath, connKey, selectedSchema, selectedTable, invalidate],
  );

  const undo = useCallback(async () => {
    const entry = $undoStack.get().at(-1);
    if (!entry) return;
    await entry.apply();
    $undoStack.set($undoStack.get().slice(0, -1));
    invalidate();
  }, [invalidate]);

  return {
    canUndo: $undoStack.get().length > 0,
    updateRow,
    insertRow,
    deleteRow,
    undo,
  };
}

type Engine = "sqlite" | "postgres" | "mongodb";

type Schema = string | null | undefined;

interface UpdateArgs {
  projectPath: string;
  connKey: string;
  schema: Schema;
  table: string;
  pk: RowMap;
  changes: RowMap;
}

interface InsertArgs {
  projectPath: string;
  connKey: string;
  schema: Schema;
  table: string;
  values: RowMap;
}

interface DeleteArgs {
  projectPath: string;
  connKey: string;
  schema: Schema;
  table: string;
  pk: RowMap;
}

async function dispatchUpdate(engine: Engine, a: UpdateArgs): Promise<number> {
  switch (engine) {
    case "sqlite":
      return await cmd.updateSqliteRow(
        a.projectPath,
        a.connKey,
        a.table,
        asJson(a.pk),
        asJson(a.changes),
      );
    case "postgres":
      return await cmd.updatePostgresRow(
        a.projectPath,
        a.connKey,
        a.schema ?? "public",
        a.table,
        asJson(a.pk),
        asJson(a.changes),
      );
    case "mongodb":
      return await cmd.updateMongodbDocument(
        a.projectPath,
        a.connKey,
        a.table,
        asJson(a.pk),
        asJson(a.changes),
      );
  }
}

async function dispatchInsert(engine: Engine, a: InsertArgs): Promise<number> {
  switch (engine) {
    case "sqlite":
      return await cmd.insertSqliteRow(
        a.projectPath,
        a.connKey,
        a.table,
        asJson(a.values),
      );
    case "postgres":
      return await cmd.insertPostgresRow(
        a.projectPath,
        a.connKey,
        a.schema ?? "public",
        a.table,
        asJson(a.values),
      );
    case "mongodb":
      return await cmd.insertMongodbDocument(
        a.projectPath,
        a.connKey,
        a.table,
        asJson(a.values),
      );
  }
}

async function dispatchDelete(engine: Engine, a: DeleteArgs): Promise<number> {
  switch (engine) {
    case "sqlite":
      return await cmd.deleteSqliteRow(
        a.projectPath,
        a.connKey,
        a.table,
        asJson(a.pk),
      );
    case "postgres":
      return await cmd.deletePostgresRow(
        a.projectPath,
        a.connKey,
        a.schema ?? "public",
        a.table,
        asJson(a.pk),
      );
    case "mongodb":
      return await cmd.deleteMongodbDocument(
        a.projectPath,
        a.connKey,
        a.table,
        asJson(a.pk),
      );
  }
}

function buildUpdateUndo(
  engine: Engine,
  a: UpdateArgs & { label: string },
): UndoEntry {
  return {
    label: a.label,
    apply: async () => {
      await dispatchUpdate(engine, a);
    },
  };
}

function buildDeleteUndo(
  engine: Engine,
  a: InsertArgs & { label: string },
): UndoEntry {
  return {
    label: a.label,
    apply: async () => {
      await dispatchInsert(engine, a);
    },
  };
}
