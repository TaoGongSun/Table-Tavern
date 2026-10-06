// @vitest-environment happy-dom
// 角色編輯儲存的存圖順序：圖先驗，驗證失敗零寫入、欄位與草稿保留；預驗期間重複儲存、換卡、換草稿
// 不讓晚回的結果寫檔或蓋掉新草稿，鎖一律放開；預驗通過後寫圖失敗回報錯誤、草稿保留。
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { CharacterCard, DraftImage } from "./card-model";

const backend = vi.hoisted(() => ({
  handlers: {} as Record<string, (args: Record<string, unknown>) => unknown>,
  calls: [] as { command: string; args: Record<string, unknown> }[],
}));
// 裁切窗替身：按下就把 next[cropShape] 當成裁好的草稿交回
const crop = vi.hoisted(() => ({
  next: {} as Record<string, DraftImage>,
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
vi.mock("./CardImageDialogs", () => ({
  AiImageDialog: () => null,
  CropDialog: ({
    cropShape,
    onConfirm,
    onCancel,
  }: {
    cropShape: string;
    onConfirm: (image: DraftImage) => Promise<void>;
    onCancel: () => void;
  }) => (
    <button
      type="button"
      className={`stub-crop-${cropShape}`}
      onClick={async () => {
        await onConfirm(crop.next[cropShape]);
        onCancel();
      }}
    >
      crop
    </button>
  ),
}));

import { t } from "../../i18n";
import { CardEditor } from "./CardEditor";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

function card(id: string, name: string): CharacterCard {
  return {
    id,
    name,
    color: "#e07a5f",
    avatar: "🎭",
    tier: "balanced",
    show_image: true,
    archived: false,
    auto_hidden: false,
    public_md: "",
    private_md: "",
    gen_prompt: "",
  };
}

function draft(byte: number): DraftImage {
  return { bytes: [byte], url: `data:image/png;base64,${btoa(String.fromCharCode(byte))}` };
}

const VALID = 1;
const INVALID = 2;

let root: Root | null = null;
let host: HTMLDivElement | null = null;
const callbacks = { images: async () => {}, saved: vi.fn() };
const leaveGuard: { current: (() => Promise<boolean>) | null } = { current: null };

function element(characterId: string) {
  return (
    <CardEditor
      title="Title"
      world="w1"
      characterId={characterId}
      isNew={false}
      newCardColor="#e07a5f"
      onImagesChanged={() => callbacks.images()}
      onSaved={callbacks.saved}
      onArchived={async () => {}}
      onDeleted={async () => {}}
      onBack={() => {}}
      leaveGuard={leaveGuard}
      config={{ api_keys: {}, tier_models: {}, preferences: {} }}
      onPreference={async () => {}}
      onOpenAiSettings={() => {}}
      onConverted={async () => {}}
      isBusy={() => false}
    />
  );
}

async function render(characterId: string) {
  await act(async () => root?.render(element(characterId)));
}

const calls = (command: string) => backend.calls.filter((call) => call.command === command);
const saveButton = () => host!.querySelector<HTMLButtonElement>(".edit-page-save")!;
const nameInput = () => host!.querySelector<HTMLInputElement>(".card-editor-identity input")!;
const errorText = () => host!.querySelector('[role="alert"]')?.textContent ?? "";

async function clickSave() {
  await act(async () => saveButton().click());
}

/** 選一張大圖檔 → 裁切替身交回 next.rect */
async function pickImage(characterId: string, byte: number) {
  crop.next.rect = draft(byte);
  const input = host!.querySelector<HTMLInputElement>(`#character-image-${characterId}`)!;
  Object.defineProperty(input, "files", {
    configurable: true,
    value: [new File([new Uint8Array([byte])], "pic.png", { type: "image/png" })],
  });
  await act(async () => input.dispatchEvent(new Event("change", { bubbles: true })));
  await vi.waitFor(() => expect(host!.querySelector(".stub-crop-rect")).not.toBeNull());
  await act(async () => host!.querySelector<HTMLButtonElement>(".stub-crop-rect")!.click());
}

/** 從大圖裁頭像 → 裁切替身交回 next.round */
async function pickAvatar(byte: number) {
  crop.next.round = draft(byte);
  const make = [...host!.querySelectorAll<HTMLButtonElement>("button")].find(
    (button) => button.textContent === t("makeAvatarBtn"),
  )!;
  await act(async () => make.click());
  await act(async () => host!.querySelector<HTMLButtonElement>(".stub-crop-round")!.click());
}

function deferred() {
  let resolve: () => void = () => {};
  const promise = new Promise<void>((done) => (resolve = done));
  return { promise, resolve };
}

beforeEach(async () => {
  callbacks.images = async () => {};
  callbacks.saved.mockClear();
  backend.calls = [];
  backend.handlers = {
    read_character: (args) => card(String(args.characterId), args.characterId === "c1" ? "Alice" : "Bob"),
    check_character_image: (args) => {
      if ((args.data as number[])[0] === INVALID) throw new Error("PNG 檔案損壞");
    },
  };
  host = document.createElement("div");
  document.body.appendChild(host);
  root = createRoot(host);
  await render("c1");
});

afterEach(() => {
  act(() => root?.unmount());
  host?.remove();
  root = null;
  host = null;
  document.body.innerHTML = "";
});

describe("CardEditor 存圖順序", () => {
  it("大圖合法、頭像不合格：零寫入，欄位與兩張草稿都留著", async () => {
    const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!;
    await act(async () => {
      setter.call(nameInput(), "Alicia");
      nameInput().dispatchEvent(new Event("input", { bubbles: true }));
    });
    await pickImage("c1", VALID);
    await pickAvatar(INVALID);
    await clickSave();
    expect(calls("check_character_image").length).toBeGreaterThan(0);
    expect(calls("write_character")).toHaveLength(0);
    expect(calls("save_character_image")).toHaveLength(0);
    expect(calls("save_character_avatar")).toHaveLength(0);
    expect(errorText()).toContain("PNG 檔案損壞");
    expect(nameInput().value).toBe("Alicia");

    // 草稿還在、鎖也放開了：改成接受後再存，兩張都照原草稿寫進去
    backend.handlers.check_character_image = () => {};
    await clickSave();
    expect(calls("write_character")).toHaveLength(1);
    expect(calls("save_character_image")[0].args.data).toEqual([VALID]);
    expect(calls("save_character_avatar")[0].args.data).toEqual([INVALID]);
  });

  it("預驗期間重複按只跑一輪；換了草稿就放棄該輪，新草稿不被清，之後能再存", async () => {
    await pickImage("c1", VALID);
    const gate = deferred();
    backend.handlers.check_character_image = () => gate.promise;
    await clickSave();
    await clickSave();
    expect(calls("check_character_image")).toHaveLength(1);

    await pickImage("c1", 7);
    await act(async () => gate.resolve());
    expect(calls("write_character")).toHaveLength(0);
    expect(calls("save_character_image")).toHaveLength(0);

    backend.handlers.check_character_image = () => {};
    await clickSave();
    expect(calls("write_character")).toHaveLength(1);
    expect(calls("save_character_image")).toHaveLength(1);
    expect(calls("save_character_image")[0].args.data).toEqual([7]);
  });

  it("預驗期間換卡：舊輪不寫檔、不改新卡畫面，新卡照常能存", async () => {
    await pickImage("c1", VALID);
    const gate = deferred();
    backend.handlers.check_character_image = () => gate.promise;
    await clickSave();

    await render("c2");
    expect(nameInput().value).toBe("Bob");
    await act(async () => gate.resolve());
    expect(calls("write_character")).toHaveLength(0);
    expect(errorText()).toBe("");
    expect(nameInput().value).toBe("Bob");

    backend.handlers.check_character_image = () => {};
    await clickSave();
    const writes = calls("write_character");
    expect(writes).toHaveLength(1);
    expect((writes[0].args.card as CharacterCard).id).toBe("c2");
  });

  it("預驗失敗、再換草稿後的晚回不覆寫畫面；鎖放開", async () => {
    await pickImage("c1", INVALID);
    await clickSave();
    expect(errorText()).toContain("PNG 檔案損壞");
    await pickImage("c1", VALID);
    await clickSave();
    expect(calls("write_character")).toHaveLength(1);
    expect(calls("save_character_image")[0].args.data).toEqual([VALID]);
  });

  it("預驗通過後寫圖失敗：顯示錯誤、草稿留著，再存會重送同一張", async () => {
    await pickImage("c1", VALID);
    backend.handlers.save_character_image = () => {
      throw new Error("disk full");
    };
    await clickSave();
    expect(errorText()).toContain("disk full");
    expect(calls("save_character_image")).toHaveLength(1);

    backend.handlers.save_character_image = () => null;
    await clickSave();
    expect(calls("save_character_image")).toHaveLength(2);
    expect(calls("save_character_image")[1].args.data).toEqual([VALID]);
    expect(errorText()).toBe("");
  });

  async function removeDraftImage() {
    const button = [...host!.querySelectorAll<HTMLButtonElement>("button")].find(
      (candidate) => candidate.textContent === t("removeImageBtn"),
    )!;
    await act(async () => button.click());
  }

  async function release(gate: { resolve: () => void }) {
    await act(async () => {
      gate.resolve();
      await new Promise((done) => setTimeout(done, 0));
    });
  }

  it("寫檔期間換卡並在新卡標記移除圖：舊輪回來不清新卡草稿，新卡照樣刪圖", async () => {
    await pickImage("c1", VALID);
    await removeDraftImage();
    const gate = deferred();
    backend.handlers.write_character = () => gate.promise;
    await clickSave();
    await render("c2");
    await pickImage("c2", 7);
    await removeDraftImage();
    await release(gate);

    backend.calls = [];
    backend.handlers.write_character = () => null;
    await clickSave();
    expect(calls("write_character")).toHaveLength(1);
    expect(calls("delete_character_image")).toHaveLength(1);
    expect(calls("delete_character_image")[0].args.characterId).toBe("c2");
  });

  it("寫檔期間換走再換回同一張卡並改名：舊輪回來不蓋掉新的編輯", async () => {
    await pickImage("c1", VALID);
    const gate = deferred();
    backend.handlers.write_character = () => gate.promise;
    await clickSave();
    await render("c2");
    await render("c1");
    const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!;
    await act(async () => {
      setter.call(nameInput(), "Alicia");
      nameInput().dispatchEvent(new Event("input", { bubbles: true }));
    });
    await release(gate);
    expect(nameInput().value).toBe("Alicia");
    expect(errorText()).toBe("");

    // 鎖已放開：新的這輪照常存得進去
    backend.handlers.write_character = () => null;
    await clickSave();
    const writes = calls("write_character");
    expect((writes[writes.length - 1].args.card as CharacterCard).name).toBe("Alicia");
  });

  it("重讀圖期間換卡：舊輪不送 onSaved（不把畫面拉回舊卡）", async () => {
    const gate = deferred();
    callbacks.images = () => gate.promise;
    await pickImage("c1", VALID);
    await clickSave();
    await render("c2");
    await release(gate);
    expect(callbacks.saved).not.toHaveBeenCalled();

    // 沒換卡時照常送
    callbacks.images = async () => {};
    await clickSave();
    expect(callbacks.saved).toHaveBeenCalledWith("c2");
  });
});
