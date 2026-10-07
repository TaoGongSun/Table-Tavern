// @vitest-environment happy-dom
// 下載頁：版本與各平台檔、沒有正式版退發佈頁、安裝繞過說明、功能對照表；`#download` 開關；碰到桌面版才有的功能給下載連結。
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeAll, describe, expect, it } from "vitest";
import { DesktopOnly } from "./DesktopOnly";
import { DOWNLOAD_HASH, useDownloadPage } from "./download-route";
import { DownloadPage } from "./DownloadPage";
import { t, type MsgKey } from "../../i18n";
import { FEATURE_COMPARE } from "./feature-compare";
import { fetchLatestRelease, LOADING_RELEASE, NO_RELEASE, RELEASE_UNAVAILABLE, type ReleaseInfo } from "./releases";

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
});

const links = () => [...host.querySelectorAll("a")].map((a) => ({ text: a.textContent, href: a.getAttribute("href"), rel: a.getAttribute("rel") }));
const targets = () => [...host.querySelectorAll("a")].map((a) => a.getAttribute("target"));

const RELEASE: ReleaseInfo = {
  status: "ok",
  version: "0.3.0",
  pageUrl: "https://github.com/TaoGongSun/Table-Tavern/releases/tag/v0.3.0",
  windows: "https://github.com/TaoGongSun/Table-Tavern/releases/download/v0.3.0/TableTavern_0.3.0_x64-setup.exe",
  mac: "https://github.com/TaoGongSun/Table-Tavern/releases/download/v0.3.0/TableTavern_0.3.0_aarch64.dmg",
};

describe("the download page", () => {
  it("with a release: the version, both platform files and the release page, all opening outside the app", async () => {
    await render(<DownloadPage release={RELEASE} onClose={() => {}} />);
    expect(host.querySelector("[data-testid=download-version]")?.textContent).toBe("目前版本 v0.3.0");
    expect(links()).toEqual([
      { text: "下載 Windows 版", href: RELEASE.windows, rel: "noopener noreferrer" },
      { text: "下載 macOS 版（Apple Silicon）", href: RELEASE.mac, rel: "noopener noreferrer" },
      { text: "所有版本與檔案", href: RELEASE.pageUrl, rel: "noopener noreferrer" },
    ]);
    expect(targets()).toEqual(["_blank", "_blank", "_blank"]);
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
      const only = links();
      expect(only, label).toHaveLength(1);
      expect(only[0].href, label).toMatch(/^https:\/\/github\.com\/TaoGongSun\/Table-Tavern\/releases/);
      expect(targets(), label).toEqual(["_blank"]);
      await act(async () => root?.unmount());
      root = null;
      host.remove();
    }
  });

  it("without a release (the API said 404): says so and only links the releases page", async () => {
    await render(<DownloadPage release={NO_RELEASE} onClose={() => {}} />);
    expect(host.querySelector("[data-testid=download-version]")?.textContent).toContain("還沒有正式版");
    expect(links().map((link) => link.href)).toEqual([NO_RELEASE.pageUrl]);
  });

  it("when the lookup failed or is still running it does not claim there is no release", async () => {
    await render(<DownloadPage release={RELEASE_UNAVAILABLE} onClose={() => {}} />);
    const version = () => host.querySelector("[data-testid=download-version]")?.textContent ?? "";
    expect(version()).toBe("版本資訊暫時無法取得，請到發佈頁查看。");
    expect(links().map((link) => link.href)).toEqual([RELEASE_UNAVAILABLE.pageUrl]);
    await act(async () => root!.render(<DownloadPage release={LOADING_RELEASE} onClose={() => {}} />));
    expect(version()).toBe("正在查詢最新版本…");
    expect(host.textContent).not.toContain("還沒有正式版");
  });

  it("a release missing one platform's file shows no button for it, only the files that exist", async () => {
    await render(<DownloadPage release={{ ...RELEASE, mac: null }} onClose={() => {}} />);
    expect(links().map((link) => link.text)).toEqual(["下載 Windows 版", "所有版本與檔案"]);
  });

  it("explains getting past SmartScreen (no signing certificate) and Gatekeeper (not notarized) without a terminal or a safety promise", async () => {
    await render(<DownloadPage release={NO_RELEASE} onClose={() => {}} />);
    const text = host.textContent ?? "";
    expect(text).toMatch(/簽章憑證[^。]*SmartScreen[^。]*仍要執行/);
    expect(text).toMatch(/公證[^。]*Gatekeeper[^。]*仍要打開/);
    expect(text).not.toMatch(/xattr|終端機|Terminal|中毒|不是病毒|安全無虞/);
    expect(host.querySelectorAll("[data-testid=feature-compare] tbody tr")).toHaveLength(FEATURE_COMPARE.length);
  });

  it("the comparison table lists everything plan 1.3 keeps out of the web version as desktop-only", () => {
    // 計畫 1.3「不進、改在下載頁功能對照表列出」，逐項對到對照表的列
    const planOut: [string, MsgKey][] = [
      ["CLI 訂閱", "compare_ai"],
      ["一句話開桌", "compare_oneLine"],
      ["狀態欄／機制", "compare_status"],
      ["多角色", "compare_multi"],
      ["GM", "compare_multi"],
      ["編輯器", "compare_editor"],
      ["生圖", "compare_images"],
      ["畫廊", "compare_images"],
      ["贊助", "compare_sponsor"],
      ["重構", "compare_refactor"],
      ["用量頁", "compare_usage"],
    ];
    for (const [item, key] of planOut) {
      const row = FEATURE_COMPARE.find((candidate) => candidate.feature === key);
      expect(row, item).toBeDefined();
      if (key === "compare_ai") {
        // 兩邊都有 AI 來源，但訂閱只寫在桌面版那格
        expect(t(row!.desktop as MsgKey)).toContain("訂閱");
        expect(t(row!.web as MsgKey)).not.toContain("訂閱");
      } else {
        expect([item, row!.web, row!.desktop]).toEqual([item, false, true]);
      }
    }
  });
});

describe("opening the download page by its address", () => {
  let state: ReturnType<typeof useDownloadPage>;
  function Probe() {
    state = useDownloadPage();
    return null;
  }

  it("opens on #download (shared link or a click), and closing clears the address", async () => {
    window.history.replaceState(null, "", `/?keep=1${DOWNLOAD_HASH}`);
    await render(<Probe />);
    expect(state.open).toBe(true);
    await act(async () => state.close());
    expect(state.open).toBe(false);
    expect(window.location.hash).toBe("");
    expect(window.location.search).toBe("?keep=1");
    await act(async () => {
      window.location.hash = DOWNLOAD_HASH;
      window.dispatchEvent(new HashChangeEvent("hashchange"));
    });
    expect(state.open).toBe(true);
  });

  it("a desktop-only hint links to the download page", async () => {
    await render(<DesktopOnly />);
    expect(links()).toEqual([{ text: "下載桌面版", href: DOWNLOAD_HASH, rel: null }]);
  });
});
