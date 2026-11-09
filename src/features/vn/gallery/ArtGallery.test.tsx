// @vitest-environment happy-dom
// 美術一覽：背景格（專屬圖、基底＋遮罩預覽、生成專屬圖、上傳、排隊取消、失敗重試、燈箱）、重播桌唯讀、立繪狀態點。
import { act } from "react";
import { createRoot } from "react-dom/client";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { t } from "../../../i18n";
import type { AudioEntry, BackgroundPlace, BackgroundStatus } from "../../../shared/contracts/backend-contracts";
import { ArtGallery } from "./ArtGallery";
import { BackgroundGallery } from "./BackgroundGallery";
import { SpriteGallery } from "./SpriteGallery";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const calls: [string, Record<string, unknown>][] = [];
const artPause = { on: false };
const audio = { entries: [] as AudioEntry[], fail: false };
const openUrl = vi.hoisted(() => vi.fn(async () => {}));
const harbor: BackgroundPlace = {
  slug: "aaaaaaaaaaaaaaaa",
  location: "港口",
  periods: ["day", "morning", "dusk", "night"],
  base: "day",
  dedicated: ["day", "night"],
};
const cave: BackgroundPlace = { slug: "bbbbbbbbbbbbbbbb", location: "洞穴", periods: ["moonlight", "crisis"], base: null, dedicated: [] };

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (command: string, args: Record<string, unknown>) => {
    calls.push([command, args]);
    switch (command) {
      case "vn_backgrounds_list":
        return [harbor, cave];
      case "list_characters":
        return [
          { id: "c1", name: "阿哲", archived: false, stage: false },
          { id: "c2", name: "小林", archived: false, stage: false },
        ];
      case "vn_sprites_read_all":
        return { c1: { body: "data:c1", faces: { happy: "data:h" }, expressions: [] } };
      case "vn_sprites_status":
        return [
          { worldId: "w", characterId: "c1", variant: null, expression: "sad", state: "running", reason: null },
          { worldId: "w", characterId: "c2", variant: null, expression: "base", state: artPause.on ? "queued" : "failed", reason: "x" },
        ];
      case "vn_sprite_read": return { candidates: [], sprite: null };
      case "vn_art_status": return { art: { started: true }, ready: 0, total: 2, backgrounds: 1, pending: 3, faces: 8, pause: artPause.on ? { reset: null } : null };
      case "vn_audio_read":
        if (audio.fail) throw new Error("boom");
        return { files: [], entries: audio.entries };
      default:
        return null;
    }
  }),
}));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl }));

// statusOf 一覽不用，給個佔位
const queuedNone = undefined as unknown as BackgroundStatus;
const settle = () => act(async () => new Promise((resolve) => setTimeout(resolve, 0)));

async function renderBackgrounds(readOnly: boolean, status: Record<string, BackgroundStatus> = {}) {
  const load = vi.fn();
  const onView = vi.fn();
  const images = {
    "aaaaaaaaaaaaaaaa-day": { src: "data:day", period: "day", rev: 1 },
    "aaaaaaaaaaaaaaaa-night": { src: "data:night", period: "night", rev: 2 },
  };
  const host = document.createElement("div");
  await act(async () =>
    createRoot(host).render(
      <BackgroundGallery worldId="w" backgrounds={{ pause: null, retry: async () => {}, images, status, statusOf: () => queuedNone, load }} readOnly={readOnly} onView={onView} />,
    ),
  );
  await settle();
  return { host, load, onView };
}

const cells = (host: HTMLElement) => [...host.querySelectorAll(".vn-gallery-cell")];
const buttons = (cell: Element) => [...cell.querySelectorAll(".vn-gallery-actions button")].map((button) => button.textContent);

