// @vitest-environment happy-dom
// 下載頁：四扇門與單一詳情面板、版本與上下兩組下載鈕、截圖選圖與缺圖不畫、安裝繞過說明、功能對照表；
// 覆蓋層行為（鎖捲動、Esc、焦點）；`#download` 與 `#download/<桌>` 開關。
import { act } from "react";
import { version as DESKTOP_VERSION } from "../../../../package.json";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { DesktopOnly } from "./DesktopOnly";
import { DOWNLOAD_HASH, downloadHash, parseDownloadTable, useDownloadPage, type DownloadTable } from "./download-route";
import { DownloadPage } from "./DownloadPage";
import { setLang, t, type MsgKey } from "../../i18n";
import { PCLOUD_PAGE } from "../../shared/endpoints/origins";
import { FEATURE_COMPARE } from "./feature-compare";
import { fetchLatestRelease, LOADING_RELEASE, NO_RELEASE, RELEASE_UNAVAILABLE, type ReleaseInfo } from "./releases";
import { DOWNLOAD_DOORS, SHARED_FEATURES, TOUR_HERO } from "./tour-features";
import { pickGuideImage, pickTourImage, shotSize, TOUR_IMAGES, type TourImages } from "./tour-images";

beforeAll(() => {
  (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
});

let root: Root | null = null;
let host: HTMLElement;

async function render(node: React.ReactNode) {
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
  await act(async () => root!.render(node));
}

afterEach(async () => {
  await act(async () => root?.unmount());
  root = null;
  host?.remove();
  window.history.replaceState(null, "", "/");
  setLang("zh-TW");
  vi.unstubAllGlobals();
});

const describeLinks = (scope: ParentNode) => [...scope.querySelectorAll("a")].map((a) => ({ text: a.textContent, href: a.getAttribute("href"), rel: a.getAttribute("rel") }));
const links = () => describeLinks(host);
const groups = () => ["download-cta", "download-cta-bottom"].map((id) => describeLinks(host.querySelector(`[data-testid=${id}]`)!));
const doorHrefs = () => [...host.querySelectorAll(".tour-door")].map((door) => door.getAttribute("href"));
const DOOR_HASHES = ["new", "oneLine", "room", "vn"].map((id) => downloadHash(id as DownloadTable));

const RELEASE: ReleaseInfo = {
  status: "ok",
  version: "0.3.0",
  pageUrl: "https://github.com/TaoGongSun/Table-Tavern/releases/tag/v0.3.0",
  windows: "https://github.com/TaoGongSun/Table-Tavern/releases/download/v0.3.0/TableTavern_0.3.0_x64-setup.exe",
  mac: "https://github.com/TaoGongSun/Table-Tavern/releases/download/v0.3.0/TableTavern_0.3.0_aarch64.dmg",
};

const TOUR_KEYS = ["lobby", "table", "status", "cardUi", "oneLine", "oneLineOutline", "vnPlay", "vnWizard"];
const GUIDE_KEYS = ["room-04", "room-15"];
const fakeTour = (langs: string[]): TourImages => Object.fromEntries(TOUR_KEYS.flatMap((key) => langs.map((lang) => [`./tour/${key}.${lang}.webp`, `/assets/${key}.${lang}.webp`])));
const fakeGuide = (langs: string[]): TourImages => Object.fromEntries(GUIDE_KEYS.flatMap((key) => langs.map((lang) => [`./guide/shots/${key}.${lang}.webp`, `/assets/${key}.${lang}.webp`])));

async function show(hash: string) {
  await act(async () => {
    window.location.hash = hash;
    window.dispatchEvent(new HashChangeEvent("hashchange"));
  });
}

describe("the download page", () => {
  const platforms = [
    { text: "下載 Windows 版", href: RELEASE.windows, rel: "noopener noreferrer" },
    { text: "下載 macOS 版（Apple Silicon）", href: RELEASE.mac, rel: "noopener noreferrer" },
    { text: "所有版本與檔案", href: RELEASE.pageUrl, rel: "noopener noreferrer" },
  ];

  it("with a release: the version once, both download groups give both platform files, and the four doors stay on this page", async () => {
    await render(<DownloadPage release={RELEASE} onClose={() => {}} />);
    expect(host.querySelector("[data-testid=download-version]")?.textContent).toBe("目前版本 v0.3.0");
    expect(groups()).toEqual([platforms, platforms]);
    expect(doorHrefs()).toEqual(DOOR_HASHES);
    expect(links().filter((link) => !link.href?.startsWith("#"))).toEqual([...platforms, ...platforms]);
    expect([...host.querySelectorAll("[data-testid^=download-cta] a")].map((a) => a.getAttribute("target"))).toEqual(Array(6).fill("_blank"));
  });

  it("simplified Chinese: only pCloud, no GitHub links; the version line is the desktop build's version", async () => {
    setLang("zh-CN");
    await render(<DownloadPage release={RELEASE} onClose={() => {}} />);
    const pcloud = { text: "从 pCloud 下载（国内可用）", href: PCLOUD_PAGE, rel: "noopener noreferrer" };
    expect(groups()).toEqual([[pcloud], [pcloud]]);
    expect(host.querySelector("[data-testid=download-cta] [data-testid=room-guide-link]")).toBeNull();
    expect(host.querySelector("[data-testid=download-version]")?.textContent).toBe(`当前版本 v${DESKTOP_VERSION}`);
    expect(host.querySelector("h1")?.textContent).toBe(t("tourTitle"));
    expect(host.querySelector(".tour-lead")?.textContent).toContain("硅基流动");
    expect(host.querySelector(".tour-lead")?.textContent).not.toMatch(/OpenRouter|Claude|ChatGPT/);
  });

  it("China provider mode: only pCloud, no GitHub links; the version comes from the build, not GitHub", async () => {
    await render(<DownloadPage release={LOADING_RELEASE} cn onClose={() => {}} />);
    expect(groups()).toEqual(Array(2).fill([{ text: "從 pCloud 下載（中國大陸可用）", href: PCLOUD_PAGE, rel: "noopener noreferrer" }]));
    expect(host.querySelector("[data-testid=download-version]")?.textContent).toBe(`目前版本 v${DESKTOP_VERSION}`);
  });

  it("rate limited, offline, or a release without matching installers: no broken link and never 'no release yet'", async () => {
    const bodies: [string, typeof fetch][] = [
      ["429", (async () => new Response("{}", { status: 429 })) as typeof fetch],
      [
        "offline",
        (async () => {
          throw new TypeError("Load failed");
        }) as typeof fetch,
      ],
      [
        "no matching installers",
        (async () =>
          new Response(
            JSON.stringify({
              tag_name: "v0.4.0",
              html_url: "https://github.com/TaoGongSun/Table-Tavern/releases/tag/v0.4.0",
              assets: [
                { name: "TableTavern_0.4.0_x64.msi", browser_download_url: "https://github.com/x/win.msi" },
                { name: "TableTavern_0.4.0_aarch64.dmg", browser_download_url: "https://evil.example/mac.dmg" },
              ],
            }),
            { status: 200 },
          )) as typeof fetch,
      ],
    ];
    for (const [label, fetchImpl] of bodies) {
      const release = await fetchLatestRelease(fetchImpl);
      await render(<DownloadPage release={release} onClose={() => {}} />);
      expect(host.textContent, label).not.toContain("還沒有正式版");
      for (const group of groups()) {
        expect(group, label).toHaveLength(1);
        expect(group[0].href, label).toMatch(/^https:\/\/github\.com\/TaoGongSun\/Table-Tavern\/releases/);
      }
      expect(doorHrefs(), label).toEqual(DOOR_HASHES);
      await act(async () => root?.unmount());
      root = null;
      host.remove();
    }
  });

  it("without a release (the API said 404): says so and the download buttons only link the releases page", async () => {
    await render(<DownloadPage release={NO_RELEASE} onClose={() => {}} />);
    expect(host.querySelector("[data-testid=download-version]")?.textContent).toContain("還沒有正式版");
    expect(groups().map((group) => group.map((link) => link.href))).toEqual([[NO_RELEASE.pageUrl], [NO_RELEASE.pageUrl]]);
  });

  it("when the lookup failed or is still running it does not claim there is no release", async () => {
    await render(<DownloadPage release={RELEASE_UNAVAILABLE} onClose={() => {}} />);
    const version = () => host.querySelector("[data-testid=download-version]")?.textContent ?? "";
    expect(version()).toBe("版本資訊暫時無法取得，請到發佈頁查看。");
    expect(groups().map((group) => group.map((link) => link.href))).toEqual([[RELEASE_UNAVAILABLE.pageUrl], [RELEASE_UNAVAILABLE.pageUrl]]);
    await act(async () => root!.render(<DownloadPage release={LOADING_RELEASE} onClose={() => {}} />));
    expect(version()).toBe("正在查詢最新版本…");
    expect(host.textContent).not.toContain("還沒有正式版");
  });

  it("a release missing one platform's file shows no button for it, only the files that exist", async () => {
    await render(<DownloadPage release={{ ...RELEASE, mac: null }} onClose={() => {}} />);
    expect(groups().map((group) => group.map((link) => link.text))).toEqual([
      ["下載 Windows 版", "所有版本與檔案"],
      ["下載 Windows 版", "所有版本與檔案"],
    ]);
  });

  it("explains getting past SmartScreen and Gatekeeper without a terminal or a safety promise", async () => {
    await render(<DownloadPage release={NO_RELEASE} onClose={() => {}} />);
    const text = host.textContent ?? "";
    expect(text).toMatch(/簽章憑證[^。]*SmartScreen[^。]*仍要執行/);
    expect(text).toMatch(/公證[^。]*Gatekeeper[^。]*仍要打開/);
    expect(text).not.toMatch(/xattr|終端機|Terminal|中毒|不是病毒|安全無虞/);
    expect(host.querySelectorAll("[data-testid=feature-compare] tbody tr")).toHaveLength(FEATURE_COMPARE.length);
  });

  it("keeps shared rows in the comparison table and moves each table's own row onto that door", () => {
    const features = new Set<string>(FEATURE_COMPARE.map((row) => row.feature));
    for (const key of ["compare_multi", "compare_room", "compare_vn", "compare_oneLine", "compare_status"]) {
      expect(features.has(key)).toBe(false);
    }
    for (const key of ["compare_editor", "compare_images", "compare_usage", "compare_sponsor", "compare_refactor", "compare_scenes"] as const) {
      const row = FEATURE_COMPARE.find((candidate) => candidate.feature === key);
      expect([key, row?.web, row?.desktop]).toEqual([key, false, true]);
    }
    const ai = FEATURE_COMPARE.find((row) => row.feature === "compare_ai");
    expect(t(ai!.desktop as MsgKey)).toContain("訂閱");
    expect(t(ai!.web as MsgKey)).not.toContain("訂閱");
  });
});

describe("the four doors", () => {
  it("uses the lobby names, says which way the web already has, and opens on 開新的一桌", async () => {
    await render(<DownloadPage release={NO_RELEASE} onClose={() => {}} images={{}} guideShots={{}} />);
    expect([...host.querySelectorAll(".tour-door b")].map((node) => node.textContent)).toEqual(DOWNLOAD_DOORS.map((door) => t(door.name)));
    expect([...host.querySelectorAll(".tour-door small")].map((node) => node.textContent)).toEqual([t("tourOnWeb"), t("tourDesktopOnly"), t("tourDesktopOnly"), t("tourDesktopOnly")]);
    expect(host.querySelector(".tour-door")?.textContent).toContain(`適合：${t("door_new_fit")}`);
    expect(host.querySelector("[data-testid=tour-panel]")?.getAttribute("data-table")).toBe("new");
    expect(host.querySelector("[data-testid=door-new]")?.getAttribute("aria-current")).toBe("true");
    expect(host.querySelector("[data-testid=tour-web-line]")?.textContent).toBe("網頁版：有");
    expect(host.querySelector("[data-testid=tour-panel]")?.textContent).toContain("多角色同桌");
    expect(host.querySelector("[data-testid=tour-card-ui]")?.textContent).toContain(t("panel_cardUi_title"));
    expect([...host.querySelectorAll(".tour-icon-card")].map((card) => card.getAttribute("data-testid"))).toEqual(SHARED_FEATURES.map((feature) => `tour-${feature.key}`));
  });

  it("switches panel from the address without closing, and the room panel links to the guide", async () => {
    const onClose = vi.fn();
    window.location.hash = downloadHash("room");
    await render(<DownloadPage release={NO_RELEASE} onClose={onClose} images={{}} guideShots={{}} />);
    expect(host.querySelector("[data-testid=tour-panel]")?.getAttribute("data-table")).toBe("room");
    expect(host.querySelector("[data-testid=tour-web-line]")?.textContent).toBe("網頁版：沒有");
    expect(host.querySelector("[data-testid=room-guide-link]")?.getAttribute("href")).toBe("#guide/room");
    expect(host.querySelector("[data-testid=download-cta] [data-testid=room-guide-link]")).toBeNull();
    expect(host.querySelector("[data-testid=tour-card-ui]")).toBeNull();
    await show(downloadHash("vn"));
    expect(onClose).not.toHaveBeenCalled();
    expect(host.querySelector("[data-testid=tour-panel]")?.getAttribute("data-table")).toBe("vn");
    expect(host.querySelector("[data-testid=room-guide-link]")).toBeNull();
    await show("#download/nope");
    expect(host.querySelector("[data-testid=tour-panel]")?.getAttribute("data-table")).toBe("new");
    await show(downloadHash("oneLine"));
    expect(host.querySelector("[data-testid=tour-web-line]")?.textContent).toBe("網頁版：沒有");
    expect(host.querySelectorAll("[data-testid=tour-panel] li")).toHaveLength(3);
  });

  it("scrolls the panel into view on a phone when a door is clicked", async () => {
    vi.stubGlobal("matchMedia", () => ({ matches: true, addEventListener() {}, removeEventListener() {} }));
    await render(<DownloadPage release={NO_RELEASE} onClose={() => {}} images={{}} guideShots={{}} />);
    const panel = host.querySelector("#tour-panel") as HTMLElement;
    const scroll = vi.spyOn(panel, "scrollIntoView");
    await act(async () => host.querySelector<HTMLAnchorElement>("[data-testid=door-vn]")!.click());
    expect(scroll).toHaveBeenCalledWith({ behavior: "smooth", block: "start" });
  });

  it("simplified Chinese pitches SiliconFlow on the AI card and in the comparison row, not Claude or ChatGPT", async () => {
    setLang("zh-CN");
    await render(<DownloadPage release={NO_RELEASE} onClose={() => {}} images={{}} guideShots={{}} />);
    const card = host.querySelector("[data-testid=tour-subscription]")?.textContent ?? "";
    expect(card).toContain("硅基流动");
    expect(card).not.toMatch(/OpenRouter|Claude|ChatGPT/);
    const row = [...host.querySelectorAll("[data-testid=feature-compare] tbody tr")].find((tr) => tr.querySelector("th")?.textContent === t("compare_ai"));
    expect(row?.querySelectorAll("td")[1]?.textContent).toContain("硅基流动");
    expect(row?.querySelectorAll("td")[1]?.textContent).not.toMatch(/OpenRouter|Claude|ChatGPT/);
    expect([...host.querySelectorAll(".tour-door")]).toHaveLength(4);
  });
});

describe("tour shots", () => {
  it("prefers the page's own language and falls back to English, never to the other Chinese", () => {
    const images = fakeTour(["zh-TW", "zh-CN", "en"]);
    expect(pickTourImage(images, "status", "zh-TW")).toBe("/assets/status.zh-TW.webp");
    expect(pickTourImage(images, "status", "zh-CN")).toBe("/assets/status.zh-CN.webp");
    expect(pickTourImage(fakeTour(["zh-TW", "en"]), "status", "zh-CN")).toBe("/assets/status.en.webp");
    expect(pickTourImage(fakeTour(["zh-TW"]), "status", "zh-CN")).toBeNull();
    expect(pickTourImage(fakeTour(["en"]), "status", "zh-TW")).toBe("/assets/status.en.webp");
    for (const lang of ["en", "ja", "ko", "es", "pt-BR", "de", "fr", "ru"] as const) {
      expect(pickTourImage(images, "status", lang), lang).toBe("/assets/status.en.webp");
    }
    expect(pickTourImage(images, "scenes", "zh-TW")).toBeNull();
    expect(pickTourImage({}, "lobby", "en")).toBeNull();
    expect(Object.keys(TOUR_IMAGES).some((key) => /subscription|editor|roomHost|roomGuest/.test(key))).toBe(false);
    expect(pickGuideImage(fakeGuide(["zh-CN"]), "room-15", "zh-TW")).toBeNull();
    expect(pickGuideImage(fakeGuide(["en"]), "room-15", "zh-TW")).toBe("/assets/room-15.en.webp");
    expect(pickGuideImage(fakeGuide(["zh-TW", "en"]), "room-04", "zh-TW")).toBe("/assets/room-04.zh-TW.webp");
  });

  it("gives each shot its own size; only the lobby hero loads eagerly; a failed shot disappears", async () => {
    await render(<DownloadPage release={NO_RELEASE} onClose={() => {}} images={fakeTour(["zh-TW"])} guideShots={fakeGuide(["zh-TW"])} />);
    const shown = () => [...host.querySelectorAll("img")].map((img) => img.getAttribute("src"));
    expect(shown()).toEqual(["/assets/lobby.zh-TW.webp", "/assets/table.zh-TW.webp", "/assets/status.zh-TW.webp", "/assets/cardUi.zh-TW.webp"]);
    const images = [...host.querySelectorAll("img")];
    expect(images.map((img) => img.getAttribute("loading"))).toEqual([null, "lazy", "lazy", "lazy"]);
    expect(images.map((img) => [img.getAttribute("width"), img.getAttribute("height")])).toEqual([
      ["1600", "1136"],
      ["1600", "1136"],
      ["1600", "1136"],
      ["1600", "1136"],
    ]);
    expect(images.map((img) => img.getAttribute("alt"))).toEqual([t("tourTitle"), t("newTable"), t("newTable"), t("panel_cardUi_title")]);
    expect(shotSize("room-15")).toEqual({ width: 600, height: 1300 });
    expect(shotSize("room-04")).toEqual({ width: 1600, height: 855 });
    expect(TOUR_HERO).toBe("lobby");

    await act(async () => host.querySelector("[data-testid=tour-card-ui] img")!.dispatchEvent(new Event("error")));
    expect(shown()).toEqual(["/assets/lobby.zh-TW.webp", "/assets/table.zh-TW.webp", "/assets/status.zh-TW.webp"]);
    expect(host.querySelector("[data-testid=tour-card-ui]")?.textContent).toContain(t("panel_cardUi_body"));

    await show(downloadHash("room"));
    const room = [...host.querySelectorAll("#tour-panel img")];
    expect(room.map((img) => img.getAttribute("src"))).toEqual(["/assets/room-04.zh-TW.webp", "/assets/room-15.zh-TW.webp"]);
    expect(room.map((img) => [img.getAttribute("width"), img.getAttribute("height")])).toEqual([
      ["1600", "855"],
      ["600", "1300"],
    ]);
    expect(room[1].closest("figure")?.classList.contains("tour-shot-phone")).toBe(true);
  });

  it("falls back to the English shot and does not draw retired images", async () => {
    const images = {
      ...fakeTour(["en"]),
      "./tour/subscription.zh-TW.webp": "/assets/subscription.zh-TW.webp",
      "./tour/editor.en.webp": "/assets/editor.en.webp",
      "./tour/roomHost.zh-CN.webp": "/assets/roomHost.zh-CN.webp",
      "./tour/roomGuest.zh-CN.webp": "/assets/roomGuest.zh-CN.webp",
    };
    await render(<DownloadPage release={NO_RELEASE} onClose={() => {}} images={images} guideShots={{}} />);
    const srcs = [...host.querySelectorAll("img")].map((img) => img.getAttribute("src") ?? "");
    expect(srcs[0]).toBe("/assets/lobby.en.webp");
    expect(srcs.some((src) => /subscription|editor|roomHost|roomGuest/.test(src))).toBe(false);
    expect(host.querySelectorAll(".tour-icon-card")).toHaveLength(SHARED_FEATURES.length);
  });

  it("with no shots the panel and the icon row stay, and there is no image", async () => {
    await render(<DownloadPage release={NO_RELEASE} onClose={() => {}} images={{}} guideShots={{}} />);
    expect(host.querySelectorAll("img")).toHaveLength(0);
    expect(host.querySelector("[data-testid=tour-panel] h2")?.textContent).toBe(t("newTable"));
    expect(host.querySelectorAll(".tour-icon-card")).toHaveLength(4);
  });

  it("has a single close button; door addresses do not ask the page to close", async () => {
    const onClose = vi.fn();
    await render(<DownloadPage release={RELEASE} onClose={onClose} />);
    expect([...host.querySelectorAll("button")].map((button) => button.textContent)).toEqual(["回到遊戲"]);
    await show(downloadHash("oneLine"));
    expect(onClose).not.toHaveBeenCalled();
  });

  it("locks the page behind it, takes focus, closes on Esc and hands focus and scrolling back", async () => {
    document.body.style.overflow = "auto";
    const opener = document.createElement("button");
    document.body.append(opener);
    opener.focus();
    const onClose = vi.fn();
    await render(<DownloadPage release={NO_RELEASE} onClose={onClose} />);
    expect(document.body.style.overflow).toBe("hidden");
    expect(document.activeElement?.textContent).toBe("回到遊戲");
    await act(async () => document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true })));
    expect(onClose).toHaveBeenCalledTimes(1);
    await act(async () => root?.unmount());
    root = null;
    expect(document.body.style.overflow).toBe("auto");
    expect(document.activeElement).toBe(opener);
    opener.remove();
    document.body.style.overflow = "";
  });
});

describe("opening the download page by its address", () => {
  let state: ReturnType<typeof useDownloadPage>;
  function Probe() {
    state = useDownloadPage();
    return null;
  }

  it("opens on #download and on #download/<table>, and closing clears the address", async () => {
    window.history.replaceState(null, "", `/?keep=1${downloadHash("room")}`);
    await render(<Probe />);
    expect(state.open).toBe(true);
    expect(state.table).toBe("room");
    expect(parseDownloadTable("#download/nope")).toBe("new");
    await act(async () => state.close());
    expect(state.open).toBe(false);
    expect(window.location.hash).toBe("");
    expect(window.location.search).toBe("?keep=1");
    await show(DOWNLOAD_HASH);
    expect(state.open).toBe(true);
    expect(state.table).toBe("new");
    await show("#downloadfoo");
    expect(state.open).toBe(false);
    await show("#guide/room");
    expect(state.open).toBe(false);
  });

  it("a desktop-only hint links to the download page", async () => {
    await render(<DesktopOnly />);
    expect(links()).toEqual([{ text: "下載桌面版", href: DOWNLOAD_HASH, rel: null }]);
  });
});
