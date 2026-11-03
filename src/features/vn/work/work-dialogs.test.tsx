// @vitest-environment happy-dom
// 作品包的匯出與二選一對話框（plans/vn-mode-pkg8.md §2.2、§2.3）：附設定預設不勾、變數模式停用並註明；
// 二選一帶劇透鎖選擇。
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { t } from "../../../i18n";
import type { VnWorkProbe } from "../../../shared/contracts/backend-contracts";

const backend = vi.hoisted(() => ({ calls: [] as [string, unknown][], varsMode: false }));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (command: string, args: unknown) => {
    backend.calls.push([command, args]);
    if (command === "vn_bible_read") return { cover: true, vars_mode: backend.varsMode };
    if (command === "export_vn_work") return { notices: [] };
    return null;
  }),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({
  confirm: vi.fn(async () => true),
  message: vi.fn(async () => {}),
  save: vi.fn(async () => "/out/w.ttvn"),
  open: vi.fn(async () => null),
}));
vi.mock("@tauri-apps/plugin-opener", () => ({ revealItemInDir: vi.fn(async () => {}) }));

import { ExportWorkDialog } from "./ExportWorkDialog";
import { ImportWorkDialog } from "./ImportWorkDialog";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

let host: HTMLDivElement;
let root: Root;
beforeEach(() => {
  backend.calls.length = 0;
  backend.varsMode = false;
  host = document.createElement("div");
  document.body.appendChild(host);
  root = createRoot(host);
});
afterEach(() => {
  act(() => root.unmount());
  host.remove();
});

const button = (label: string) =>
  [...document.querySelectorAll("button")].find((item) => item.textContent === label)!;
const checkbox = () => document.querySelector<HTMLInputElement>("input[type=checkbox]")!;

it("exports with settings only when the player ticks it", async () => {
  const onClose = vi.fn();
  await act(async () =>
    root.render(<ExportWorkDialog worldId="W" tableName="霧港" onClose={onClose} onError={() => {}} />),
  );
  expect(document.body.textContent).toContain(t("vnWorkExportWarn"));
  expect(checkbox().checked).toBe(false);
  await act(async () => checkbox().click());
  await act(async () => button(t("vnWorkExport")).click());
  expect(onClose).toHaveBeenCalled();
  expect(backend.calls).toContainEqual([
    "export_vn_work",
    { worldId: "W", path: "/out/w.ttvn", cover: null, settings: true },
  ]);
});

it("disables settings on a card-vars table", async () => {
  backend.varsMode = true;
  await act(async () =>
    root.render(<ExportWorkDialog worldId="W" tableName="霧港" onClose={() => {}} onError={() => {}} />),
  );
  expect(checkbox().disabled).toBe(true);
  expect(document.body.textContent).toContain(t("vnWorkSettingsVars"));
});

it("passes the chosen mode and spoiler lock", async () => {
  const probe: VnWorkProbe = {
    title: "霧港",
    lang: "zh-TW",
    chapters: 2,
    endings: 1,
    settings: true,
    vars_mode: false,
    rating: null,
  };
  const onChoose = vi.fn();
  await act(async () =>
    root.render(<ImportWorkDialog probe={probe} onChoose={onChoose} onCancel={() => {}} />),
  );
  await act(async () => checkbox().click());
  await act(async () => button(t("vnWorkAsContinue")).click());
  await act(async () => button(t("vnWorkAsReplay")).click());
  expect(onChoose.mock.calls).toEqual([
    ["continue", false],
    ["replay", false],
  ]);
});
