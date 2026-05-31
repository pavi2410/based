export type InstallOs = "mac" | "linux" | "windows";

export interface InstallDownload {
  label: string;
  url: string;
  digest?: string;
  downloads: number;
}

export interface InstallPackageManager {
  label: string;
  command: string;
}

export interface InstallPlatform {
  os: InstallOs;
  name: string;
  detail: string;
  primary: InstallDownload;
  secondary?: InstallDownload | null;
  packageManager?: InstallPackageManager;
}