describe("BackgroundGallery", () => {
  beforeEach(() => {
    calls.length = 0;
  });

  it("lays out place × period: own images, tinted base preview, generate own image", async () => {
    const { host, load } = await renderBackgrounds(false);
    expect(load.mock.calls.map(([backdrop]) => backdrop.period)).toEqual(["day", "night"]);
    const [day, morning, dusk, night, moonlight, crisis] = cells(host);
    expect(day.querySelector("img")?.getAttribute("src")).toBe("data:day");
    expect(day.textContent).toContain(t("vnGalleryBase"));
    expect(buttons(day)).toEqual([t("vnGalleryRegenerate"), t("vnGalleryUpload")]);
    // 沒專屬圖：基底圖＋該時段遮罩
    expect(dusk.querySelector("img")?.getAttribute("src")).toBe("data:day");
    expect(dusk.querySelector(".vn-gallery-tint")).not.toBeNull();
    expect(dusk.textContent).toContain(t("vnGalleryTintPreview"));
    expect(buttons(morning)).toEqual([t("vnGalleryGeneratePeriod"), t("vnGalleryUploadOwn")]);
    expect(night.querySelector("img")?.getAttribute("src")).toBe("data:night");
    expect(night.querySelector(".vn-gallery-tint")).toBeNull();
    // 卡自訂時段、地點還沒圖：顯示卡上的鍵，第一格生成（成為基底）
    expect(moonlight.textContent).toContain("moonlight");
    expect(buttons(moonlight)).toEqual([t("vnGalleryGenerate"), t("vnGalleryUploadOwn")]);
    expect(buttons(crisis)).toEqual([t("vnGalleryUploadOwn")]);
    await act(async () => (morning.querySelector(".vn-gallery-actions button") as HTMLButtonElement).click());
    expect(calls[calls.length - 1]).toEqual(["vn_background_generate_period", { worldId: "w", slug: harbor.slug, period: "morning" }]);
  });

  it("opens the lightbox with the cell's tint", async () => {
    const { host, onView } = await renderBackgrounds(true);
    await act(async () => (cells(host)[2].querySelector(".vn-gallery-open") as HTMLButtonElement).click());
    expect(onView).toHaveBeenCalledWith(
      expect.objectContaining({ kind: "background", src: "data:day", tint: expect.objectContaining({ color: expect.any(String) }) }),
    );
  });

  it("failed cells retry the same action", async () => {
    const failed: BackgroundStatus = { worldId: "w", slug: harbor.slug, period: "dusk", state: "failed", reason: "boom" };
    const { host } = await renderBackgrounds(false, { [`${harbor.slug}-dusk`]: failed });
    const dusk = cells(host)[2];
    expect(dusk.querySelector(".field-warn")).not.toBeNull();
    expect(buttons(dusk)[0]).toBe(t("vnGalleryRetry"));
    await act(async () => (dusk.querySelector(".vn-gallery-actions button") as HTMLButtonElement).click());
    expect(calls[calls.length - 1]).toEqual(["vn_background_generate_period", { worldId: "w", slug: harbor.slug, period: "dusk" }]);
  });

  it("uploads by slug and period; queued cells can be cancelled and disable their buttons", async () => {
    const queued: BackgroundStatus = { worldId: "w", slug: harbor.slug, period: "dusk", state: "queued", reason: null };
    const { host } = await renderBackgrounds(false, { [`${harbor.slug}-dusk`]: queued });
    const dusk = cells(host)[2];
    expect(dusk.textContent).toContain(t("vnGalleryQueued"));
    expect([...dusk.querySelectorAll<HTMLButtonElement>(".vn-gallery-actions button")].every((button) => button.disabled)).toBe(true);
    await act(async () => (dusk.querySelector(".vn-gallery-busy button") as HTMLButtonElement).click());
    expect(calls[calls.length - 1]).toEqual(["vn_background_cancel", { worldId: "w", slug: harbor.slug, period: "dusk" }]);
    const input = cells(host)[0].querySelector("input") as HTMLInputElement;
    const file = new File([new Uint8Array([1, 2, 3])], "a.png", { type: "image/png" });
    Object.defineProperty(input, "files", { value: [file] });
    await act(async () => input.dispatchEvent(new Event("change", { bubbles: true })));
    await settle();
    expect(calls[calls.length - 1]).toEqual(["vn_background_upload", { worldId: "w", slug: harbor.slug, period: "day", data: [1, 2, 3] }]);
  });

  it("replay tables only show images", async () => {
    const { host } = await renderBackgrounds(true);
    expect(host.querySelectorAll(".vn-gallery-actions")).toHaveLength(0);
    expect(host.querySelectorAll("img").length).toBeGreaterThan(0);
  });
});

describe("SpriteGallery", () => {
  async function renderSprites(readOnly: boolean) {
    const host = document.createElement("div");
    const onView = vi.fn();
    await act(async () =>
      createRoot(host).render(<SpriteGallery worldId="w" images={{ c2: "data:card" }} readOnly={readOnly} onView={onView} />),
    );
    await settle();
    return { onView, rows: [...host.querySelectorAll<HTMLElement>(".vn-gallery-row")] };
  }
  const dots = (row: Element) => [...row.querySelectorAll(".vn-gallery-dot")].map((dot) => dot.className.split("-").pop());

  it("one row per character with base and expression dots, thumbnail view and edit", async () => {
    const { rows, onView } = await renderSprites(false);
    expect(rows.map((row) => row.querySelector(".vn-gallery-name")?.firstChild?.textContent)).toEqual(["阿哲", "小林"]);
    expect(dots(rows[0])).toEqual(["done", "done", "running", "missing", "missing"]);
    expect(rows[1].querySelector("img")?.getAttribute("src")).toBe("data:card");
    expect(rows[1].textContent).toContain(t("vnGalleryBaseFailed"));
    expect(dots(rows[1])[0]).toBe("failed");
    await act(async () => rows[0].querySelector<HTMLButtonElement>(".vn-gallery-thumb")!.click());
    expect(onView).toHaveBeenCalledWith(expect.objectContaining({ kind: "sprite", label: "阿哲" }));
    expect(rows[0].textContent).toContain(t("vnGalleryEdit"));
  });

  it("replay rows view only and draw only finished expressions", async () => {
    const { rows } = await renderSprites(true);
    expect(dots(rows[0])).toEqual(["done", "done"]);
    expect(rows[0].textContent).not.toContain(t("vnGalleryEdit"));
    expect(rows[0].querySelector<HTMLButtonElement>(".vn-gallery-thumb")!.disabled).toBe(false);
  });
});

