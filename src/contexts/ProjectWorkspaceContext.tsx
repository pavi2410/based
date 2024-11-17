import {createContext, useContext, useState, ReactNode} from 'react';

type TableViewTab = {
  type: 'table-view';
  connectionId: string;
  tableName: string;
}

type QueryViewTab = {
  type: 'query-view';
  connectionId: string;
}

export type Tab = {
  id: string;
  name: string;
  descriptor: TableViewTab | QueryViewTab;
}

type ProjectWorkspaceContextType = {
  tabs: Tab[];
  addTab: (name: string, descriptor: Tab['descriptor']) => void;
  removeTab: (id: string) => void;
}

const ProjectWorkspaceContext = createContext<ProjectWorkspaceContextType | undefined>(undefined);

export const ProjectWorkspaceProvider = ({children}: { children: ReactNode }) => {
  const [tabs, setTabs] = useState<Tab[]>([]);

  const addTab = (name: string, descriptor: Tab['descriptor']) => {
    const newTabId = crypto.randomUUID();
    setTabs((prevTabs) => [...prevTabs, {id: newTabId, name, descriptor}]);
    return newTabId;
  };

  const removeTab = (id: string) => {
    setTabs((prevTabs) => prevTabs.filter((tab) => tab.id !== id));
  };

  return (
    <ProjectWorkspaceContext.Provider value={{tabs, addTab, removeTab}}>
      {children}
    </ProjectWorkspaceContext.Provider>
  );
};

export const useProjectWorkspace = () => {
  const context = useContext(ProjectWorkspaceContext);
  if (!context) {
    throw new Error('useProjectWorkspace must be used within a ProjectWorkspaceProvider');
  }
  return context;
};