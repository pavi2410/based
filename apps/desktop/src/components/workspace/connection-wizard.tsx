/**
 * ConnectionWizard — add a new connection without hand-editing
 * `.based/config.toml`.
 *
 * Scope for v1 (intentional limits):
 *   - No multi-step flow. One dialog, engine picker at top, the
 *     engine-specific fields below. Every time I've seen a wizard
 *     split this into multiple pages it makes the "oh wait I want to
 *     change the engine" loop painful.
 *   - No fancy secret pickers. The password/URL fields are plain
 *     strings — users who want to reference env vars can still hand-
 *     edit config.toml for now. We'll layer `SecretValue` UX in a
 *     follow-up once we have the rest of the onboarding done.
 *   - "Test connection" uses the backend `test_connection` command
 *     so the user gets a real handshake (not just form validation)
 *     before committing.
 *   - Connection key defaults to a slugified label; user can override.
 */
import { useMutation } from "@tanstack/react-query";
import { Loader2Icon, PlugIcon, PlusIcon } from "lucide-react";
import { useMemo, useState } from "react";
import { toast } from "sonner";
import { cmd } from "@/commands";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { ConnectionConfig, Engine, ProjectConfig } from "@/types/project";

export interface ConnectionWizardProps {
  projectPath: string;
  config: ProjectConfig;
  onSaved: (connKey: string) => void;
}

function slugify(input: string): string {
  return (
    input
      .trim()
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "_")
      .replace(/^_+|_+$/g, "") || "connection"
  );
}

function emptyConnection(engine: Engine): ConnectionConfig {
  return {
    label: null,
    engine,
    group: engine === "sqlite" ? "local" : "remote",
    disabled: null,
    order: null,
    color: null,
    icon: null,
    file: null,
    readonly: null,
    url: null,
    host: null,
    port: null,
    database: null,
    username: null,
    password: null,
    ssl: null,
  };
}

