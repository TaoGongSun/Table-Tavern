// @vitest-environment happy-dom
// VN 開桌精靈三步：① 首屏只有一句話＋題材＋生成、全空白也能生成且帶偏好；①↔② 草稿不丟；建立後聖經與候選各排一次；
// ③ 首幕背景、按開始把目前候選裝成定稿、清設定階段、才排表情；返回 ② 已有聖經不重生、未存改動換步前先存。
import { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { t } from "../../../i18n";

const state = vi.hoisted(() => ({
  calls: [] as [string, Record<string, unknown>][],
  bible: null as unknown,
  candidates: [] as string[],
  /** 已裝定稿是哪張候選（undefined＝沒定稿） */
  installed: undefined as string | null | undefined,
  pickFails: false,
}));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (command: string, args: Record<string, unknown> = {}) => {
    state.calls.push([command, args]);
    switch (command) {
      case "generate_table_outline":
        return {
          parsed: { title: "霧港", world: "簡介", characters: [{ name: "莫斯", tagline: "隊長" }], player: { name: "you", tagline: "新人" } },
          raw: "raw",
        };
      case "generate_table_expand":
        return { worldId: "W", raw: "" };
      case "vn_bible_read":
        return { spoiler_lock: true, bible: state.bible, flagValues: {}, revealed: [], played: false, names: {}, cover: false };
      case "list_characters":
        return [{ id: "c1", name: "莫斯", archived: false, stage: false }];
      case "vn_sprite_pick":
        if (state.pickFails) throw new Error("disk full");
        return null;
      case "read_character":
        return { id: "c1", name: "莫斯", avatar: "🛡️", public_md: "守衛隊長", archived: false };
      case "detect_clis":
        return [];
      case "vn_sprite_read":
        return {
          sprite: state.installed === undefined ? null : { body: "data:image/png;base64,", meta: { candidate: state.installed ?? undefined } },
          candidates: state.candidates,
        };
      case "vn_sprite_candidate":
        return "data:image/png;base64,";
      case "vn_sprite_generate":
        return "new.png";
      case "vn_backgrounds_fill":
      case "vn_backgrounds_status":
        return [];
      case "read_config":
        return { preferences: {} };
      default:
        return null;
    }
  }),
}));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));

import { VnWizard, type VnWizardStart } from "./VnWizard";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

let host: HTMLDivElement;
let root: ReturnType<typeof createRoot>;
beforeEach(() => {
  state.calls.length = 0;
  state.bible = null;
  state.candidates = [];
  state.installed = undefined;
  state.pickFails = false;
  host = document.createElement("div");
  document.body.appendChild(host);
  root = createRoot(host);
});
afterEach(() => {
  act(() => root.unmount());
  host.remove();
});

// SwapLabel 兩個字都在 DOM 裡：用開頭比對
const button = (label: string) =>
  [...document.querySelectorAll("button")].find((b) => b.textContent?.trim().startsWith(label))!;
const called = (command: string) => state.calls.filter(([name]) => name === command);
const settle = () => act(async () => new Promise((resolve) => setTimeout(resolve, 0)));

const onStage = vi.fn();
async function mount(start: VnWizardStart, onFinish = vi.fn(), onCreated = vi.fn(async () => {})) {
  await act(async () =>
    root.render(
      <VnWizard
        start={start}
        onCreated={onCreated}
        onStage={onStage}
        onImportCard={() => {}}
        onFinish={onFinish}
        onClose={() => {}}
      />,
    ),
  );
  await settle();
}

it("walks idea → story → art with a blank idea, keeping the draft across steps", async () => {
  const onCreated = vi.fn(async () => {});
  await mount({ worldId: null }, vi.fn(), onCreated);
  expect(document.querySelectorAll(".vn-stepbar li")).toHaveLength(3);
  expect(document.querySelectorAll('[role="tab"]')).toHaveLength(2);
  expect(document.querySelector(".vn-more-prefs")).toBeNull();
  await act(async () => button(`${t("vnMorePrefs")} ▸`).click());
  expect(document.querySelectorAll(".vn-pref-choice")).toHaveLength(3);
  await act(async () => button(t("vnLengthShort")).click());

  // 什麼都沒填也能生成，偏好一起送
  await act(async () => button(t("genGenerateBtn")).click());
  const [, outlineArgs] = called("generate_table_outline")[0];
  expect(outlineArgs.input).toBe("");
  expect(outlineArgs.preferences).toMatchObject({ length: "short", name: "" });
  expect(document.querySelector(".gen-outline-preview")).not.toBeNull();

  // 上一步回 ① 草稿還在，生成鈕改名重寫
  await act(async () => button(t("vnPrevStep")).click());
  expect(button(t("vnRewriteOutline"))).toBeDefined();
  await act(async () => button(t("vnNext")).click());
  expect(document.querySelector<HTMLInputElement>(".gen-outline-preview > input")!.value).toBe("霧港");

  await act(async () => button(t("genCreateBtn")).click());
  await settle();
  const [, expandArgs] = called("generate_table_expand")[0];
  expect(expandArgs).toMatchObject({ tableKind: "vn", spoilerLock: true, preferences: { length: "short" } });
  expect(onCreated).toHaveBeenCalledWith("W");
  // 建立後：聖經寫一次、後端自動定稿排一次（③ 隱藏掛載；前端不自己生候選），① 變灰
  expect(called("vn_bible_generate")).toHaveLength(1);
  expect(called("vn_sprites_prepare")).toHaveLength(1);
  expect(called("vn_sprite_generate")).toHaveLength(0);
  expect(document.querySelector(".vn-stepbar li[data-disabled]")?.textContent).toContain(t("vnStepIdea"));
  expect(called("vn_backgrounds_fill")).toHaveLength(0);
});

