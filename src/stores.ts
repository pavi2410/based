import { LazyStore } from '@tauri-apps/plugin-store';

const store = new LazyStore('settings.json');

export type Project = {
  dbType: 'sqlite';
  filePath: string;
}

export async function getProjects() {
  return (await store.get<Project[]>('projects')) ?? [];
}

export async function addProject(project: Project) {
  const projects = await getProjects();
  if (projects.filter(p => p.dbType === project.dbType).some(p => p.filePath === project.filePath)) {
    return;
  }
  projects.push(project);
  await store.set('projects', projects);
}

export async function removeProject(project: Project) {
  const projects = await getProjects();
  const index = projects.findIndex(p => p.dbType === project.dbType && p.filePath === project.filePath);
  if (index === -1) {
    return;
  }
  projects.splice(index, 1);
  await store.set('projects', projects);
}