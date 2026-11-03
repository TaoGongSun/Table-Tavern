// @vitest-environment happy-dom
// VN 開桌精靈與開場白面板不同時出現（包 0 限制 a）：精靈關掉後面板才出。
import { act } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import type { AppConfig } from "../../shared/contracts/backend-contracts";
import type { ImportController } from "../import/useImportController";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async () => null) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock("../lobby/GenerateTableDialog", () => ({ GenerateTableDialog: () => null }));

import { AppDialogs } from "../../views/AppDialogs";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

it("holds the opening-line panel back while the VN wizard is open", async () => {
  const host = document.createElement("div");
  document.body.appendChild(host);
  const root = createRoot(host);
  const imports = {
    choice: null,
    route: null,
    openings: ["Once upon a time"],
    expanded: null,
    transState: {},
    translations: {},
    tierModels: [],
  } as unknown as ImportController;
  const render = (wizard: boolean) =>
    act(async () =>
      root.render(
        <AppDialogs
          {...({} as Parameters<typeof AppDialogs>[0])}
          genTableOpen={false}
          settingsOpen={false}
          config={{ preferences: {} } as AppConfig}
          vnWizard={wizard && <div className="wizard" />}
          imports={imports}
        />,
      ),
    );
  await render(true);
  expect(host.querySelector(".wizard")).not.toBeNull();
  expect(document.body.textContent).not.toContain("Once upon a time");
  await render(false);
  expect(document.body.textContent).toContain("Once upon a time");
  act(() => root.unmount());
  host.remove();
});
