// @vitest-environment happy-dom
// 裁切窗的錯誤訊息走翻譯字串，不顯示英文原文、不帶「Error:」前綴。
import { act, useEffect } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async () => null) }));
vi.mock("react-easy-crop", () => ({
  // 替身：掛上就回報一個裁切範圍（真的 Cropper 在 happy-dom 量不到尺寸）
  default: ({
    onCropComplete,
  }: {
    onCropComplete: (area: unknown, pixels: { x: number; y: number; width: number; height: number }) => void;
  }) => {
    // 只在掛上時回報一次（onCropComplete 每次重繪都是新函式，跟著重跑會無限迴圈）
    // eslint-disable-next-line react-hooks/exhaustive-deps
    useEffect(() => onCropComplete({}, { x: 0, y: 0, width: 10, height: 15 }), []);
    return null;
  },
}));

import { t } from "../../i18n";
import { CropDialog } from "./CardImageDialogs";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

let root: Root | null = null;
let host: HTMLDivElement | null = null;

afterEach(() => {
  act(() => root?.unmount());
  host?.remove();
  vi.unstubAllGlobals();
  document.body.innerHTML = "";
});

it("圖片載入失敗顯示翻譯字串", async () => {
  // 圖解不開：設 src 就觸發 onerror
  vi.stubGlobal(
    "Image",
    class {
      onload: (() => void) | null = null;
      onerror: (() => void) | null = null;
      set src(_value: string) {
        setTimeout(() => this.onerror?.(), 0);
      }
    },
  );
  const onConfirm = vi.fn(async () => {});
  host = document.createElement("div");
  document.body.appendChild(host);
  root = createRoot(host);
  await act(async () =>
    root?.render(
      <CropDialog
        title="Crop"
        src="data:image/png;base64,AAAA"
        aspect={2 / 3}
        cropShape="rect"
        onConfirm={onConfirm}
        onCancel={() => {}}
      />,
    ),
  );
  const confirm = [...document.querySelectorAll<HTMLButtonElement>("button")].find(
    (button) => button.textContent === t("cropConfirm"),
  )!;
  await act(async () => {
    confirm.click();
    await new Promise((done) => setTimeout(done, 10));
  });
  expect(document.body.textContent).toContain(t("imageLoadFailed"));
  expect(document.body.textContent).not.toContain("Error:");
  expect(document.body.textContent).not.toContain("Unable to load image");
  expect(onConfirm).not.toHaveBeenCalled();
});
