// @vitest-environment happy-dom
// 精靈 ③④：沒有角色時 ③ 可直接下一步；一進 ④ 就排首幕背景；按開演才排入背景補表情，③ 挑選期間不排。
import { act } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import { t } from "../../i18n";

const calls = vi.hoisted(() => [] as string[]);
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (command: string) => {
    calls.push(command);
    return ["list_characters", "vn_backgrounds_fill", "vn_backgrounds_status"].includes(command) ? [] : null;
  }),
}));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock("../lobby/GenerateTableDialog", () => ({ GenerateTableDialog: () => null }));

import { VnWizard } from "./VnWizard";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const button = (label: string) =>
  [...document.querySelectorAll("button")].find((b) => b.textContent === label)!;

it("queues the first background on entering ④ and expression fills only when the show starts", async () => {
  const host = document.createElement("div");
  document.body.appendChild(host);
  const root = createRoot(host);
  const onFinish = vi.fn();
  await act(async () =>
    root.render(
      <VnWizard
        start={{ worldId: "W" }}
        onCreated={async () => {}}
        onImportCard={() => {}}
        onImportWork={() => {}}
        onFinish={onFinish}
        onClose={() => {}}
      />,
    ),
  );
  await act(async () => button(t("vnNext")).click());
  expect(document.body.textContent).toContain(t("vnStepSpritesTitle"));
  expect(calls).not.toContain("vn_backgrounds_fill");
  await act(async () => button(t("vnNext")).click());
  expect(calls).toContain("vn_backgrounds_fill");
  expect(calls).not.toContain("vn_sprites_fill");
  await act(async () => button(t("vnStart")).click());
  expect(calls).toContain("vn_sprites_fill");
  expect(onFinish).toHaveBeenCalledWith("W");
  act(() => root.unmount());
  host.remove();
});
