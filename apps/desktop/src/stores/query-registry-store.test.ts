import { beforeEach, describe, expect, it } from "vitest";
import {
  $runningQueries,
  markQueryEnd,
  markQueryStart,
} from "./query-registry-store";

beforeEach(() => {
  $runningQueries.set([]);
});

describe("query-registry-store", () => {
  it("tracks a running token", () => {
    markQueryStart("t1");
    expect($runningQueries.get()).toEqual(["t1"]);
  });

  it("ignores duplicate starts for the same token", () => {
    markQueryStart("t1");
    markQueryStart("t1");
    expect($runningQueries.get()).toEqual(["t1"]);
  });

  it("removes a token on end", () => {
    markQueryStart("t1");
    markQueryStart("t2");
    markQueryEnd("t1");
    expect($runningQueries.get()).toEqual(["t2"]);
  });

  it("is a no-op for unknown tokens", () => {
    markQueryStart("t1");
    markQueryEnd("t99");
    expect($runningQueries.get()).toEqual(["t1"]);
  });
});
