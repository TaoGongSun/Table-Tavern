import { describe, expect, it, vi } from "vitest";
import { openImportTable } from "./open-import-table";

function io(result: { entered: boolean; writable: boolean }) {
  return {
    createWorld: vi.fn(async () => "new-id"),
    refreshWorlds: vi.fn(async () => {}),
    enterTable: vi.fn(async () => result),
  };
}

describe("openImportTable", () => {
  it("returns the new id when the table is entered and writable", async () => {
    const calls = io({ entered: true, writable: true });
    expect(await openImportTable(calls, "Card")).toBe("new-id");
    expect(calls.createWorld).toHaveBeenCalledWith("Card");
    expect(calls.enterTable).toHaveBeenCalledWith("new-id");
    expect(calls.refreshWorlds).toHaveBeenCalledTimes(1);
  });

  it("returns null when the table opened read-only or needing repair, and stays there", async () => {
    const calls = io({ entered: true, writable: false });
    expect(await openImportTable(calls, "Card")).toBeNull();
    expect(calls.refreshWorlds).toHaveBeenCalledTimes(1);
  });

  it("returns null and re-reads the list when the table could not be entered", async () => {
    const calls = io({ entered: false, writable: false });
    expect(await openImportTable(calls, "Card")).toBeNull();
    expect(calls.refreshWorlds).toHaveBeenCalledTimes(2);
  });
});
