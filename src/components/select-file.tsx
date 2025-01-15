import { open } from '@tauri-apps/plugin-dialog'
import { XIcon } from "lucide-react"
import { useState } from "react"
import { Button } from "./ui/button"
import { Input } from "./ui/input"

export function SelectFile() {
  const [filePath, setFilePath] = useState<string | null>(null)

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
        setFilePath(path)
      }}
    >
      Select File
    </Button>
  )
}