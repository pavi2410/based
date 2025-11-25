import { useState } from "react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { type SqliteConnectionMeta } from "@/stores/db-connections";
import { Label } from "@/components/ui/label";
import { SelectFile } from "@/components/select-file";
import { useEditConnectionMutation } from "@/mutations/edit-connection";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { AlertCircle } from "lucide-react";

interface EditSQLiteConnectionDialogProps {
  connection: SqliteConnectionMeta;
  trigger: React.ReactNode;
}

export function EditSQLiteConnectionDialog({ 
  connection,
  trigger
}: EditSQLiteConnectionDialogProps) {
  const [open, setOpen] = useState(false);
  const editMutation = useEditConnectionMutation();

  const handleSubmit = (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    const formData = new FormData(e.currentTarget);
    const filePath = formData.get("filePath") as string;
    
    if (!filePath) return;
    
    editMutation.mutate({
      connectionId: connection.id,
      dbType: 'sqlite',
      filePath,
      tags: [],
    }, {
      onSuccess: () => {
        setOpen(false);
      }
    });
  };

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      {trigger}
      <DialogContent className="sm:max-w-[500px]">
        <DialogHeader>
          <DialogTitle>Edit SQLite Connection</DialogTitle>
          <DialogDescription>
            Update the file path for this SQLite database connection.
          </DialogDescription>
        </DialogHeader>
        
        <form id="edit-sqlite-form" onSubmit={handleSubmit}>
          <div className="grid gap-4 py-4">
            <div className="grid grid-cols-4 items-center gap-4">
              <Label htmlFor="filePath" className="text-right text-nowrap">
                Database File
              </Label>
              <div className="col-span-3">
                <SelectFile defaultValue={connection.filePath} />
              </div>
            </div>
            
            {editMutation.isError && (
              <Alert variant="destructive">
                <AlertCircle className="h-4 w-4" />
                <AlertDescription>
                  {editMutation.error.message}
                </AlertDescription>
              </Alert>
            )}
          </div>
        </form>
        
        <DialogFooter>
          <Button 
            type="submit" 
            form="edit-sqlite-form"
            disabled={editMutation.isPending}
          >
            {editMutation.isPending ? "Saving..." : "Save Changes"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
} 