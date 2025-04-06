import { store, STORE_KEYS } from './store-config';

export type QueryHistoryItem = {
  id: string;
  connectionId: string;
  query: string;
  timestamp: number;
  isStarred: boolean;
  tags?: string[];
  executionTime?: number;
  resultsCount?: number;
}

export type QueryHistoryFilter = {
  connectionId?: string;
  search?: string;
  isStarred?: boolean;
  tags?: string[];
  startDate?: number;
  endDate?: number;
  limit?: number;
  offset?: number;
}

// Query History Functions
export async function getQueryHistory(filterOrConnectionId?: QueryHistoryFilter | string) {
  const history = (await store.get<QueryHistoryItem[]>(STORE_KEYS.QUERY_HISTORY)) ?? [];
  
  // Handle string connectionId (for backward compatibility)
  if (typeof filterOrConnectionId === 'string') {
    return history.filter(item => item.connectionId === filterOrConnectionId);
  }
  
  // If no filter, return all
  if (!filterOrConnectionId) {
    return history;
  }
  
  const filter = filterOrConnectionId;
  
  // Apply filters
  let filtered = history;
  
  if (filter.connectionId) {
    filtered = filtered.filter(item => item.connectionId === filter.connectionId);
  }
  
  if (filter.search) {
    const searchLower = filter.search.toLowerCase();
    filtered = filtered.filter(item => 
      item.query.toLowerCase().includes(searchLower)
    );
  }
  
  if (filter.isStarred !== undefined) {
    filtered = filtered.filter(item => item.isStarred === filter.isStarred);
  }
  
  if (filter.tags && filter.tags.length > 0) {
    filtered = filtered.filter(item => 
      item.tags && filter.tags?.some(tag => item.tags?.includes(tag))
    );
  }
  
  if (filter.startDate) {
    filtered = filtered.filter(item => item.timestamp >= filter.startDate!);
  }
  
  if (filter.endDate) {
    filtered = filtered.filter(item => item.timestamp <= filter.endDate!);
  }
  
  // Apply pagination if specified
  if (filter.limit !== undefined) {
    const offset = filter.offset || 0;
    filtered = filtered.slice(offset, offset + filter.limit);
  }
  
  return filtered;
}

export async function addQueryToHistory(connectionId: string, query: string, metadata?: {
  executionTime?: number;
  resultsCount?: number;
  tags?: string[];
}) {
  const history = await getQueryHistory();
  const newHistoryItem: QueryHistoryItem = {
    id: crypto.randomUUID(),
    connectionId,
    query,
    timestamp: Date.now(),
    isStarred: false,
    ...(metadata || {}),
  };
  
  await store.set(STORE_KEYS.QUERY_HISTORY, [newHistoryItem, ...history]);
  return newHistoryItem;
}

export async function toggleQueryStar(queryId: string) {
  const history = await getQueryHistory();
  const queryIndex = history.findIndex(item => item.id === queryId);
  
  if (queryIndex === -1) {
    throw new Error('Query not found in history');
  }
  
  history[queryIndex].isStarred = !history[queryIndex].isStarred;
  await store.set(STORE_KEYS.QUERY_HISTORY, history);
  return history[queryIndex];
}

export async function updateQueryTags(queryId: string, tags: string[]) {
  const history = await getQueryHistory();
  const queryIndex = history.findIndex(item => item.id === queryId);
  
  if (queryIndex === -1) {
    throw new Error('Query not found in history');
  }
  
  history[queryIndex].tags = tags;
  await store.set(STORE_KEYS.QUERY_HISTORY, history);
  return history[queryIndex];
}

export async function deleteQuery(queryId: string) {
  const history = await getQueryHistory();
  await store.set(
    STORE_KEYS.QUERY_HISTORY,
    history.filter(item => item.id !== queryId)
  );
}

export async function clearQueryHistory(connectionId?: string) {
  if (connectionId) {
    const history = await getQueryHistory();
    await store.set(
      STORE_KEYS.QUERY_HISTORY,
      history.filter(item => item.connectionId !== connectionId)
    );
  } else {
    await store.set(STORE_KEYS.QUERY_HISTORY, []);
  }
}

export async function getAllTags() {
  const history = await getQueryHistory();
  const tags = new Set<string>();
  
  history.forEach(item => {
    if (item.tags) {
      item.tags.forEach(tag => tags.add(tag));
    }
  });
  
  return Array.from(tags);
} 