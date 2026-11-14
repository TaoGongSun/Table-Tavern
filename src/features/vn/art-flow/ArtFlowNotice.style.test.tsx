// @vitest-environment happy-dom
// 畫風輸入框與被拒絕（plans/vn-player-art-style.md §4）：存好才生圖、存失敗不生圖、被拒絕的三個選擇、標題畫面有路可走。
import { act } from "react";
import { createRoot } from "react-dom/client";
import { beforeEach, expect, it, vi } from "vitest";
import { t } from "../../../i18n";
import { ArtFlowNotice } from "./ArtFlowNotice";

const backend = vi.hoisted(() => ({
  calls: [] as { command: string; args: Record<string, unknown> }[],
  started: false,
  style: "",
  avoid: "",
  refused: 0,
  saveFails: false,
}));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (command: string, args: Record<string, unknown> = {}) => {
    backend.calls.push({ command, args });
    if (command === "read_config") return { preferences: { vn_sprite_source: "grok", "cli_connected:grok": true } };
    if (command === "detect_clis") return [{ id: "grok" }];
    if (command === "vn_art_style_set") {
      if (backend.saveFails) throw new Error("disk full");
      const style = args.style as { style: string; avoid: string };
      return { style: style.style.trim(), avoid: style.avoid.trim() };
    }
    if (command === "vn_art_status")
      return { art: { started: backend.started, appeared: [], style: backend.style || undefined, avoid: backend.avoid || undefined }, source: "grok", ready: 1, failed: 0, refused: backend.refused, total: 3, backgrounds: 1, faces: 8, facesPerCharacter: 4, pause: null, pending: 0 };
    return null;
  }),
}));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));
(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
beforeEach(() => Object.assign(backend, { calls: [], started: false, style: "", avoid: "", refused: 0, saveFails: false }));

const names = () => backend.calls.map((call) => call.command);
const button = (host: HTMLElement, label: string) => [...host.querySelectorAll("button")].find((b) => b.textContent === label);

async function mount(props: Partial<Parameters<typeof ArtFlowNotice>[0]> = {}) {
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  await act(async () => root.render(<ArtFlowNotice worldId="w" {...props} />));
  return { host, unmount: () => act(async () => root.unmount()) };
}

async function type(area: HTMLTextAreaElement, text: string) {
  await act(async () => {
    const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, "value")!.set!;
    setter.call(area, text);
    area.dispatchEvent(new Event("input", { bubbles: true }));
  });
}

it("starts small, opens the avoid box when it has text, and saves on blur without generating", async () => {
  const { host, unmount } = await mount();
  const [style] = host.querySelectorAll("textarea");
  expect(Number(style.rows)).toBe(1);
  expect(host.querySelector("details")?.open).toBe(false);
  await type(style, " 水彩手繪 ");
  await act(async () => style.dispatchEvent(new FocusEvent("focusout", { bubbles: true })));
  const saved = backend.calls.find((call) => call.command === "vn_art_style_set");
  expect(saved?.args).toEqual({ worldId: "w", style: { style: " 水彩手繪 ", avoid: "" } });
  expect(style.value).toBe("水彩手繪");
  expect(host.textContent).toContain(t("vnStyleSaved"));
  expect(names()).not.toContain("vn_art_start");
  await unmount();
  backend.avoid = "文字";
  const again = await mount();
  expect(again.host.querySelector("details")?.open).toBe(true);
  await again.unmount();
});

it("waits for the latest style to be saved before starting, and does not start when saving fails", async () => {
  const { host, unmount } = await mount();
  await type(host.querySelector("textarea")!, "像素風");
  await act(async () => button(host, t("vnArtBegin"))!.click());
  const order = names();
  expect(order.indexOf("vn_art_style_set")).toBeGreaterThan(-1);
  expect(order.indexOf("vn_art_style_set")).toBeLessThan(order.indexOf("vn_art_start"));
  await unmount();

  backend.calls = [];
  backend.saveFails = true;
  const failing = await mount();
  await type(failing.host.querySelector("textarea")!, "油畫");
  await act(async () => button(failing.host, t("vnArtBegin"))!.click());
  expect(names()).toContain("vn_art_style_set");
  expect(names()).not.toContain("vn_art_start");
  expect(failing.host.textContent).toContain(t("vnStyleSaveFailed"));
  await failing.unmount();
});

it("tells the player that a changed style only affects pictures drawn from now on", async () => {
  backend.started = true;
  backend.style = "水彩";
  const { host, unmount } = await mount();
  const style = host.querySelector("textarea")!;
  expect(style.value).toBe("水彩");
  await type(style, "像素風");
  await act(async () => style.dispatchEvent(new FocusEvent("focusout", { bubbles: true })));
  expect(host.textContent).toContain(t("vnStyleChanged"));
  expect(names().filter((name) => name === "vn_art_retry" || name === "vn_art_start")).toEqual([]);
  await unmount();
});

it("offers edit style, retry without my style and change source when a style is filled", async () => {
  Object.assign(backend, { started: true, style: "某畫家風", refused: 2 });
  const { host, unmount } = await mount();
  expect(host.textContent).toContain(t("vnArtRefused", { n: 2 }));
  await act(async () => button(host, t("vnArtEditStyle"))!.click());
  expect(document.activeElement).toBe(host.querySelector("textarea"));
  await act(async () => button(host, t("vnArtChangeSource"))!.click());
  expect(document.activeElement).toBe(host.querySelector("select"));
  // 滑鼠點的按鈕也要畫焦點框，玩家才看得出焦點跳到來源選單
  expect(host.querySelector("select")!.hasAttribute("data-focus-ring")).toBe(true);
  await act(async () => button(host, t("vnArtRetryPlain"))!.click());
  const retry = backend.calls.find((call) => call.command === "vn_art_retry");
  expect(retry?.args).toEqual({ worldId: "w", withoutStyle: true });
  expect(names()).not.toContain("vn_art_style_set");
  expect(button(host, t("vnArtRetryNow"))).toBeUndefined();
  await unmount();
});

it("offers change source and a plain retry when no style is filled", async () => {
  Object.assign(backend, { started: true, refused: 1 });
  const { host, unmount } = await mount();
  expect(button(host, t("vnArtEditStyle"))).toBeUndefined();
  expect(button(host, t("vnArtRetryPlain"))).toBeUndefined();
  expect(button(host, t("vnArtChangeSource"))).toBeDefined();
  await act(async () => button(host, t("vnArtRetryNow"))!.click());
  expect(backend.calls.find((call) => call.command === "vn_art_retry")?.args).toEqual({ worldId: "w", withoutStyle: false });
  await unmount();
});

it("title summary shows the refusal with a way to the art panel and no input box", async () => {
  Object.assign(backend, { started: true, refused: 1 });
  const onOpenGallery = vi.fn();
  const { host, unmount } = await mount({ compact: true, onOpenGallery });
  expect(host.querySelector("textarea")).toBeNull();
  expect(host.textContent).toContain(t("vnArtRefused", { n: 1 }));
  await act(async () => button(host, t("vnArtOpenGallery"))!.click());
  expect(onOpenGallery).toHaveBeenCalledOnce();
  await unmount();
});
