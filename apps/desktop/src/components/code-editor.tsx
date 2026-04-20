import { json } from "@codemirror/lang-json";
import { PostgreSQL, SQLite, StandardSQL, sql } from "@codemirror/lang-sql";
import { keymap } from "@codemirror/view";
import { vscodeDark } from "@uiw/codemirror-theme-vscode";
import CodeMirror from "@uiw/react-codemirror";
import { useMemo } from "react";
import { useTheme } from "@/components/theme-provider";
import type { Engine } from "@/types/project";

export type SqlSchema = Record<string, string[]>;

export interface CodeEditorProps {
  value: string;
  onChange: (value: string) => void;
  language: "sql" | "json";
  className?: string;
  placeholder?: string;
  /**
   * SQL dialect hint. Selects the right keywords/identifier quoting for
   * completion; ignored for JSON.
   */
  engine?: Engine;
  /**
   * Map of `table -> column[]` used to drive CodeMirror's SQL
   * autocomplete. When provided the user gets schema-aware completion
   * for `SELECT * FROM <Tab>` and `table.<Tab>` style access.
   */
  sqlSchema?: SqlSchema;
  /**
   * Invoked when the user hits `Mod-Enter` inside the editor. Wired
   * through CodeMirror's keymap so it fires regardless of DOM focus
   * nesting (e.g. when the editor is inside a dialog).
   */
  onRun?: () => void;
  /**
   * Invoked when the user hits `Mod-s`. Prevents the browser "save
   * page" action.
   */
  onSave?: () => void;
}

export function CodeEditor({
  value,
  onChange,
  language,
  className,
  placeholder,
  engine,
  sqlSchema,
  onRun,
  onSave,
  ...props
}: CodeEditorProps) {
  const { theme } = useTheme();
  const isDark =
    theme === "dark" ||
    (theme === "system" &&
      window.matchMedia("(prefers-color-scheme: dark)").matches);

  const extensions = useMemo(() => {
    const base =
      language === "json"
        ? [json()]
        : [
            sql({
              dialect:
                engine === "postgres"
                  ? PostgreSQL
                  : engine === "sqlite"
                    ? SQLite
                    : StandardSQL,
              schema: sqlSchema,
              upperCaseKeywords: true,
            }),
          ];

    const shortcuts = keymap.of([
      {
        key: "Mod-Enter",
        preventDefault: true,
        run: () => {
          onRun?.();
          return true;
        },
      },
      {
        key: "Mod-s",
        preventDefault: true,
        run: () => {
          onSave?.();
          return true;
        },
      },
    ]);
    return [...base, shortcuts];
  }, [language, engine, sqlSchema, onRun, onSave]);

  return (
    <CodeMirror
      value={value}
      onChange={onChange}
      extensions={extensions}
      theme={isDark ? vscodeDark : "light"}
      className={className}
      placeholder={placeholder}
      style={{ fontSize: "13px" }}
      basicSetup={{
        lineNumbers: true,
        highlightActiveLine: true,
        highlightSelectionMatches: true,
        autocompletion: true,
      }}
      {...props}
    />
  );
}
