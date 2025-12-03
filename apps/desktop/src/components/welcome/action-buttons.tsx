import { Button } from "@/components/ui/button";
import { FolderOpenIcon, BookOpenIcon } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { useState } from "react";
import { useNavigate } from "@tanstack/react-router";
import { readProjectConfig, initializeProject } from "@/stores/projects";
import { addRecentProject } from "@/stores/project-state";
import { toast } from "sonner";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

export function ActionButtons() {
  const navigate = useNavigate();
  const [showInitDialog, setShowInitDialog] = useState(false);
  const [initPath, setInitPath] = useState("");
  const [projectName, setProjectName] = useState("");
  const [isInitializing, setIsInitializing] = useState(false);

  const handleOpenFolder = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: "Open Project Folder",
      });

      if (!selected || typeof selected !== "string") {
        return;
      }

      // Check if .based/config.toml exists
      try {
        const config = await readProjectConfig(selected);

        // Project exists, add to recent and navigate
        addRecentProject({
          path: selected,
          name: config.name,
          lastOpened: new Date().toISOString(),
        });

        const projectId = btoa(selected);
        navigate({ to: "/project/$projectId", params: { projectId } });
      } catch (error) {
        // Project not initialized, show init dialog
        setInitPath(selected);
        // Extract folder name for default project name
        const folderName = selected.split("/").pop() || "New Project";
        setProjectName(folderName);
        setShowInitDialog(true);
      }
    } catch (error) {
      toast.error("Failed to open folder", {
        description: error instanceof Error ? error.message : String(error),
      });
    }
  };

  const handleInitialize = async () => {
    if (!initPath || !projectName.trim()) {
      return;
    }

    setIsInitializing(true);
    try {
      await initializeProject(initPath);

      // Read the config to verify it was created
      const config = await readProjectConfig(initPath);

      // Add to recent projects
      addRecentProject({
        path: initPath,
        name: config.name,
        lastOpened: new Date().toISOString(),
      });

      toast.success("Project initialized successfully");

      // Navigate to project
      const projectId = btoa(initPath);
      navigate({ to: "/project/$projectId", params: { projectId } });

      setShowInitDialog(false);
      setInitPath("");
      setProjectName("");
    } catch (error) {
      toast.error("Failed to initialize project", {
        description: error instanceof Error ? error.message : String(error),
      });
    } finally {
      setIsInitializing(false);
    }
  };

  return (
    <>
      <div className="flex flex-col sm:flex-row gap-4 items-center justify-center">
        <Button
          size="lg"
          className="w-full sm:w-auto"
          onClick={handleOpenFolder}
        >
          <FolderOpenIcon className="mr-2 size-5" />
          Open Folder
        </Button>
        <Button
          size="lg"
          variant="outline"
          className="w-full sm:w-auto"
          disabled
        >
          <BookOpenIcon className="mr-2 size-5" />
          Example Projects
        </Button>
      </div>

      <Dialog open={showInitDialog} onOpenChange={setShowInitDialog}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Initialize Based Project</DialogTitle>
            <DialogDescription>
              This folder doesn't contain a Based project. Would you like to initialize one?
            </DialogDescription>
          </DialogHeader>

          <div className="space-y-4 py-4">
            <div className="space-y-2">
              <Label htmlFor="project-path">Project Path</Label>
              <Input
                id="project-path"
                value={initPath}
                disabled
                className="font-mono text-sm"
              />
            </div>

            <div className="space-y-2">
              <Label htmlFor="project-name">Project Name</Label>
              <Input
                id="project-name"
                value={projectName}
                onChange={(e) => setProjectName(e.target.value)}
                placeholder="My Project"
              />
            </div>
          </div>

          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => setShowInitDialog(false)}
              disabled={isInitializing}
            >
              Cancel
            </Button>
            <Button
              onClick={handleInitialize}
              disabled={isInitializing || !projectName.trim()}
            >
              {isInitializing ? "Initializing..." : "Initialize Project"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}
