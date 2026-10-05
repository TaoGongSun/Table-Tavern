// @vitest-environment happy-dom
// 世界設定頁（WorldEditor）：頂列儲存只送 world.md 那張表單，世界書的條目表單是另一張，彼此不串。

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { t } from "../../i18n";
import type { WorldbookEntry } from "../../shared/contracts/backend-contracts";
import type { PendingRefactorCard } from "../refactor/refactor-card";

const backend = vi.hoisted(() => ({
  handlers: {} as Record<string, (args: Record<string, unknown>) => unknown>,
  calls: [] as { command: string; args: Record<string, unknown> }[],
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (command: string, args: Record<string, unknown> = {}) => {
    backend.calls.push({ command, args });
    const handler = backend.handlers[command];
    return handler ? handler(args) : null;
  }),
}));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock("@tauri-apps/plugin-dialog", () => ({
  confirm: vi.fn(async () => true),
  message: vi.fn(async () => {}),
  save: vi.fn(async () => null),
}));
vi.mock("@tauri-apps/plugin-opener", () => ({ revealItemInDir: vi.fn(async () => {}) }));

import { WorldEditor } from "./WorldEditor";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const ENTRY: WorldbookEntry = {
  uid: 7,
  title: "Harbor",
  keys: ["harbor"],
  content: "Foggy.",
  constant: false,
  order: 0,
  disabled: false,
  locked: false,
  visibility: { type: "gm" },
};

describe("WorldEditor", () => {
  let root: Root | null = null;
  let host: HTMLDivElement | null = null;
  const leaveGuard: { current: (() => Promise<boolean>) | null } = { current: null };

  beforeEach(() => {
    backend.handlers = {
      read_world_md: () => "World text",
      read_worldbook: () => [ENTRY],
      list_characters: () => [],
      mechanism_ledger: () => ({ entries: [], rejected: 0, clamped: 0, errors: 0, jumps: 0 }),
    };
    backend.calls = [];
  });

  afterEach(() => {
    act(() => {
      root?.unmount();
    });
    host?.remove();
    root = null;
    host = null;
    document.body.innerHTML = "";
  });

  const taken = vi.fn();

  function editor(pending: PendingRefactorCard | null = null) {
    return (
      <WorldEditor
        title={t("worldAria")}
        world="w1"
        worldName="Alpha"
        onBack={() => {}}
        leaveGuard={leaveGuard}
        convertColor="#e07a5f"
        onEntryConverted={async () => {}}
        onRefactorApplied={async () => {}}
        isTurnRunning={() => false}
        pendingRefactorCard={pending}
        onPendingRefactorCardTaken={taken}
      />
    );
  }

  async function mount() {
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    await act(async () => {
      root?.render(
        <WorldEditor
          title={t("worldAria")}
          world="w1"
          worldName="Alpha"
          onBack={() => {}}
          leaveGuard={leaveGuard}
          convertColor="#e07a5f"
          onEntryConverted={async () => {}}
          onRefactorApplied={async () => {}}
          isTurnRunning={() => false}
          pendingRefactorCard={null}
          onPendingRefactorCardTaken={() => {}}
        />,
      );
    });
  }

  const commands = (name: string) => backend.calls.filter((call) => call.command === name);
  const saveButton = () => host!.querySelector<HTMLButtonElement>(".edit-page-save")!;

  it("saves only world.md from the top bar", async () => {
    await mount();
    await act(async () => saveButton().click());
    expect(commands("write_world_md")).toHaveLength(1);
    expect(commands("upsert_worldbook_entry")).toHaveLength(0);
  });

  it("keeps the entry form outside the world form", async () => {
    await mount();
    const worldForm = document.getElementById(saveButton().getAttribute("form")!)!;
    expect(worldForm.querySelector("textarea")).not.toBeNull();
    expect(worldForm.querySelector(".worldbook-section")).toBeNull();
    const edit = [
      ...host!.querySelectorAll<HTMLButtonElement>(".worldbook-row-actions button"),
    ].find((button) => button.textContent === t("editBtn"))!;
    await act(async () => edit.click());
    const entryInput = host!.querySelector<HTMLInputElement>(".worldbook-form input")!;
    // 條目欄位按 Enter＝隱式送出它所屬的表單；那是條目表單，不會寫 world.md
    expect(entryInput.form).not.toBe(worldForm);
    await act(async () => entryInput.form!.requestSubmit());
    expect(commands("upsert_worldbook_entry")).toHaveLength(1);
    expect(commands("write_world_md")).toHaveLength(0);
  });

  it("takes a pending refactor card only for its own table, once per generation", async () => {
    taken.mockReset();
    backend.handlers.refactor_card_open = () => ({
      card: { characters: [{ name: "阿福", source_uids: ["1"] }] },
      assets: [],
      token: null,
    });
    await mount();
    const file = new File(["{}"], "card.png");
    await act(async () => root?.render(editor({ worldId: "other", file, generation: 1 })));
    expect(commands("refactor_card_open")).toHaveLength(0);
    expect(taken).not.toHaveBeenCalled();
    const pending = { worldId: "w1", file, generation: 2 };
    await act(async () => root?.render(editor(pending)));
    await vi.waitFor(() => expect(commands("refactor_card_open")).toHaveLength(1));
    expect(taken).toHaveBeenCalledWith(2);
    // 同一份待處理檔重繪不會再開一次
    await act(async () => root?.render(editor(pending)));
    expect(commands("refactor_card_open")).toHaveLength(1);
  });
});
