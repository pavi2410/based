import { Input } from "@/components/ui/input.tsx";
import { Label } from "@/components/ui/label.tsx";
import { SelectFile } from "@/components/select-file";
import { newConnectionMutation } from "@/mutations/new-connection";

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
        if (!filePath) return;
        newConnMutation.mutate({
          dbType: "sqlite",
          filePath,
        });
      }}
    >
      <div className="grid gap-4 py-4">
        <div className="grid grid-cols-4 items-center gap-4">
          <Label htmlFor="filePath" className="text-right text-nowrap">
            File Path
          </Label>
          <Input type="file" name="filePath" className="col-span-3" />
        </div>
      </div>
    </form>
  )
}