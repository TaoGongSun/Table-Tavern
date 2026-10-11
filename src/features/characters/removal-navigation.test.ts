// @vitest-environment happy-dom
// 刪角色、隱藏、轉條目後主欄要不要關（world-editor-delete-character-guard）：只關正在顯示被刪那張卡的
// 編輯器，其餘畫面不動；卡確定已不在才發刪除通知給世界設定。測的是 controllers/ 的導覽 hook，
// 但行為屬角色刪除，測試跟著角色功能住（controllers/ 不放測試）。
import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async () => "new-id") }));

import {
  type MainViewState,
  type WorkspaceNavigationController,
  useWorkspaceNavigationController,
} from "../../controllers/useWorkspaceNavigationController";

type RemoveResult = "cancelled" | "deleted" | "failed";

describe("removal navigation", () => {
  let root: Root | null = null;
  let host: HTMLDivElement | null = null;
  let nav: WorkspaceNavigationController | null = null;
  let errors: string[] = [];
  let worldId = "w1";
  let removeResult: () => Promise<RemoveResult>;
  let noteRemoved: (id: string, speaker: string) => Promise<string | null>;

  const characters = {
    player: null,
    refresh: async () => [],
    reloadPlayer: async () => {},
    noteRemoved: (id: string, speaker: string) => noteRemoved(id, speaker),
    remove: () => removeResult(),
    removePlayer: async () => true,
  };

  function Probe() {
    nav = useWorkspaceNavigationController({
      worldId,
      characters,
      onError: (message) => errors.push(message),
    });
    return null;
  }

  async function render() {
    await act(async () => root!.render(createElement(Probe)));
  }

  beforeEach(async () => {
    errors = [];
    worldId = "w1";
    removeResult = async () => "deleted";
    noteRemoved = async () => null;
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    await render();
  });

  afterEach(() => {
    act(() => root?.unmount());
    host?.remove();
  });

  async function show(view: MainViewState) {
    await act(async () => nav!.setMainView(view));
  }

  async function del(id: string) {
    await act(async () => nav!.deleteCharacter(id));
  }

  const views: MainViewState[] = [
    { kind: "world" },
    { kind: "character", id: "A" },
    { kind: "new-character", id: "N" },
    { kind: "player", id: "P" },
    { kind: "scene", n: 2 },
  ];

  it("keeps every view that is not showing the deleted card and notifies", async () => {
    for (const [index, view] of views.entries()) {
      await show(view);
      await del("B");
      expect(nav!.mainView).toEqual(view);
      expect(nav!.castRemoval).toMatchObject({ worldId: "w1", generation: index + 1 });
    }
    // 還沒被取走的 id 累積
    expect(nav!.castRemoval!.ids).toEqual(["B", "B", "B", "B", "B"]);
  });

  it("closes the editor showing the deleted card and clears its guard", async () => {
    await show({ kind: "character", id: "A" });
    nav!.leaveGuard.current = async () => false;
    await del("A");
    expect(nav!.mainView).toBeNull();
    expect(nav!.leaveGuard.current).toBeNull();
    expect(nav!.castRemoval!.ids).toEqual(["A"]);
  });

  it("does nothing when the delete is cancelled or fails", async () => {
    await show({ kind: "character", id: "A" });
    for (const result of ["cancelled", "failed"] as const) {
      removeResult = async () => result;
      await del("A");
      expect(nav!.mainView).toEqual({ kind: "character", id: "A" });
      expect(nav!.castRemoval).toBeNull();
    }
  });

  it("decides on the view current when the delete finishes", async () => {
    await show({ kind: "character", id: "A" });
    let finish: (value: RemoveResult) => void = () => {};
    removeResult = () => new Promise((resolve) => (finish = resolve));
    let pending: Promise<void> = Promise.resolve();
    act(() => {
      pending = nav!.deleteCharacter("A");
    });
    // 確認框等人作答期間換到世界設定
    await show({ kind: "world" });
    await act(async () => {
      finish("deleted");
      await pending;
    });
    expect(nav!.mainView).toEqual({ kind: "world" });
    expect(nav!.castRemoval!.ids).toEqual(["A"]);
  });

  it("closes and notifies even when the cast refresh fails", async () => {
    noteRemoved = async () => {
      throw "名單重讀失敗";
    };
    await show({ kind: "character", id: "A" });
    await del("A");
    expect(nav!.mainView).toBeNull();
    expect(nav!.castRemoval!.ids).toEqual(["A"]);
    expect(errors).toContain("名單重讀失敗");
  });

  it("hiding closes the card editor without a delete notice", async () => {
    await show({ kind: "character", id: "A" });
    await act(async () => nav!.finishArchiveToggled("A"));
    expect(nav!.mainView).toBeNull();
    expect(nav!.castRemoval).toBeNull();
  });

  it("converting closes the card editor and notifies", async () => {
    await show({ kind: "character", id: "A" });
    await act(async () => nav!.finishConverted("A"));
    expect(nav!.mainView).toBeNull();
    expect(nav!.castRemoval!.ids).toEqual(["A"]);
  });

  it("takes only the matching world and generation, and resets ids on another table", async () => {
    await del("B");
    await del("C");
    await act(async () => nav!.takeCastRemoval("w1", 1));
    expect(nav!.castRemoval!.ids).toEqual(["B", "C"]);
    await act(async () => nav!.takeCastRemoval("w2", 2));
    expect(nav!.castRemoval!.ids).toEqual(["B", "C"]);
    await act(async () => nav!.takeCastRemoval("w1", 2));
    expect(nav!.castRemoval).toEqual({ worldId: "w1", generation: 2, ids: [] });
    await del("D");
    worldId = "w2";
    await render();
    await del("E");
    expect(nav!.castRemoval).toEqual({ worldId: "w2", generation: 4, ids: ["E"] });
  });

  // 刪除或善後停在 await 時換桌：回來的續程不得動新桌（Sol 驗收 r1）
  async function switchTo(next: string) {
    worldId = next;
    await render();
  }

  function deferred<T>() {
    let resolve: (value: T) => void = () => {};
    const promise = new Promise<T>((done) => (resolve = done));
    return { promise, resolve };
  }

  for (const route of [["w2"], ["w2", "w1"]]) {
    it(`drops a delete that finishes after switching tables (${route.join("→")})`, async () => {
      await show({ kind: "character", id: "A" });
      await act(async () => nav!.setSpeaker("A"));
      const removal = deferred<RemoveResult>();
      removeResult = () => removal.promise;
      let pending: Promise<void> = Promise.resolve();
      act(() => {
        pending = nav!.deleteCharacter("A");
      });
      for (const next of route) await switchTo(next);
      noteRemoved = async () => "";
      await act(async () => {
        removal.resolve("deleted");
        await pending;
      });
      expect(nav!.mainView).toEqual({ kind: "character", id: "A" });
      expect(nav!.speaker).toBe("A");
      expect(nav!.castRemoval).toBeNull();
    });

    it(`drops a cleanup that finishes after switching tables (${route.join("→")})`, async () => {
      await act(async () => nav!.setSpeaker("A"));
      const refresh = deferred<string | null>();
      noteRemoved = () => refresh.promise;
      let pending: Promise<void> = Promise.resolve();
      act(() => {
        pending = nav!.deleteCharacter("A");
      });
      await act(async () => {});
      for (const next of route) await switchTo(next);
      // 新桌自己已有待取通知
      noteRemoved = async () => null;
      await del("Z");
      const before = nav!.castRemoval;
      await act(async () => {
        refresh.resolve("");
        await pending;
      });
      expect(nav!.castRemoval).toEqual(before);
      expect(nav!.speaker).toBe("A");
    });
  }
});
