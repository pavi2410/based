import { beforeEach, describe, expect, it } from "vitest";
import {
  $queryHistory,
  clearHistory,
  historyForScope,
  recordHistory,
} from "./query-history-store";

beforeEach(() => {
  $queryHistory.set([]);
});

describe("query-history-store", () => {
  it("records entries in reverse chronological order", () => {
    recordHistory({
      projectPath: "/p",
      connKey: "c",
      query: "SELECT 1",
      ranAt: 1,
      durationMs: 10,
      rowCount: 1,
    });
    recordHistory({
      projectPath: "/p",
      connKey: "c",
      query: "SELECT 2",
      ranAt: 2,
      durationMs: 10,
      rowCount: 1,
    });
    expect($queryHistory.get().map((e) => e.query)).toEqual([
      "SELECT 2",
      "SELECT 1",
    ]);
  });

  it("caps entries at 200", () => {
    for (let i = 0; i < 250; i++) {
      recordHistory({
        projectPath: "/p",
        connKey: "c",
        query: `q${i}`,
        ranAt: i,
        durationMs: 1,
        rowCount: 0,
      });
    }
    expect($queryHistory.get()).toHaveLength(200);
  });

  it("filters by scope", () => {
    recordHistory({
      projectPath: "/p",
      connKey: "c1",
      query: "a",
      ranAt: 1,
      durationMs: 1,
      rowCount: 0,
    });
    recordHistory({
      projectPath: "/p",
      connKey: "c2",
      query: "b",
      ranAt: 2,
      durationMs: 1,
      rowCount: 0,
    });
    const c1 = historyForScope({ projectPath: "/p", connKey: "c1" });
    expect(c1).toHaveLength(1);
    expect(c1[0].query).toBe("a");
  });

  it("clears only the scoped history when a scope is supplied", () => {
    recordHistory({
      projectPath: "/p",
      connKey: "c1",
      query: "a",
      ranAt: 1,
      durationMs: 1,
      rowCount: 0,
    });
    recordHistory({
      projectPath: "/p",
      connKey: "c2",
      query: "b",
      ranAt: 2,
      durationMs: 1,
      rowCount: 0,
    });
    clearHistory({ projectPath: "/p", connKey: "c1" });
    const remaining = $queryHistory.get();
    expect(remaining).toHaveLength(1);
    expect(remaining[0].connKey).toBe("c2");
  });
});
