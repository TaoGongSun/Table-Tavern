// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import { setLang } from "../../i18n";
import { FormatRepair } from "./FormatNotice";
import type { RepairNotice } from "./open-world";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

describe("FormatRepair", () => {
  let root: Root | null = null;
  let host: HTMLDivElement | null = null;

  afterEach(() => {
    act(() => root?.unmount());
    host?.remove();
    root = null;
    host = null;
    setLang("zh-TW");
  });

  function show(notice: Omit<RepairNotice, "directory">): string {
    host = document.createElement("div");
    document.body.append(host);
    root = createRoot(host);
    act(() =>
      root!.render(<FormatRepair notice={{ ...notice, directory: "/w" }} onOpenFolder={vi.fn()} />),
    );
    return host.querySelector("p")?.textContent ?? "";
  }

  it.each([
    ["outside", "Папки этого стола"],
    ["rename", "не удалось переименовать"],
    ["convert", "Преобразование формата не завершилось"],
    ["missing", "Основная папка этого стола не найдена"],
  ] as const)("ru 下 %s 顯示俄文說明", (reason, expected) => {
    setLang("ru");
    const text = show({ reason, error: null });
    expect(text).toContain(expected);
    expect(text).not.toMatch(/[一-鿿]/);
    expect(host?.querySelector("h2")?.textContent).toBe("Этот стол нужно починить");
  });

  it("io 帶系統錯誤原文；錯誤本身是代碼也一起翻", () => {
    setLang("ru");
    expect(show({ reason: "io", error: "Permission denied (os error 13)" })).toBe(
      "При восстановлении не удалось прочитать или записать файл. Стол, резервная копия и отложенное содержимое на месте: Permission denied (os error 13)",
    );
    act(() => root?.unmount());
    host?.remove();
    const nested = `TTMSG:${JSON.stringify({ code: "io_failed", error: "disk full" })}`;
    expect(show({ reason: "io", error: nested })).toContain(
      "на месте: Не удалось прочитать или записать файл: disk full",
    );
  });

  it("zh-TW 是正典原句", () => {
    expect(show({ reason: "convert", error: null })).toBe(
      "格式轉換沒有完成，原桌未改動。下次開啟會再試。",
    );
  });
});
