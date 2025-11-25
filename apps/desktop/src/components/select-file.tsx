import { open } from '@tauri-apps/plugin-dialog'
import { XIcon } from "lucide-react"
import { useState } from "react"
import { Button } from "./ui/button"
import { Input } from "./ui/input"

export function SelectFile(
  { defaultValue }: { defaultValue?: string }
) {
  const [filePath, setFilePath] = useState<string | null>(defaultValue ?? null)

  if (filePath) {
    return (
      <div className="relative">
        <Button
          variant="ghost"
          size="icon"
          onClick={() => setFilePath(null)}
          className="absolute right-1 top-1.5 size-6"
        >
          <XIcon />
        </Button>
        <Input readOnly name="filePath" value={filePath} className="pr-8" />
      </div>
    )
  }

  return (
    <Button
      onClick={async () => {
        try {
          const path = await open({
            title: 'Select a SQLite file',
            filters: [
              {
                name: 'SQLite files',
                extensions: ['db', 'sqlite', 'sqlite3'],
              },
            ],
            multiple: false,
            directory: false,
          })
          
          // Handle null case (when user cancels dialog)
          if (!path) {
            console.log('File selection canceled by user');
            return;
          }
          
          // Ensure we have a string path
          const pathStr = typeof path === 'string' ? path : String(path);
          console.log('Selected file path:', pathStr);
          
          setFilePath(pathStr);
        } catch (error) {
          console.error('Error selecting file:', error);
        }
      }}
    >
      Select File
    </Button>
  )
}