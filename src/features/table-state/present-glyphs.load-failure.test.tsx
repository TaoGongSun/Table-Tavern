// @vitest-environment happy-dom
// 轉換器載入失敗：present 一律顯示原字，不報錯、不卡住狀態列。

import { act } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import { setLang } from "../../i18n";
import { StateBar } from "./StateBar";
import { loadGlyphEngine } from "./present-glyphs";

vi.mock("opencc-js/cn2t", () => {
  throw new Error("chunk 載入失敗");
});

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

it("載入失敗保留原字", async () => {
  const store = new Map<string, string>();
  vi.stubGlobal("localStorage", {
    getItem: (key: string) => store.get(key) ?? null,
    setItem: (key: string, value: string) => void store.set(key, value),
  });
  setLang("zh-TW");
  const host = document.createElement("div");
  const root = createRoot(host);
  const noop = () => {};
  const view = (
    <StateBar
      fields={{ present: "贺铮、周掌柜" }}
      tree={{}}
      jumps={{}}
      bindings={[]}
      editing={null}
      onBeginEdit={noop}
      onChangeEditValue={noop}
      onSave={noop}
      onCancelEdit={noop}
      onMarkCounter={noop}
      onBind={noop}
      player={null}
      castCount={0}
      cast={[]}
      knownNames={["贺铮"]}
    />
  );
  act(() => root.render(view));
  await act(async () => {
    await loadGlyphEngine("tw").catch(() => {});
  });
  act(() => root.render(view));
  expect(host.querySelectorAll(".state-bar-value")[2].textContent).toBe("贺铮、周掌柜");
  act(() => root.unmount());
  vi.unstubAllGlobals();
});
