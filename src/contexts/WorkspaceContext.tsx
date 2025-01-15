import { createContext, useContext, useState, ReactNode } from 'react';

type TableViewTab = {
  type: 'table-view';
  tableName: string;
}

type QueryViewTab = {
  type: 'query-view';
}

export type Tab = {
  id: string;
  name: string;
  descriptor: TableViewTab | QueryViewTab;
}

type WorkspaceContextValue = {
  tabs: Tab[];
  activeTab: Tab | undefined;
  setActiveTabId: (id: Tab['id']) => void;
  addTab: (name: Tab['name'], descriptor: Tab['descriptor']) => void;
  removeTab: (id: Tab['id']) => void;
}

const WorkspaceContext = createContext<WorkspaceContextValue | undefined>(undefined);

export const WorkspaceProvider = ({ children }: { children: ReactNode }) => {
  const [tabs, setTabs] = useState<Tab[]>([]);
  const [activeTabId, setActiveTabId] = useState<Tab['id'] | undefined>(undefined);

  const activeTab = tabs.find((tab) => tab.id === activeTabId);

  const addTab = (name: Tab['name'], descriptor: Tab['descriptor']) => {
    const newTabId = crypto.randomUUID();

    // do not create duplicate table tabs
    if (descriptor.type === 'table-view') {
      const existingTab = tabs.find((tab) => tab.descriptor.type === 'table-view' && tab.descriptor.tableName === descriptor.tableName);
      if (existingTab) {
        console.warn(`Tab for table ${descriptor.tableName} already exists`);
        setActiveTabId(existingTab.id);
        return existingTab.id;
      }
    }

    setTabs((prevTabs) => [...prevTabs, { id: newTabId, name, descriptor }]);
    setActiveTabId(newTabId);
    return newTabId;
  };

  const removeTab = (id: Tab['id']) => {
    if (activeTabId === id) {
      const removedTabIndex = tabs.findIndex((tab) => tab.id === id);

      const prevTab = tabs[removedTabIndex - 1];
      const nextTab = tabs[removedTabIndex + 1];

      if (prevTab) {
        setActiveTabId(prevTab.id);
      } else if (nextTab) {
        setActiveTabId(nextTab.id);
      } else {
        setActiveTabId(undefined);
      }
    }

    setTabs((prevTabs) => prevTabs.filter((tab) => tab.id !== id));
  };

  return (
    <WorkspaceContext.Provider value={{ tabs, activeTab, setActiveTabId, addTab, removeTab }}>
      {children}
    </WorkspaceContext.Provider>
  );
};

export const useWorkspace = () => {
  const context = useContext(WorkspaceContext);
  if (!context) {
    throw new Error('useWorkspace must be used within a WorkspaceProvider');
  }
  return context;
};