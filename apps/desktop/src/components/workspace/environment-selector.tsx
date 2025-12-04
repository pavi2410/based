import { FlaskConicalIcon, CheckIcon } from "lucide-react";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { ProjectConfig } from "@/types/project";
import { Badge } from "@/components/ui/badge";

interface EnvironmentSelectorProps {
  environments: ProjectConfig["environments"];
  activeEnvironment: string;
  onEnvironmentChange: (env: string) => void;
}

function getEnvironmentBadgeVariant(env: string, defaultEnv: string) {
  if (env === defaultEnv) {
    return "default";
  }
  switch (env) {
    case "production":
    case "prod":
      return "destructive";
    case "staging":
      return "secondary";
    default:
      return "outline";
  }
}

export function EnvironmentSelector({
  environments,
  activeEnvironment,
  onEnvironmentChange,
}: EnvironmentSelectorProps) {
  return (
    <Select value={activeEnvironment} onValueChange={onEnvironmentChange}>
      <SelectTrigger className="w-[180px]">
        <div className="flex items-center gap-2">
          <FlaskConicalIcon className="size-4" />
          <SelectValue>
            <div className="flex items-center gap-2">
              <span className="capitalize">{activeEnvironment}</span>
              {activeEnvironment === environments.default && (
                <Badge variant="outline" className="text-xs">
                  default
                </Badge>
              )}
            </div>
          </SelectValue>
        </div>
      </SelectTrigger>
      <SelectContent>
        {environments.available.map((env) => (
          <SelectItem key={env} value={env}>
            <div className="flex items-center gap-2">
              {activeEnvironment === env && (
                <CheckIcon className="size-3 text-primary" />
              )}
              <span className="capitalize">{env}</span>
              {env === environments.default && (
                <Badge
                  variant={getEnvironmentBadgeVariant(env, environments.default)}
                  className="text-xs"
                >
                  default
                </Badge>
              )}
            </div>
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}
