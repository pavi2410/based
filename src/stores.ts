import { LazyStore } from '@tauri-apps/plugin-store';

const store = new LazyStore('settings.json');

export type DbConnectionMeta = {
  id: string;
  createdAt: number;
  updatedAt: number;
  dbType: 'sqlite';
  filePath: string;
}

export type ProjectMeta = {
  id: string;
  createdAt: number;
  updatedAt: number;
  name: string;
  connections: DbConnectionMeta[];
}

type CoreFields = 'id' | 'createdAt' | 'updatedAt';

export async function getProjects() {
  return (await store.get<ProjectMeta[]>('projects')) ?? [];
}

export async function addProject(project: Omit<ProjectMeta, CoreFields>) {
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

export async function getProject(projectId: string) {
  const projects = await getProjects();
  return projects.find(project => project.id === projectId);
}

async function updateProject(projectId: string, project: ProjectMeta) {
  const projects = await getProjects();
  const projectIndex = projects.findIndex(project => project.id === projectId);
  if (projectIndex === -1) {
    throw new Error('Project not found');
  }
  project.updatedAt = Date.now();
  projects[projectIndex] = project;
  await store.set('projects', projects);
}

export async function getConnections(projectId: string) {
  const project = await getProject(projectId)
  if (!project) {
    throw new Error('Project not found');
  }
  return project.connections;
}

export async function getConnection(projectId: string, connectionId: string) {
  const connections = await getConnections(projectId);
  return connections.find(connection => connection.id === connectionId);
}

export async function addConnection(projectId: string, connection: Omit<DbConnectionMeta, CoreFields>) {
  const project = await getProject(projectId)
  if (!project) {
    throw new Error('Project not found');
  }
  const newConnectionId = crypto.randomUUID();
  project.connections.push({
    ...connection,
    id: newConnectionId,
    createdAt: Date.now(),
    updatedAt: Date.now(),
  });
  await updateProject(projectId, project);
  return newConnectionId;
}

export async function removeConnection(projectId: string, connectionId: string) {
  const project = await getProject(projectId)
  if (!project) {
    throw new Error('Project not found');
  }
  project.connections = project.connections.filter(connection => connection.id !== connectionId);
  await updateProject(projectId, project);
}