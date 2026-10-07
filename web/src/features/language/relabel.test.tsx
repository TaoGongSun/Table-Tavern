// @vitest-environment happy-dom
// 換語系時畫面上的錯誤提示跟著換：錯誤存成「怎麼說」，繪製時才翻（存檔區、匯出列）。
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeAll, describe, expect, it } from "vitest";
import { setLang } from "../../i18n";
import type { ChatController } from "../chat/useChat";
import { ExportBar } from "../chat/ChatView";
import { LOADING_RELEASE } from "../funnel/releases";
import { SavesPanel } from "../saves/SavesPanel";

beforeAll(() => {
  (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
});

let root: Root | null = null;
let host: HTMLElement;

afterEach(async () => {
  await act(async () => root?.unmount());
  root = null;
  host?.remove();
  setLang("zh-TW");
});

async function render(node: () => React.ReactNode) {
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
  await act(async () => root!.render(node()));
  // App 換語系：setLang 後整棵重繪
  return async (lang: "en" | "ja") => {
    setLang(lang);
    await act(async () => root!.render(node()));
  };
}

const alertText = () => host.querySelector('[role="alert"]')?.textContent ?? "";

describe("errors already on screen follow a language switch", () => {
  it("the saves panel's storage error", async () => {
    const switchTo = await render(() => <SavesPanel saves={null} persisted={null} release={LOADING_RELEASE} onContinue={() => {}} />);
    expect(alertText()).toContain("這個瀏覽器不讓網站存資料");
    await switchTo("en");
    expect(alertText()).toBe("This browser doesn't let sites store data, so conversations won't be saved. To keep one, press “Export save” during the conversation.");
  });

  it("the export bar's failed-export error", async () => {
    const chat = {
      busy: false,
      exportSave: () => {
        throw new Error("boom");
      },
    } as unknown as ChatController;
    const switchTo = await render(() => <ExportBar chat={chat} name="瑟拉" release={LOADING_RELEASE} />);
    await act(async () => host.querySelector("button")!.click());
    expect(alertText()).toBe("這桌的資料超出存檔上限，沒辦法匯出（Error: boom）。");
    await switchTo("ja");
    expect(alertText()).toBe("この卓のデータはセーブの上限を超えているため、書き出せません（Error: boom）。");
  });
});