it("resumes at art: queues the backdrop, installs the shown candidate and clears setup on start", async () => {
  state.bible = { chapter_max: 5, branching: "full", endings: [], chapters: [], secrets: [], flags: [] };
  state.candidates = ["a.png"];
  const onFinish = vi.fn();
  await mount({ worldId: "W", step: "art" }, onFinish);
  expect(called("vn_setup_set")[0][1]).toMatchObject({ worldId: "W", stage: "art" });
  // 階段寫入後大廳清單重讀（桌卡標記與重進路由用最新階段）
  expect(onStage).toHaveBeenCalled();
  expect(called("vn_backgrounds_fill")).toHaveLength(1);
  // 前端不自己生候選
  expect(called("vn_sprite_generate")).toHaveLength(0);
  expect(called("vn_bible_generate")).toHaveLength(0);
  expect(called("vn_sprites_fill")).toHaveLength(0);

  // 返回 ② 已有聖經：先讀完再決定，不重生
  await act(async () => button(t("vnPrevStep")).click());
  await settle();
  expect(called("vn_bible_generate")).toHaveLength(0);
  // 劇本規格層改了沒存：按下一步先存
  const branching = document.querySelector<HTMLSelectElement>(".vn-bible-branching select")!;
  await act(async () => {
    Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, "value")!.set!.call(branching, "none");
    branching.dispatchEvent(new Event("change", { bubbles: true }));
  });
  await act(async () => button(t("vnNext")).click());
  await settle();
  expect(called("vn_bible_save")[0][1]).toMatchObject({ worldId: "W", bible: { branching: "none" } });

  await act(async () => button(t("vnStartGame")).click());
  await settle();
  expect(called("vn_sprite_pick")[0][1]).toMatchObject({ characterId: "c1", file: "a.png" });
  const prepares = called("vn_sprites_prepare");
  expect(prepares[prepares.length - 1][1]).toMatchObject({ worldId: "W", adopt: [] });
  const setups = called("vn_setup_set");
  expect(setups[setups.length - 1][1]).toMatchObject({ stage: null });
  expect(called("vn_sprites_fill")).toHaveLength(1);
  expect(onFinish).toHaveBeenCalledWith("W");
});

it("stays on art and shows the failure when installing the shown candidate fails", async () => {
  state.bible = { chapter_max: 5, branching: "full", endings: [], chapters: [], secrets: [], flags: [] };
  state.candidates = ["a.png"];
  state.pickFails = true;
  const onFinish = vi.fn();
  await mount({ worldId: "W", step: "art" }, onFinish);
  await act(async () => button(t("vnStartPreparing", { n: 0, total: 1 })).click());
  await settle();
  expect(onFinish).not.toHaveBeenCalled();
  expect(called("vn_setup_set").some(([, args]) => args.stage === null)).toBe(false);
  expect(document.querySelector('.vn-sprite-row [role="alert"]')?.textContent).toContain("disk full");
});

it("knows which candidate is installed: the same one is not picked again, an upload is not replaced", async () => {
  state.bible = { chapter_max: 5, branching: "full", endings: [], chapters: [], secrets: [], flags: [] };
  state.candidates = ["b.png", "a.png"];
  state.installed = "a.png";
  await mount({ worldId: "W", step: "art" });
  // 後端自動定稿裝了舊的那張：預設選它，「挑定這張」變灰
  expect(document.querySelector<HTMLButtonElement>(".vn-picker-actions .btn-primary")!.disabled).toBe(true);
  await act(async () => button(t("vnStartGame")).click());
  await settle();
  expect(called("vn_sprite_pick")).toHaveLength(0);

  // 上傳的定稿（不記候選）：不選任何候選，按開始不蓋掉上傳
  act(() => root.unmount());
  root = createRoot(host);
  state.calls.length = 0;
  state.installed = null;
  state.candidates = ["a.png"];
  await mount({ worldId: "W", step: "art" });
  // 唯一的候選沒被選：仍列出縮圖可點回
  expect(document.querySelectorAll(".vn-picker-thumb")).toHaveLength(1);
  await act(async () => button(t("vnStartGame")).click());
  await settle();
  expect(called("vn_sprite_pick")).toHaveLength(0);
});
