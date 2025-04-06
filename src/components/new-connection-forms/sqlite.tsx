import { Label } from "@/components/ui/label.tsx";
import { SelectFile } from "@/components/select-file";
import { newConnectionMutation } from "@/mutations/new-connection";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { AlertCircle } from "lucide-react";

export function SQLiteConnectionForm() {
  const newConnMutation = newConnectionMutation();

  return (
    <form
      id="new-connection-form"
      onSubmit={(e) => {
        e.preventDefault();
        e.stopPropagation();
        
        const formData = new FormData(e.currentTarget);
        const filePath = formData.get("filePath") as string;
        console.log('Form data:', { filePath });
        
        if (!filePath) {
          console.log('No file path provided');
          return;
        }
        
        console.log('Starting mutation with:', { filePath });
        newConnMutation.mutate({
          dbType: "sqlite",
          filePath,
          tags: [],
        });
      }}
    >
      <div className="grid gap-4 py-4">
        <div className="grid grid-cols-4 items-center gap-4">
          <Label htmlFor="filePath" className="text-right text-nowrap">
            File Path
          </Label>
          <div className="col-span-3">
            <SelectFile />
          </div>
        </div>
        {newConnMutation.isError && (
          <Alert variant="destructive">
            <AlertCircle className="h-4 w-4" />
            <AlertDescription>
              {newConnMutation.error.message}
            </AlertDescription>
          </Alert>
        )}
      </div>
    </form>
  );
}