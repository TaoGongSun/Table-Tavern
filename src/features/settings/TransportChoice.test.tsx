// @vitest-environment happy-dom
// 連線清單依介面語系分流：簡中沒有 API 直連、其他語系沒有矽基流動；四個 CLI 都列。已選中的列仍列出。

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import { setLang, t } from "../../i18n";
import type { AppConfig } from "../../shared/contracts/backend-contracts";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async () => null) }));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn() }));

import { TransportChoice } from "./TransportChoice";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const mounted: { root: Root; host: HTMLDivElement }[] = [];
const config: AppConfig = { api_keys: {}, tier_models: {}, preferences: {} };

function mount(transport: string, siliconflow: boolean) {
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  mounted.push({ root, host });
  act(() =>
    root.render(
      <TransportChoice
        transport={transport}
        siliconflow={siliconflow}
        onTransport={() => {}}
        clis={[]}
        config={config}
        installingCli={null}
        installProgress={{}}
        onInstall={() => {}}
        testedPrompts={{}}
      />,
    ),
  );
  return host;
}

afterEach(() => {
  for (const { root, host } of mounted.splice(0)) {
    act(() => root.unmount());
    host.remove();
  }
  setLang("zh-TW");
});

describe("連線清單依語系分流", () => {
  it("簡中列出矽基流動與 Claude、沒有 API 直連", () => {
    setLang("zh-CN");
    const host = mount("claude", false);
    expect(host.textContent).toContain(t("transportCn"));
    expect(host.textContent).toContain("Claude");
    expect(host.textContent).not.toContain(t("transportApi"));
  });

  it("繁中有 API 直連、沒有矽基流動", () => {
    setLang("zh-TW");
    const host = mount("api", false);
    expect(host.textContent).toContain(t("transportApi"));
    expect(host.textContent).not.toContain(t("transportCn"));
  });

  it("簡中已選 API 直連時那列仍列出", () => {
    setLang("zh-CN");
    const host = mount("api", false);
    expect(host.textContent).toContain(t("transportApi"));
  });

  it("繁中已選矽基流動時那列仍列出", () => {
    setLang("zh-TW");
    const host = mount("api", true);
    expect(host.textContent).toContain(t("transportCn"));
  });
});