describe("ArtGallery music", () => {
  beforeEach(() => {
    calls.length = 0;
    audio.entries = [];
    audio.fail = false;
    openUrl.mockClear();
  });

  async function openMusic(replay = false) {
    const host = document.createElement("div");
    await act(async () => {
      createRoot(host).render(
        <ArtGallery worldId="w" images={{}} backgrounds={{} as never} replay={replay} onClose={() => {}} />,
      );
    });
    await settle();
    const tab = [...host.querySelectorAll<HTMLButtonElement>("[role=tab]")].find((button) => button.textContent === t("vnGalleryTabMusic"));
    await act(async () => tab!.click());
    await settle();
    return host;
  }

  it("names the panel and adds a music tab that lists both kinds", async () => {
    audio.entries = [
      { kind: "bgm", key: "theme", title: "Theme", author: "Kevin MacLeod", source_url: "https://incompetech.com/a", license: "Attribution" },
      { kind: "sfx", key: "door", title: "Door", author: "Ann", source_url: "http://freesound.org/a", license: "Creative Commons 0" },
      { kind: "sfx", key: "bell", title: "Bell", author: "Bo", source_url: "", license: "CC0" },
    ];
    const host = await openMusic(true);
    expect(host.querySelector(".vn-story-panel")?.getAttribute("aria-label")).toBe(t("vnGallery"));
    expect(t("vnGallery")).toBe("美術與音樂");
    const body = host.querySelector(".vn-credits")!;
    expect([...body.querySelectorAll("h3")].map((node) => node.textContent)).toEqual([t("vnCreditsMusic"), t("vnCreditsSfx")]);
    expect(body.textContent).toContain("Theme");
    expect(body.textContent).toContain("CC BY");
    expect(body.textContent).toContain("CC0");
    const source = [...body.querySelectorAll("button")].filter((button) => button.textContent === t("vnCreditsSource"));
    expect(source).toHaveLength(1);
    await act(async () => source[0].click());
    expect(openUrl).toHaveBeenCalledWith("https://incompetech.com/a");
    expect(calls.filter(([command]) => command === "vn_audio_read")).toHaveLength(1);
  });

  it("hides an empty section and shows the empty state when nothing is landed", async () => {
    audio.entries = [
      { kind: "bgm", key: "theme", title: "Theme", author: "Kevin MacLeod", source_url: "https://incompetech.com/a", license: "Attribution" },
    ];
    const onlyMusic = await openMusic();
    expect(onlyMusic.querySelector(".vn-credits")?.textContent).not.toContain(t("vnCreditsSfx"));
    audio.entries = [];
    const empty = await openMusic();
    expect(empty.querySelector(".vn-credits")).toBeNull();
    expect(empty.textContent).toContain(t("vnCreditsEmpty"));
    audio.fail = true;
    audio.entries = [
      { kind: "bgm", key: "theme", title: "Theme", author: "Kevin MacLeod", source_url: "https://incompetech.com/a", license: "Attribution" },
    ];
    expect((await openMusic()).textContent).toContain(t("vnCreditsEmpty"));
  });
});

it("has one source selector in the gallery character editor and explains queued quota waits", async () => {
  artPause.on = true;
  const host = document.createElement("div"); const root = createRoot(host);
  await act(async () => root.render(<ArtGallery worldId="w" images={{}} backgrounds={{ pause: { reset: null } } as never} replay={false} onClose={() => {}} />));
  await settle();
  expect(host.querySelectorAll("select")).toHaveLength(1);
  expect(host.querySelectorAll(".vn-gallery-row")[1].textContent).toContain(t("vnArtWaitQuota"));
  await act(async () => host.querySelectorAll<HTMLButtonElement>(".vn-gallery-row .btn")[1].click());
  await settle();
  expect(host.querySelectorAll("select")).toHaveLength(1);
  expect(host.querySelector(".vn-picker-empty")?.textContent).toBe(t("vnArtWaitQuota"));
  await act(async () => root.unmount());
  artPause.on = false;
});
