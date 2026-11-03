// VN 作品包的前端流程（plans/vn-mode-pkg8.md §2.2–§2.3）：確認框內文、匯入只走路徑與模式、取消不建桌、匯出帶附設定。
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { VnWorkProbe } from "../../../shared/contracts/backend-contracts";

const backend = vi.hoisted(() => ({ calls: [] as [string, unknown][], confirm: true }));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (command: string, args: unknown) => {
    backend.calls.push([command, args]);
    if (command === "probe_vn_work") return probe;
    if (command === "import_vn_work") return { world_id: "W2", notices: [{ kind: "clipped", target: "scene 2" }] };
    if (command === "vn_bible_read") return { cover: true };
    if (command === "export_vn_work") return { notices: [] };
    return null;
  }),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({
  confirm: vi.fn(async () => backend.confirm),
  message: vi.fn(async () => {}),
  save: vi.fn(async () => "/out/霧港.ttvn"),
  open: vi.fn(async () => null),
}));
vi.mock("@tauri-apps/plugin-opener", () => ({ revealItemInDir: vi.fn(async () => {}) }));

import { message } from "@tauri-apps/plugin-dialog";
import {
  canContinueWork,
  confirmVnWork,
  exportVnWork,
  importVnWork,
  isWorkPath,
  probeVnWork,
  vnWorkConfirmText,
} from "./vn-work";

const probe: VnWorkProbe = {
  title: "霧港",
  lang: "zh-TW",
  chapters: 3,
  endings: 2,
  settings: false,
  vars_mode: false,
  rating: { level: "teen", warnings: ["死亡"] },
};

describe("vn-work", () => {
  beforeEach(() => {
    backend.calls.length = 0;
    backend.confirm = true;
  });

  it("recognises packs by extension only", () => {
    expect(isWorkPath("/a/B.TTVN")).toBe(true);
    expect(isWorkPath("/a/card.png")).toBe(false);
  });

  it("puts title, counts and rating in the confirmation", () => {
    const text = vnWorkConfirmText(probe);
    for (const part of ["霧港", "3", "2", "死亡"]) expect(text).toContain(part);
    expect(vnWorkConfirmText({ ...probe, rating: null })).not.toContain("死亡");
  });

  it("imports by path and mode and shows the notices", async () => {
    expect(await probeVnWork("/in/x.ttvn")).toEqual(probe);
    expect(await importVnWork("/in/x.ttvn", "continue", true)).toBe("W2");
    expect(backend.calls).toEqual([
      ["probe_vn_work", { path: "/in/x.ttvn" }],
      ["import_vn_work", { path: "/in/x.ttvn", mode: "continue", spoilerLock: true }],
    ]);
    expect(message).toHaveBeenCalled();
  });

  it("offers continuing only for packs with settings from a non-vars table", () => {
    expect(canContinueWork(probe)).toBe(false);
    expect(canContinueWork({ ...probe, settings: true })).toBe(true);
    expect(canContinueWork({ ...probe, settings: true, vars_mode: true })).toBe(false);
    expect(vnWorkConfirmText(probe, true)).not.toBe(vnWorkConfirmText(probe));
  });

  it("passes the player's answer through the confirmation", async () => {
    backend.confirm = false;
    expect(await confirmVnWork(probe)).toBe(false);
  });

  it("exports to the chosen path with or without settings", async () => {
    expect(await exportVnWork("W", "霧港", { settings: true, cover: false })).toBe(true);
    expect(backend.calls).toContainEqual([
      "export_vn_work",
      { worldId: "W", path: "/out/霧港.ttvn", cover: null, settings: true },
    ]);
  });
});
