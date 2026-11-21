import { describe, expect, it } from "vitest";
import { cardAction, projectNumber, statusColumns } from "../scripts/roadmap-board.js";

const roadmap = { name: "roadmap" };

describe("roadmap card moves", () => {
  it("puts a new roadmap issue in Backlog and a closed one in Done", () => {
    expect(cardAction({ action: "opened", issue: { labels: [roadmap] } })).toEqual({
      kind: "add",
      status: "Backlog",
    });
    expect(cardAction({ action: "labeled", label: roadmap, issue: { labels: [roadmap] } })).toEqual({
      kind: "add",
      status: "Backlog",
    });
    expect(cardAction({ action: "closed", issue: { labels: [roadmap] } })).toEqual({
      kind: "set",
      status: "Done",
    });
    expect(cardAction({ action: "reopened", issue: { labels: [roadmap] } })).toEqual({
      kind: "set",
      status: "Backlog",
    });
  });

  it("leaves other issues alone and never chooses Up next or Now", () => {
    expect(cardAction({ action: "opened", issue: { labels: [{ name: "bug" }] } })).toEqual({ kind: "ignore" });
    expect(cardAction({ action: "labeled", label: { name: "bug" }, issue: { labels: [roadmap] } })).toEqual({
      kind: "ignore",
    });
    expect(cardAction({ action: "closed", issue: { labels: [], pull_request: {} } })).toEqual({ kind: "ignore" });

    for (const action of ["opened", "labeled", "closed", "reopened"]) {
      const plan = cardAction({ action, label: roadmap, issue: { labels: [roadmap] } });
      expect(plan.kind === "ignore" || plan.status === "Backlog" || plan.status === "Done").toBe(true);
    }
  });
});

describe("roadmap project number", () => {
  it("uses project 3 unless another number is set", () => {
    expect(projectNumber(undefined)).toBe(3);
    expect(projectNumber("")).toBe(3);
    expect(projectNumber("3")).toBe(3);
    expect(() => projectNumber("0")).toThrow(/positive integer/);
  });
});

describe("roadmap columns", () => {
  it("renames the default columns and keeps Done", () => {
    const columns = statusColumns([
      { id: "todo", name: "Todo", color: "GRAY", description: "" },
      { id: "progress", name: "In Progress", color: "YELLOW", description: "" },
      { id: "done", name: "Done", color: "PURPLE", description: "" },
    ]);
    expect(columns.map((column) => column.name)).toEqual(["Backlog", "Up next", "Now", "Done"]);
    expect(columns.find((column) => column.name === "Backlog")?.id).toBe("todo");
    expect(columns.find((column) => column.name === "Now")?.id).toBe("progress");
    expect(columns.find((column) => column.name === "Done")?.id).toBe("done");
    expect(columns.find((column) => column.name === "Up next")?.id).toBeUndefined();
  });

  it("keeps a column the owner added", () => {
    const columns = statusColumns([
      { id: "backlog", name: "Backlog", color: "GRAY", description: "Not chosen yet." },
      { id: "next", name: "Up next", color: "YELLOW", description: "Chosen to build." },
      { id: "now", name: "Now", color: "GREEN", description: "Being built." },
      { id: "done", name: "Done", color: "PURPLE", description: "Finished." },
      { id: "extra", name: "Later", color: "BLUE", description: "Waiting." },
    ]);
    expect(columns.map((column) => column.name)).toEqual(["Backlog", "Up next", "Now", "Done", "Later"]);
    expect(columns.find((column) => column.name === "Later")?.id).toBe("extra");
  });
});
