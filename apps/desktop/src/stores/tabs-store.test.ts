import { beforeEach, describe, expect, it } from "vitest";
import {
  $activeByScope,
  $tabs,
  closeTab,
  savedQueryTabId,
  setActiveTab,
  type TabScope,
  tableTabId,
  upsertTab,
} from "./tabs-store";

const scope: TabScope = { projectPath: "/p", connKey: "conn" };

beforeEach(() => {
  $tabs.set([]);
  $activeByScope.set({});
});

describe("tabs-store id builders", () => {
  it("tableTabId is stable for the same schema+name", () => {
    expect(tableTabId("public", "users")).toBe(tableTabId("public", "users"));
    expect(tableTabId(null, "t")).not.toBe(tableTabId("public", "t"));
  });

  it("savedQueryTabId is distinct from table ids", () => {
    expect(savedQueryTabId("q.sql")).not.toBe(tableTabId(null, "q.sql"));
  });
});

describe("upsertTab", () => {
  it("adds a new tab", () => {
    upsertTab({
      id: tableTabId("public", "users"),
      kind: "table",
      title: "users",
      scope,
    });
    expect($tabs.get()).toHaveLength(1);
  });

  it("merges metadata onto an existing tab rather than duplicating", () => {
    upsertTab({
      id: "query:file.sql",
      kind: "query",
      title: "file.sql",
      scope,
    });
    upsertTab({
      id: "query:file.sql",
      kind: "query",
      title: "file.sql (modified)",
      scope,
      queryFilename: "file.sql",
    });
    const tabs = $tabs.get();
    expect(tabs).toHaveLength(1);
    expect(tabs[0].title).toBe("file.sql (modified)");
    expect(tabs[0].queryFilename).toBe("file.sql");
  });

  it("treats tabs in different scopes as independent", () => {
    const other: TabScope = { projectPath: "/p", connKey: "other" };
    upsertTab({ id: "t", kind: "query", title: "t", scope });
    upsertTab({ id: "t", kind: "query", title: "t", scope: other });
    expect($tabs.get()).toHaveLength(2);
  });
});

describe("closeTab", () => {
  it("returns the next tab id and clears when the last tab is closed", () => {
    upsertTab({ id: "a", kind: "query", title: "a", scope });
    upsertTab({ id: "b", kind: "query", title: "b", scope });
    upsertTab({ id: "c", kind: "query", title: "c", scope });
    setActiveTab(scope, "b");

    expect(closeTab(scope, "b")).toBe("c");
    expect(closeTab(scope, "c")).toBe("a");
    expect(closeTab(scope, "a")).toBe(null);
    expect($tabs.get()).toHaveLength(0);
  });

  it("is a no-op for unknown tab ids", () => {
    upsertTab({ id: "a", kind: "query", title: "a", scope });
    setActiveTab(scope, "a");
    expect(closeTab(scope, "does-not-exist")).toBe("a");
    expect($tabs.get()).toHaveLength(1);
  });

  it("only clears active if the closed tab was active", () => {
    upsertTab({ id: "a", kind: "query", title: "a", scope });
    upsertTab({ id: "b", kind: "query", title: "b", scope });
    setActiveTab(scope, "a");
    closeTab(scope, "b");
    expect($activeByScope.get()[`${scope.projectPath}::${scope.connKey}`]).toBe(
      "a",
    );
  });
});
