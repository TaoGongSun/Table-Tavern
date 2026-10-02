// @vitest-environment happy-dom
// 角色卡／玩家卡編輯頁（CardEditor）的頂列與 ⋯：驗按鈕搬家後的接線，不驗存檔細節。
// 共用頁框 EditPage 的測試在 shared/ui/EditPage.test.tsx。

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { t } from "../../i18n";
import type { CharacterCard } from "./card-model";

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
vi.mock("@tauri-apps/plugin-dialog", () => ({
  confirm: vi.fn(async () => true),
  message: vi.fn(async () => {}),
  save: vi.fn(async () => null),
}));
vi.mock("@tauri-apps/plugin-opener", () => ({ revealItemInDir: vi.fn(async () => {}) }));

import { CardEditor } from "./CardEditor";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

function card(id: string, name: string, extra: Partial<CharacterCard> = {}): CharacterCard {
  return {
    id,
    name,
    color: "#e07a5f",
    avatar: "🎭",
    tier: "balanced",
    show_image: false,
    archived: false,
    auto_hidden: false,
    public_md: "",
    private_md: "",
    gen_prompt: "",
    ...extra,
  };
}

interface MountOptions {
  characterId?: string;
  isNew?: boolean;
  isPlayer?: boolean;
}

describe("CardEditor", () => {
  let root: Root | null = null;
  let host: HTMLDivElement | null = null;
  const onBack = vi.fn();
  const onDeleted = vi.fn(async () => {});
  const leaveGuard: { current: (() => Promise<boolean>) | null } = { current: null };

  beforeEach(() => {
    backend.handlers = {};
    backend.calls = [];
    onBack.mockReset();
    onDeleted.mockClear();
    leaveGuard.current = null;
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

  function element({ characterId = "c1", isNew = false, isPlayer = false }: MountOptions) {
    return (
      <CardEditor
        title="Title"
        world="w1"
        characterId={characterId}
        isNew={isNew}
        isPlayer={isPlayer}
        newCardColor="#e07a5f"
        onImagesChanged={async () => {}}
        onSaved={() => {}}
        onArchived={async () => {}}
        onDeleted={onDeleted}
        onBack={onBack}
        leaveGuard={leaveGuard}
        config={{ api_keys: {}, tier_models: {}, preferences: {} }}
        onPreference={async () => {}}
        onOpenAiSettings={() => {}}
        onConverted={async () => {}}
      />
    );
  }

  async function mount(options: MountOptions = {}) {
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    await act(async () => {
      root?.render(element(options));
    });
  }

  const moreButton = () =>
    host!.querySelector<HTMLButtonElement>(`[aria-label="${t("moreActions")}"]`);
  const saveButton = () => host!.querySelector<HTMLButtonElement>(".edit-page-save")!;
  const nameInput = () => host!.querySelector<HTMLInputElement>(".card-editor-identity input");
  const menuItems = () => [...document.querySelectorAll<HTMLElement>('[role="menuitem"]')];

  function openMore() {
    act(() => moreButton()!.click());
    return menuItems();
  }

  it("lists export, convert, archive and danger delete for an existing character", async () => {
    backend.handlers.read_character = () => card("c1", "Alice");
    await mount();
    const items = openMore();
    expect(items.map((item) => item.textContent)).toEqual([
      t("exportCard"),
      t("convertCardToEntry"),
      t("archiveCharacter"),
      t("deleteCharacter"),
    ]);
    expect(items[3].classList.contains("menu-item-danger")).toBe(true);
    // 刪除直接交給 controller（確認框在那裡），這裡不再包一層
    act(() => items[3].click());
    expect(onDeleted).toHaveBeenCalledTimes(1);
  });

  it("swaps the archive item to restore for an archived character", async () => {
    backend.handlers.read_character = () => card("c1", "Alice", { archived: true });
    await mount();
    expect(openMore().map((item) => item.textContent)).toContain(t("restoreCharacter"));
  });

  it("lists only export and delete for a player card", async () => {
    backend.handlers.read_character = () => card("p1", "Me");
    await mount({ characterId: "p1", isPlayer: true });
    expect(openMore().map((item) => item.textContent)).toEqual([
      t("exportCard"),
      t("deleteCharacter"),
    ]);
  });

  it("has no more menu for a new card", async () => {
    await mount({ characterId: "n1", isNew: true });
    expect(nameInput()).not.toBeNull();
    expect(moreButton()).toBeNull();
  });

  it("writes the character from the top-bar save button", async () => {
    backend.handlers.read_character = () => card("c1", "Alice");
    await mount();
    await act(async () => saveButton().click());
    const writes = backend.calls.filter((call) => call.command === "write_character");
    expect(writes).toHaveLength(1);
    expect((writes[0].args.card as CharacterCard).id).toBe("c1");
  });

  it("keeps back usable when loading fails", async () => {
    backend.handlers.read_character = () => {
      throw new Error("read failed");
    };
    await mount();
    expect(saveButton().disabled).toBe(true);
    expect(host!.querySelector('[role="alert"]')!.textContent).toContain("read failed");
    act(() => host!.querySelector<HTMLButtonElement>(".edit-page-back")!.click());
    expect(onBack).toHaveBeenCalledTimes(1);
  });

  it("does not expose the previous card while the next one is loading", async () => {
    backend.handlers.read_character = () => card("c1", "Alice");
    await mount();
    expect(nameInput()!.value).toBe("Alice");
    let release: (value: unknown) => void = () => {};
    backend.handlers.read_character = () => new Promise((resolve) => (release = resolve));
    await act(async () => {
      root?.render(element({ characterId: "c2" }));
    });
    // 新 ID 還沒讀到：不畫舊卡、儲存停用，舊卡的離開守門也不能留著
    expect(nameInput()).toBeNull();
    expect(saveButton().disabled).toBe(true);
    expect(await leaveGuard.current!()).toBe(true);
    await act(async () => release(card("c2", "Bob")));
    expect(nameInput()!.value).toBe("Bob");
    expect(saveButton().disabled).toBe(false);
  });

  it("ignores a previous card's request that resolves after the next card loaded", async () => {
    let releaseOld: (value: unknown) => void = () => {};
    backend.handlers.read_character = () => new Promise((resolve) => (releaseOld = resolve));
    await mount();
    backend.handlers.read_character = () => card("c2", "Bob");
    await act(async () => {
      root?.render(element({ characterId: "c2" }));
    });
    expect(nameInput()!.value).toBe("Bob");
    await act(async () => releaseOld(card("c1", "Alice")));
    expect(nameInput()!.value).toBe("Bob");
    expect(saveButton().disabled).toBe(false);
  });
});