export function ConnectionWizard({
  projectPath,
  config,
  onSaved,
}: ConnectionWizardProps) {
  const [open, setOpen] = useState(false);
  const [engine, setEngine] = useState<Engine>("sqlite");
  const [label, setLabel] = useState("");
  const [customKey, setCustomKey] = useState("");
  const [form, setForm] = useState<ConnectionConfig>(emptyConnection("sqlite"));

  // Engine change resets engine-specific fields so stale values from a
  // previous pick don't sneak into the TOML.
  const updateEngine = (next: Engine) => {
    setEngine(next);
    setForm((prev) => ({ ...emptyConnection(next), label: prev.label }));
  };

  const connKey = useMemo(
    () => customKey.trim() || slugify(label),
    [customKey, label],
  );
  const keyCollides = connKey in config.connection;

  const testMutation = useMutation({
    mutationFn: async () => {
      await cmd.testConnection(projectPath, { ...form, engine, label });
    },
    onSuccess: () => toast.success("Connection OK"),
    onError: (err) =>
      toast.error("Connection failed", {
        description: err instanceof Error ? err.message : String(err),
      }),
  });

  const saveMutation = useMutation({
    mutationFn: async () => {
      const next: ProjectConfig = {
        ...config,
        connection: {
          ...config.connection,
          [connKey]: { ...form, engine, label: label || null },
        },
      };
      await cmd.writeProjectConfig(projectPath, next);
    },
    onSuccess: () => {
      toast.success("Connection saved");
      setOpen(false);
      onSaved(connKey);
    },
    onError: (err) =>
      toast.error("Failed to save connection", {
        description: err instanceof Error ? err.message : String(err),
      }),
  });

  const canSave = !!label.trim() && !keyCollides && !saveMutation.isPending;

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button size="sm" variant="outline" className="h-8">
          <PlusIcon className="size-4 mr-1" />
          Add connection
        </Button>
      </DialogTrigger>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle>New connection</DialogTitle>
          <DialogDescription>
            Adds a new entry to{" "}
            <code className="bg-muted px-1 rounded">.based/config.toml</code>.
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-3 py-2">
          <div className="space-y-1.5">
            <Label>Engine</Label>
            <Select value={engine} onValueChange={(v) => updateEngine(v as Engine)}>
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="sqlite">SQLite</SelectItem>
                <SelectItem value="postgres">PostgreSQL</SelectItem>
                <SelectItem value="mongodb">MongoDB</SelectItem>
              </SelectContent>
            </Select>
          </div>

          <div className="grid grid-cols-2 gap-3">
            <div className="space-y-1.5">
              <Label htmlFor="conn-label">Label</Label>
              <Input
                id="conn-label"
                value={label}
                onChange={(e) => setLabel(e.target.value)}
                placeholder="Production DB"
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="conn-key">Key</Label>
              <Input
                id="conn-key"
                value={customKey}
                onChange={(e) => setCustomKey(e.target.value)}
                placeholder={slugify(label) || "prod_db"}
                className="font-mono text-sm"
              />
              {keyCollides ? (
                <p className="text-xs text-destructive">
                  Key already exists in this project.
                </p>
              ) : null}
            </div>
          </div>

          {engine === "sqlite" ? (
            <div className="space-y-1.5">
              <Label htmlFor="conn-file">Database file</Label>
              <Input
                id="conn-file"
                value={form.file ?? ""}
                onChange={(e) =>
                  setForm((p) => ({ ...p, file: e.target.value || null }))
                }
                placeholder="sample.db (relative to project)"
                className="font-mono text-sm"
              />
            </div>
          ) : engine === "postgres" ? (
            <>
              <div className="grid grid-cols-3 gap-3">
                <div className="col-span-2 space-y-1.5">
                  <Label htmlFor="pg-host">Host</Label>
                  <Input
                    id="pg-host"
                    value={form.host ?? ""}
                    onChange={(e) =>
                      setForm((p) => ({ ...p, host: e.target.value || null }))
                    }
                    placeholder="localhost"
                  />
                </div>
                <div className="space-y-1.5">
                  <Label htmlFor="pg-port">Port</Label>
                  <Input
                    id="pg-port"
                    type="number"
                    value={form.port ?? ""}
                    onChange={(e) =>
                      setForm((p) => ({
                        ...p,
                        port: e.target.value ? Number(e.target.value) : null,
                      }))
                    }
                    placeholder="5432"
                  />
                </div>
              </div>
              <div className="grid grid-cols-2 gap-3">
                <div className="space-y-1.5">
                  <Label htmlFor="pg-db">Database</Label>
                  <Input
                    id="pg-db"
                    value={form.database ?? ""}
                    onChange={(e) =>
                      setForm((p) => ({
                        ...p,
                        database: e.target.value || null,
                      }))
                    }
                    placeholder="myapp"
                  />
                </div>
                <div className="space-y-1.5">
                  <Label htmlFor="pg-user">Username</Label>
                  <Input
                    id="pg-user"
                    value={form.username ?? ""}
                    onChange={(e) =>
                      setForm((p) => ({
                        ...p,
                        username: e.target.value || null,
                      }))
                    }
                    placeholder="postgres"
                  />
                </div>
              </div>
              <div className="space-y-1.5">
                <Label htmlFor="pg-pass">Password</Label>
                <Input
                  id="pg-pass"
                  type="password"
                  value={
                    typeof form.password === "string"
                      ? form.password
                      : ((form.password as { value?: string } | null)
                          ?.value ?? "")
                  }
                  onChange={(e) =>
                    setForm((p) => ({
                      ...p,
                      password: e.target.value
                        ? { value: e.target.value }
                        : null,
                    }))
                  }
                  placeholder="••••••"
                />
                <p className="text-[11px] text-muted-foreground">
                  Stored as a literal value in the project config. To use an
                  environment variable instead, edit the file manually:{" "}
                  <code className="bg-muted px-1 rounded">
                    password = {"{"} env = "PGPASSWORD" {"}"}
                  </code>
                  .
                </p>
              </div>
            </>
          ) : (
            <div className="space-y-1.5">
              <Label htmlFor="mongo-url">MongoDB URL</Label>
              <Input
                id="mongo-url"
                value={
                  typeof form.url === "string"
                    ? form.url
                    : ((form.url as { value?: string } | null)?.value ?? "")
                }
                onChange={(e) =>
                  setForm((p) => ({
                    ...p,
                    url: e.target.value ? { value: e.target.value } : null,
                  }))
                }
                placeholder="mongodb://localhost:27017/mydb"
                className="font-mono text-sm"
              />
            </div>
          )}
        </div>

        <DialogFooter className="gap-2">
          <Button
            variant="outline"
            onClick={() => testMutation.mutate()}
            disabled={testMutation.isPending}
          >
            {testMutation.isPending ? (
              <Loader2Icon className="size-4 mr-1 animate-spin" />
            ) : (
              <PlugIcon className="size-4 mr-1" />
            )}
            Test
          </Button>
          <Button onClick={() => saveMutation.mutate()} disabled={!canSave}>
            {saveMutation.isPending ? (
              <Loader2Icon className="size-4 mr-1 animate-spin" />
            ) : null}
            Save
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
