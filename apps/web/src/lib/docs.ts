export type DocGroup = "Start" | "Workspace" | "App" | "Project";

export interface DocPage {
  href: string;
  title: string;
  group: DocGroup;
  description: string;
}

export const DOC_PAGES: DocPage[] = [
  {
    href: "/docs",
    title: "Overview",
    group: "Start",
    description:
      "How based is organized: project files, the workspace, and where each setting lives.",
  },
  {
    href: "/docs/databases",
    title: "Databases",
    group: "Workspace",
    description: "PostgreSQL, SQLite, and MongoDB — what each engine can do in the app today.",
  },
  {
    href: "/docs/connections",
    title: "Connections",
    group: "Workspace",
    description:
      "Project connection files, the connection tree, wizards, secrets, and connect lifecycle.",
  },
  {
    href: "/docs/editor",
    title: "Editor",
    group: "Workspace",
    description:
      "SQL and aggregation editors, run, results, explain, variables, and saved queries.",
  },
  {
    href: "/docs/table-grid",
    title: "Table grid",
    group: "Workspace",
    description: "Data viewers, pagination, filters, sorting, export, and grid preferences.",
  },
  {
    href: "/docs/ui",
    title: "UI",
    group: "App",
    description: "Title bar, sidebar, center tabs, side panes, Home, command palette, and theme.",
  },
  {
    href: "/docs/navigation",
    title: "Navigation",
    group: "App",
    description:
      "Tabs, shortcuts, catalog, saved queries, history, favorites, and pop-out windows.",
  },
  {
    href: "/docs/settings",
    title: "Settings",
    group: "App",
    description: "The settings window: appearance, updates, query defaults, and table interaction.",
  },
  {
    href: "/docs/configuration",
    title: "Configuration",
    group: "Project",
    description:
      "The .based/ folder, project.toml, vars, secrets, local state, and native preferences.",
  },
];

export const DOC_GROUPS: DocGroup[] = ["Start", "Workspace", "App", "Project"];

export function normalizeDocPath(pathname: string): string {
  if (pathname.length > 1 && pathname.endsWith("/")) {
    return pathname.slice(0, -1);
  }
  return pathname;
}

export function docPageByPath(pathname: string): DocPage | undefined {
  const path = normalizeDocPath(pathname);
  return DOC_PAGES.find((page) => page.href === path);
}

export function docPager(pathname: string): { prev: DocPage | null; next: DocPage | null } {
  const path = normalizeDocPath(pathname);
  const index = DOC_PAGES.findIndex((page) => page.href === path);
  if (index < 0) {
    return { prev: null, next: null };
  }
  return {
    prev: index > 0 ? DOC_PAGES[index - 1] : null,
    next: index < DOC_PAGES.length - 1 ? DOC_PAGES[index + 1] : null,
  };
}
