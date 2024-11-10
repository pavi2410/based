import { LazyStore } from '@tauri-apps/plugin-store';
const store = new LazyStore('settings.json');

export type ProjectMeta = {
  id: string;
  createdAt: number;
  updatedAt: number;
  name: string;
  connections: Array<{
    dbType: 'sqlite';
    filePath: string;
  }>,
}

export async function getProjects() {
  return (await store.get<ProjectMeta[]>('projects')) ?? [];
}

export async function addProject(project: Omit<ProjectMeta, 'id' | 'createdAt' | 'updatedAt'>) {
  const newProjectId = crypto.randomUUID();
  const projects = await getProjects();
  projects.push({
    ...project,
    id: newProjectId,
    createdAt: Date.now(),
    updatedAt: Date.now(),
  });
  await store.set('projects', projects);
  return newProjectId;
}

export async function removeProject(projectId: string) {
  const projects = await getProjects();
  await store.set('projects', projects.filter(project => project.id !== projectId));
}