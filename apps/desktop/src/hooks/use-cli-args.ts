import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useNavigate } from "@tanstack/react-router";

interface CliArgs {
  path?: string;
}

/**
 * Hook to handle CLI arguments on app startup
 * If a path is provided via CLI, it will:
 * 1. Find the Based project directory (searches upward for .based)
 * 2. Navigate to the project workspace
 */
export function useCliArgs() {
  const navigate = useNavigate();
  const [isProcessing, setIsProcessing] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    async function processCliArgs() {
      try {
        // Get CLI arguments
        const matches = await invoke<{ args: CliArgs }>("plugin:cli|cli_matches");

        if (matches.args.path) {
          const providedPath = matches.args.path;
          console.log("CLI path provided:", providedPath);

          // Find the Based project directory
          try {
            const projectPath = await invoke<string>("find_based_project", {
              startPath: providedPath,
            });

            console.log("Found Based project at:", projectPath);

            // Navigate to project workspace
            const projectId = btoa(projectPath);
            navigate({ to: "/project/$projectId", params: { projectId } });
          } catch (err) {
            const errorMsg = err instanceof Error ? err.message : String(err);
            console.error("Failed to find Based project:", errorMsg);
            setError(errorMsg);
          }
        }
      } catch (err) {
        // CLI plugin not available or no args - this is fine, just means app was opened normally
        console.log("No CLI args or CLI plugin not available");
      } finally {
        setIsProcessing(false);
      }
    }

    processCliArgs();
  }, [navigate]);

  return { isProcessing, error };
}
