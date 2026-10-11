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
    let removed = false;
    await act(async () => {
      removed = await controller!.remove("c1");
    });
    expect(removed).toBe(true);
    expect(lastError()).toBe(t("deleteCharacterCleanupFailed"));
  });

  it("stays quiet when the cleanup finished or the backend returns nothing", async () => {
    for (const result of [{ worldbook_cleanup_failed: false }, undefined]) {
      backend.handlers.delete_character = () => result;
      let removed = false;
      await act(async () => {
        removed = await controller!.remove("c1");
      });
      expect(removed).toBe(true);
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
});
