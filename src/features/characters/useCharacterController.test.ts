// @vitest-environment happy-dom
// 刪角色後世界書清理沒做完：仍當成刪除成功，另外提示一行（character-delete-visibility-cleanup）。
import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { t } from "../../i18n";

const backend = vi.hoisted(() => ({
  handlers: {} as Record<string, (args: Record<string, unknown>) => unknown>,
  calls: [] as string[],
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (command: string, args: Record<string, unknown> = {}) => {
    backend.calls.push(command);
    const handler = backend.handlers[command];
    if (handler) return handler(args);
    return command.startsWith("list_") ? [] : undefined;
  }),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ confirm: vi.fn(async () => true) }));

import { useCharacterController, type CharacterController } from "./useCharacterController";

describe("useCharacterController delete cleanup notice", () => {
  let root: Root | null = null;
  let host: HTMLDivElement | null = null;
  let controller: CharacterController | null = null;
  let errors: string[] = [];

  function Probe() {
    controller = useCharacterController({ worldId: "w1", onError: (m) => errors.push(m) });
    return null;
  }

  beforeEach(async () => {
    backend.handlers = {};
    backend.calls = [];
    errors = [];
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    await act(async () => root!.render(createElement(Probe)));
  });

  afterEach(() => {
    act(() => root?.unmount());
    host?.remove();
  });

  const lastError = () => errors[errors.length - 1] ?? "";

  it("deletes and then shows the notice when the cleanup did not finish", async () => {
    backend.handlers.delete_character = () => ({ worldbook_cleanup_failed: true });
    let removed = "";
    await act(async () => {
      removed = await controller!.remove("c1");
    });
    expect(removed).toBe("deleted");
    expect(lastError()).toBe(t("deleteCharacterCleanupFailed"));
  });

  it("stays quiet when the cleanup finished or the backend returns nothing", async () => {
    for (const result of [{ worldbook_cleanup_failed: false }, undefined]) {
      backend.handlers.delete_character = () => result;
      let removed = "";
      await act(async () => {
        removed = await controller!.remove("c1");
      });
      expect(removed).toBe("deleted");
      expect(lastError()).toBe("");
    }
  });

  it("keeps the notice after the player card cleanup", async () => {
    backend.handlers.delete_character = () => ({ worldbook_cleanup_failed: true });
    let removed = false;
    await act(async () => {
      removed = await controller!.removePlayer("p1");
    });
    expect(removed).toBe(true);
    expect(backend.calls.indexOf("set_player_card")).toBeGreaterThan(
      backend.calls.indexOf("delete_character"),
    );
    expect(lastError()).toBe(t("deleteCharacterCleanupFailed"));
  });

  // 刪檔回錯：問後端卡檔是否確定已不在（world-editor-delete-character-guard）
  async function removeAfterFailure(gone: (() => unknown) | undefined) {
    backend.handlers.delete_character = () => {
      throw "磁碟錯誤";
    };
    if (gone) backend.handlers.character_card_gone = gone;
    let removed = "";
    await act(async () => {
      removed = await controller!.remove("c1");
    });
    return removed;
  }

  it("counts a failed delete as deleted when the card file is confirmed gone", async () => {
    expect(await removeAfterFailure(() => true)).toBe("deleted");
    expect(lastError()).toBe(t("deleteCharacterPartial", { reason: "磁碟錯誤" }));
  });

  it("keeps a failed delete as failed when the card is still there or the check fails", async () => {
    expect(await removeAfterFailure(() => false)).toBe("failed");
    expect(lastError()).toBe("磁碟錯誤");
    expect(
      await removeAfterFailure(() => {
        throw "查不到";
      }),
    ).toBe("failed");
    expect(lastError()).toBe("磁碟錯誤");
  });

  it("reports a cancelled confirm without touching the backend", async () => {
    const { confirm } = await import("@tauri-apps/plugin-dialog");
    vi.mocked(confirm).mockResolvedValueOnce(false);
    let removed = "";
    await act(async () => {
      removed = await controller!.remove("c1");
    });
    expect(removed).toBe("cancelled");
    expect(backend.calls).not.toContain("delete_character");
  });
});

