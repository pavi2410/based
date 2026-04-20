import { Button } from "@/components/ui/button";
import { FolderOpenIcon, SparklesIcon } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { useState } from "react";
import { useNavigate } from "@tanstack/react-router";
import { readProjectConfig, initializeProject } from "@/stores/projects";
import { addRecentProject } from "@/stores/project-state";
import { toast } from "sonner";
import { cmd } from "@/commands";
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
  const [isCreatingSample, setIsCreatingSample] = useState(false);

  const handleCreateSample = async () => {
    try {
      const parentDir = await open({
        directory: true,
        multiple: false,
        title: "Where should we create the sample project?",
      });
      if (!parentDir || typeof parentDir !== "string") return;

      setIsCreatingSample(true);
      // Unique-ish default name — the backend rejects non-empty dirs,
      // so a timestamp suffix is a cheap way to avoid a second prompt
      // when the user creates multiple samples side-by-side.
      const suffix = new Date().toISOString().slice(0, 10);
      const name = `based-sample-${suffix}`;
      const projectPath = await cmd.createSampleProject(parentDir, name);
      const config = await readProjectConfig(projectPath);
      addRecentProject({
        path: projectPath,
        name: config.name,
        lastOpened: new Date().toISOString(),
      });
      toast.success("Sample project ready");
      const projectId = btoa(projectPath);
      navigate({ to: "/project/$projectId", params: { projectId } });
    } catch (error) {
      toast.error("Failed to create sample project", {
        description: error instanceof Error ? error.message : String(error),
      });
    } finally {
      setIsCreatingSample(false);
    }
  };

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
      } catch {
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
          onClick={handleCreateSample}
          disabled={isCreatingSample}
        >
          <SparklesIcon className="mr-2 size-5" />
          {isCreatingSample ? "Creating..." : "Try Sample Project"}
        </Button>
      </div>

      <Dialog open={showInitDialog} onOpenChange={setShowInitDialog}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Initialize Based Project</DialogTitle>
            <DialogDescription>
              This folder doesn't contain a Based project. Would you like to
              initialize one?
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