// 刪除、還原、排序的 await 回來時已換桌：錯誤與名單寫入不落到新桌（undo-import-editor-guard）
describe("useCharacterController after switching tables mid-call", () => {
  let root: Root | null = null;
  let host: HTMLDivElement | null = null;
  let controller: CharacterController | null = null;
  let errors: string[] = [];
  let worldId = "w1";

  function Probe() {
    controller = useCharacterController({ worldId, onError: (m) => errors.push(m) });
    return null;
  }

  const render = () => act(async () => root!.render(createElement(Probe)));

  function deferred<T>() {
    let resolve: (value: T) => void = () => {};
    let reject: (reason: unknown) => void = () => {};
    const promise = new Promise<T>((done, fail) => {
      resolve = done;
      reject = fail;
    });
    return { promise, resolve, reject };
  }

  const meta = (id: string) => ({
    id,
    name: id,
    color: "#e07a5f",
    avatar: "🎭",
    tier: "balanced" as const,
    show_image: true,
    archived: false,
    auto_hidden: false,
  });

  beforeEach(async () => {
    backend.handlers = {};
    backend.calls = [];
    errors = [];
    worldId = "w1";
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    await render();
  });

  afterEach(() => {
    act(() => root?.unmount());
    host?.remove();
  });

  for (const route of [["w2"], ["w2", "w1"]]) {
    const label = route.join("→");

    async function switchAway() {
      for (const next of route) {
        worldId = next;
        await render();
      }
    }

    it(`drops a failed delete's error (${label})`, async () => {
      const call = deferred<unknown>();
      backend.handlers.delete_character = () => call.promise;
      backend.handlers.character_card_gone = () => false;
      let pending: Promise<unknown> = Promise.resolve();
      act(() => {
        pending = controller!.remove("c1");
      });
      await act(async () => {});
      await switchAway();
      await act(async () => {
        call.reject("刪不掉");
        await pending;
      });
      expect(errors.filter(Boolean)).toEqual([]);
    });

    it(`keeps the new table's player card when removePlayer returns late (${label})`, async () => {
      const call = deferred<unknown>();
      backend.handlers.delete_character = () => call.promise;
      let removed: boolean | null = null;
      let pending: Promise<void> = Promise.resolve();
      act(() => {
        pending = controller!.removePlayer("p1").then((value) => {
          removed = value;
        });
      });
      await act(async () => {});
      await switchAway();
      await act(async () => {
        call.resolve({ worldbook_cleanup_failed: true });
        await pending;
      });
      expect(removed).toBe(false);
      expect(errors.filter(Boolean)).toEqual([]);
    });

    it(`does not reread or report after a late restore (${label})`, async () => {
      const call = deferred<unknown>();
      backend.handlers.set_character_archived = () => call.promise;
      let pending: Promise<void> = Promise.resolve();
      act(() => {
        pending = controller!.restore("c1");
      });
      await switchAway();
      backend.calls = [];
      await act(async () => {
        call.resolve(undefined);
        await pending;
      });
      expect(backend.calls).not.toContain("list_characters");
    });

    it(`does not roll a late failed reorder back onto the new table (${label})`, async () => {
      await act(async () => controller!.hydrate([meta("A"), meta("B")], new Set(), null));
      const call = deferred<unknown>();
      backend.handlers.reorder_characters = () => call.promise;
      let pending: Promise<void> = Promise.resolve();
      act(() => {
        pending = controller!.reorder([meta("B"), meta("A")]);
      });
      await switchAway();
      await act(async () => controller!.hydrate([meta("X")], new Set(), null));
      await act(async () => {
        call.reject("寫不進去");
        await pending;
      });
      expect(controller!.list.map((character) => character.id)).toEqual(["X"]);
      expect(errors.filter(Boolean)).toEqual([]);
    });
  }

  it("does not let an earlier failed reorder undo a later one", async () => {
    await act(async () => controller!.hydrate([meta("A"), meta("B"), meta("C")], new Set(), null));
    const first = deferred<unknown>();
    backend.handlers.reorder_characters = () => first.promise;
    let pending: Promise<void> = Promise.resolve();
    act(() => {
      pending = controller!.reorder([meta("B"), meta("A"), meta("C")]);
    });
    backend.handlers.reorder_characters = () => undefined;
    await act(async () => controller!.reorder([meta("C"), meta("B"), meta("A")]));
    await act(async () => {
      first.reject("寫不進去");
      await pending;
    });
    expect(controller!.list.map((character) => character.id)).toEqual(["C", "B", "A"]);
    expect(errors.filter(Boolean)).toEqual([]);
  });

  it("a reorder does not void a cast reread already on the way", async () => {
    await act(async () => controller!.hydrate([meta("A"), meta("B")], new Set(), null));
    const reread = deferred<unknown>();
    backend.handlers.list_characters = () => reread.promise;
    let refreshing: Promise<unknown> = Promise.resolve();
    act(() => {
      refreshing = controller!.refresh();
    });
    backend.handlers.reorder_characters = () => undefined;
    await act(async () => controller!.reorder([meta("B"), meta("A")]));
    await act(async () => {
      reread.resolve([meta("B"), meta("A"), meta("C")]);
      await refreshing;
    });
    expect(controller!.list.map((character) => character.id)).toEqual(["B", "A", "C"]);
  });

  it("a failed reorder rereads the cast from disk instead of restoring a stale snapshot", async () => {
    await act(async () => controller!.hydrate([meta("A"), meta("B")], new Set(), null));
    // 重讀已送出 → 排序送出 → 重讀完成加入 C → 排序失敗
    const reread = deferred<unknown>();
    backend.handlers.list_characters = () => reread.promise;
    let refreshing: Promise<unknown> = Promise.resolve();
    act(() => {
      refreshing = controller!.refresh();
    });
    const call = deferred<unknown>();
    backend.handlers.reorder_characters = () => call.promise;
    let reordering: Promise<void> = Promise.resolve();
    act(() => {
      reordering = controller!.reorder([meta("B"), meta("A")]);
    });
    await act(async () => {
      reread.resolve([meta("A"), meta("B"), meta("C")]);
      await refreshing;
    });
    backend.handlers.list_characters = () => [meta("A"), meta("B"), meta("C")];
    await act(async () => {
      call.reject("寫不進去");
      await reordering;
    });
    expect(controller!.list.map((character) => character.id)).toEqual(["A", "B", "C"]);
    expect(errors.filter(Boolean)).toEqual(["寫不進去"]);
  });
});
