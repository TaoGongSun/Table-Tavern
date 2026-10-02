// @vitest-environment happy-dom

import { act, useState } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { t } from "../../i18n";
import type { AppConfig } from "../../shared/contracts/backend-contracts";

const backend = vi.hoisted(() => ({
  update: null as null | ((patch: Record<string, unknown>) => Promise<unknown>),
  updateCalls: 0,
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string, args?: { patch?: Record<string, unknown> }) => {
    switch (cmd) {
      case "detect_clis":
        return [{ id: "claude", path: "/bin/claude", version: "1.0.0" }];
      case "update_config":
        backend.updateCalls += 1;
        return backend.update!(args?.patch ?? {});
      case "smart_free_recommendations":
        return { limited: [], stable: [] };
      case "smart_free_status":
        return { model: "", freeDaily: null };
      case "usage_report":
        return { worlds: [] };
      case "cli_verified":
        return false;
      default:
        return null;
    }
  }),
}));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ confirm: vi.fn() }));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn() }));

import { confirm } from "@tauri-apps/plugin-dialog";
import { SettingsWindow, type SettingsTab } from "../../views/SettingsWindow";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const confirmMock = vi.mocked(confirm);

const API_CONFIG: AppConfig = {
  api_keys: {},
  tier_models: {},
  preferences: { transport: "api", language: "zh-TW" },
};

// 已同意風險、選了 claude 的設定：存檔成功會冒出 CLI 權限提示（還沒看過）
const CLAUDE_CONFIG: AppConfig = {
  api_keys: {},
  tier_models: {},
  preferences: { transport: "claude", cli_risk_accepted: true, language: "zh-TW" },
};

type HarnessProps = {
  initialConfig: AppConfig;
  initialTab?: SettingsTab;
  requestKey: number;
  onClose: () => void;
  onPreference: (key: string, value: unknown) => void;
};

function Harness({ initialConfig, initialTab, requestKey, onClose, onPreference }: HarnessProps) {
  const [config, setConfig] = useState(initialConfig);
  return (
    <SettingsWindow
      config={config}
      onSaved={setConfig}
      onPreference={(key, value) => {
        onPreference(key, value);
        setConfig((current) => ({
          ...current,
          preferences: { ...current.preferences, [key]: value },
        }));
      }}
      sponsorUnlocked={false}
      onSponsorUnlocked={() => {}}
      onClose={onClose}
      initialTab={initialTab}
      requestKey={requestKey}
      currentWorld=""
      versionTab={<div className="version-stub">versions</div>}
    />
  );
}

describe("SettingsWindow", () => {
  let root: Root | null = null;
  let host: HTMLDivElement | null = null;
  let props: HarnessProps;
  let answers: ((value: boolean) => void)[] = [];
  const onClose = vi.fn();
  const onPreference = vi.fn();

  beforeEach(() => {
    answers = [];
    onClose.mockReset();
    onPreference.mockReset();
    confirmMock.mockReset();
    confirmMock.mockImplementation(() => new Promise<boolean>((resolve) => answers.push(resolve)));
    backend.updateCalls = 0;
    backend.update = async (patch) => ({
      ...API_CONFIG,
      preferences: { ...API_CONFIG.preferences, ...(patch.preferences as object) },
    });
  });

  afterEach(() => {
    act(() => {
      root?.unmount();
    });
    host?.remove();
    root = null;
    host = null;
    document.body.innerHTML = "";
  });

  async function flush() {
    await act(async () => {
      for (let i = 0; i < 10; i += 1) await Promise.resolve();
    });
  }

  async function render(next: Partial<HarnessProps> = {}) {
    props = { ...props, ...next };
    if (!root) {
      host = document.createElement("div");
      document.body.appendChild(host);
      root = createRoot(host);
    }
    await act(async () => {
      root?.render(<Harness {...props} />);
    });
    await flush();
  }

  async function mount(initialConfig: AppConfig = API_CONFIG, initialTab?: SettingsTab) {
    props = { initialConfig, initialTab, requestKey: 0, onClose, onPreference };
    await render();
  }

  async function answer(value: boolean) {
    const resolve = answers.shift();
    expect(resolve).toBeDefined();
    await act(async () => {
      resolve!(value);
    });
    await flush();
  }

  async function click(element: Element) {
    await act(async () => {
      (element as HTMLElement).click();
    });
    await flush();
  }

  async function press(target: EventTarget, key: string) {
    await act(async () => {
      target.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true }));
    });
    await flush();
  }

  async function typeInto(input: HTMLInputElement | HTMLSelectElement, value: string) {
    const proto =
      input instanceof HTMLSelectElement ? HTMLSelectElement.prototype : HTMLInputElement.prototype;
    await act(async () => {
      Object.getOwnPropertyDescriptor(proto, "value")!.set!.call(input, value);
      input.dispatchEvent(
        new Event(input instanceof HTMLSelectElement ? "change" : "input", { bubbles: true }),
      );
    });
    await flush();
  }

  async function submit() {
    await act(async () => {
      form().dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    });
    await flush();
  }

  const tabs = () => [...document.querySelectorAll<HTMLButtonElement>('[role="tab"]')];
  const tabNamed = (key: Parameters<typeof t>[0]) => tabs().find((b) => b.textContent === t(key))!;
  const selected = () => document.querySelector('[role="tab"][aria-selected="true"]')!.textContent;
  const closeButton = () => document.querySelector<HTMLButtonElement>(".settings-close")!;
  const footer = () => document.querySelector(".settings-foot");
  const form = () => document.querySelector<HTMLFormElement>("#ai-settings-form")!;
  const roundInput = () => document.querySelector<HTMLInputElement>('input[type="number"]')!;
  const saveButton = () => document.querySelector<HTMLButtonElement>(".settings-save")!;
  const backButton = () => document.querySelector<HTMLButtonElement>(".settings-back")!;
  const visibleBackLabel = () =>
    backButton().querySelector('.settings-back-labels > [aria-hidden="false"]')!.textContent;

  async function openDirtyAi(config: AppConfig = API_CONFIG) {
    await mount(config);
    await click(tabNamed("aiTab"));
    expect(selected()).toBe(t("aiTab"));
    await typeInto(roundInput(), "5");
    expect(document.querySelector(".settings-unsaved")!.textContent).toBe(
      t("unsavedChanges", { n: 1 }),
    );
  }

  it("exposes a labelled tablist with roving focus driven by arrows, Home and End", async () => {
    await mount();
    const list = document.querySelector('[role="tablist"]')!;
    const title = document.getElementById(list.getAttribute("aria-labelledby")!)!;
    expect(title.tagName).toBe("H2");
    expect(title.textContent).toBe(t("settingsBtn"));
    expect(list.contains(closeButton())).toBe(false);
    expect(tabs()).toHaveLength(5);
    const panel = document.querySelector('[role="tabpanel"]')!;
    for (const tab of tabs()) expect(tab.getAttribute("aria-controls")).toBe(panel.id);
    expect(tabs().map((tab) => tab.tabIndex)).toEqual([0, -1, -1, -1, -1]);
    expect(panel.getAttribute("aria-labelledby")).toBe(tabs()[0].id);

    tabs()[0].focus();
    await press(tabs()[0], "ArrowRight");
    expect(document.activeElement).toBe(tabs()[1]);
    // 方向鍵只移焦點，不切頁
    expect(selected()).toBe(t("appearanceTab"));
    await press(tabs()[1], "End");
    expect(document.activeElement).toBe(tabs()[4]);
    await press(tabs()[4], "ArrowRight");
    expect(document.activeElement).toBe(tabs()[0]);
    await press(tabs()[0], "ArrowLeft");
    expect(document.activeElement).toBe(tabs()[4]);
    await press(tabs()[4], "Home");
    expect(document.activeElement).toBe(tabs()[0]);
    expect(confirmMock).not.toHaveBeenCalled();
  });

  it("clicking the current tab never asks, even with unsaved changes", async () => {
    await openDirtyAi();
    await click(tabNamed("aiTab"));
    expect(confirmMock).not.toHaveBeenCalled();
    expect(roundInput().value).toBe("5");
  });

  it("switching away with unsaved changes asks; cancel keeps the tab and the draft", async () => {
    await openDirtyAi();
    await click(tabNamed("usageTab"));
    expect(confirmMock).toHaveBeenCalledTimes(1);
    await answer(false);
    expect(selected()).toBe(t("aiTab"));
    expect(roundInput().value).toBe("5");

    await click(tabNamed("usageTab"));
    await answer(true);
    expect(selected()).toBe(t("usageTab"));
  });

  it("an external tab request with unsaved changes asks first", async () => {
    await openDirtyAi();
    await render({ initialTab: "versions", requestKey: 1 });
    expect(confirmMock).toHaveBeenCalledTimes(1);
    expect(selected()).toBe(t("aiTab"));
    await answer(true);
    expect(selected()).toBe(t("versionsTab"));
  });

  it("only the AI tab has the save bar", async () => {
    await mount();
    for (const key of ["appearanceTab", "usageTab", "versionsTab", "authorTab"] as const) {
      await click(tabNamed(key));
      expect(footer()).toBeNull();
    }
    await click(tabNamed("aiTab"));
    expect(footer()).not.toBeNull();
  });

  it("clean shows Back with save disabled; dirty shows Discard with save enabled", async () => {
    await mount();
    await click(tabNamed("aiTab"));
    expect(visibleBackLabel()).toBe(t("settingsBack"));
    expect(saveButton().disabled).toBe(true);
    await typeInto(roundInput(), "5");
    expect(visibleBackLabel()).toBe(t("settingsDiscard"));
    expect(saveButton().disabled).toBe(false);
  });

  it("keeps Saved after a save and drops it once the player edits again", async () => {
    await openDirtyAi();
    await submit();
    expect(backend.updateCalls).toBe(1);
    const status = document.querySelector('.settings-foot-status [role="status"]')!;
    expect(status.textContent).toBe(t("saved"));
    expect(document.querySelector(".settings-unsaved")).toBeNull();
    expect(saveButton().disabled).toBe(true);

    await typeInto(roundInput(), "6");
    expect(document.querySelector(".settings-foot-status")!.textContent).not.toContain(t("saved"));
  });

  it("ignores a second submit and every way out while saving", async () => {
    let finish: (value: unknown) => void = () => {};
    backend.update = () => new Promise((resolve) => (finish = resolve));
    await openDirtyAi();
    await act(async () => {
      form().dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
      form().dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    });
    await flush();
    expect(backend.updateCalls).toBe(1);
    expect(saveButton().disabled).toBe(true);
    expect(backButton().disabled).toBe(true);
    expect(document.querySelector<HTMLFieldSetElement>(".settings-fieldset")!.disabled).toBe(true);

    await click(tabNamed("usageTab"));
    await click(closeButton());
    await press(window, "Escape");
    await click(document.querySelector(".modal-overlay")!);
    expect(confirmMock).not.toHaveBeenCalled();
    expect(onClose).not.toHaveBeenCalled();
    expect(selected()).toBe(t("aiTab"));

    await act(async () => {
      finish({ ...API_CONFIG, preferences: { ...API_CONFIG.preferences, max_round_speakers: 5 } });
    });
    await flush();
    await click(tabNamed("usageTab"));
    expect(selected()).toBe(t("usageTab"));
  });

  it("a failed save keeps the draft and shows the error as an alert", async () => {
    backend.update = async () => {
      throw new Error("disk full");
    };
    await openDirtyAi();
    await submit();
    expect(document.querySelector('.settings-foot-status [role="alert"]')!.textContent).toContain(
      "disk full",
    );
    expect(roundInput().value).toBe("5");
    expect(saveButton().disabled).toBe(false);
    expect(document.querySelector(".settings-unsaved")).not.toBeNull();
  });

  it("has the close button on every tab, and it asks when there are unsaved changes", async () => {
    await mount();
    const first = closeButton();
    expect(first.getAttribute("aria-label")).toBe(t("closeBtn"));
    for (const key of ["aiTab", "usageTab", "versionsTab", "authorTab", "appearanceTab"] as const) {
      await click(tabNamed(key));
      expect(closeButton()).toBe(first);
    }
    await click(tabNamed("aiTab"));
    await typeInto(roundInput(), "5");
    await click(closeButton());
    expect(confirmMock).toHaveBeenCalledTimes(1);
    expect(onClose).not.toHaveBeenCalled();
    await answer(true);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("the back button never submits: clean closes, dirty asks", async () => {
    await mount();
    await click(tabNamed("aiTab"));
    await click(backButton());
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(backend.updateCalls).toBe(0);

    onClose.mockReset();
    await typeInto(roundInput(), "5");
    await click(backButton());
    expect(confirmMock).toHaveBeenCalledTimes(1);
    expect(backend.updateCalls).toBe(0);
    await answer(false);
    expect(onClose).not.toHaveBeenCalled();
  });

  it("nothing leaves while the CLI notice is open; a held external request runs after", async () => {
    backend.update = async (patch) => ({
      ...CLAUDE_CONFIG,
      preferences: { ...CLAUDE_CONFIG.preferences, ...(patch.preferences as object) },
    });
    await openDirtyAi(CLAUDE_CONFIG);
    await submit();
    const notice = document.querySelector(`[aria-label="${t("cliPermissionTitle")}"]`);
    expect(notice).not.toBeNull();
    // 提示畫在停用的 fieldset 外，按得到
    expect(notice!.closest("fieldset")).toBeNull();

    await press(window, "Escape");
    await click(closeButton());
    await click(tabNamed("usageTab"));
    await render({ initialTab: "versions", requestKey: 1 });
    expect(onClose).not.toHaveBeenCalled();
    expect(selected()).toBe(t("aiTab"));
    expect(confirmMock).not.toHaveBeenCalled();

    await click(notice!.querySelector("button")!);
    expect(document.querySelector(`[aria-label="${t("cliPermissionTitle")}"]`)).toBeNull();
    expect(selected()).toBe(t("versionsTab"));
    expect(onClose).not.toHaveBeenCalled();
  });

  async function openNotice() {
    backend.update = async (patch) => ({
      ...CLAUDE_CONFIG,
      preferences: { ...CLAUDE_CONFIG.preferences, ...(patch.preferences as object) },
    });
    await openDirtyAi(CLAUDE_CONFIG);
    await submit();
    expect(document.querySelector(`[aria-label="${t("cliPermissionTitle")}"]`)).not.toBeNull();
  }

  it("refuses to submit and disables the fields while the CLI notice is open", async () => {
    await openNotice();
    expect(backend.updateCalls).toBe(1);
    expect(document.querySelector<HTMLFieldSetElement>(".settings-fieldset")!.disabled).toBe(true);
    expect(document.querySelector(".settings-scroll")!.hasAttribute("inert")).toBe(true);
    expect(footer()!.hasAttribute("inert")).toBe(true);
    await submit();
    expect(backend.updateCalls).toBe(1);
  });

  it("a save attempt that would fail while the notice is open never lifts the block", async () => {
    await openNotice();
    backend.update = async () => {
      throw new Error("disk full");
    };
    await submit();
    expect(backend.updateCalls).toBe(1);
    await click(closeButton());
    await press(window, "Escape");
    expect(confirmMock).not.toHaveBeenCalled();
    expect(onClose).not.toHaveBeenCalled();
    expect(document.querySelector(`[aria-label="${t("cliPermissionTitle")}"]`)).not.toBeNull();
  });

  it("a confirm dialog that fails to open counts as cancel and does not lock later exits", async () => {
    await openDirtyAi();
    confirmMock.mockImplementationOnce(() => Promise.reject(new Error("no dialog")));
    await click(closeButton());
    expect(onClose).not.toHaveBeenCalled();
    expect(selected()).toBe(t("aiTab"));
    await click(closeButton());
    expect(confirmMock).toHaveBeenCalledTimes(2);
    await answer(true);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("an unanswered confirm is never stacked by another tab click or close", async () => {
    await openDirtyAi();
    await click(tabNamed("usageTab"));
    await click(tabNamed("authorTab"));
    await click(closeButton());
    await press(window, "Escape");
    await render({ initialTab: "versions", requestKey: 1 });
    expect(confirmMock).toHaveBeenCalledTimes(1);
    await answer(false);
    // 外部請求在確認窗關掉後才處理，仍有修改所以再問一次
    expect(confirmMock).toHaveBeenCalledTimes(2);
    await answer(false);
    expect(selected()).toBe(t("aiTab"));
    expect(onClose).not.toHaveBeenCalled();
  });

  it("a newer external request during an open confirm asks again when still dirty", async () => {
    await openDirtyAi();
    await render({ initialTab: "versions", requestKey: 1 });
    expect(confirmMock).toHaveBeenCalledTimes(1);
    await render({ initialTab: "appearance", requestKey: 2 });
    expect(confirmMock).toHaveBeenCalledTimes(1);
    await answer(false);
    expect(confirmMock).toHaveBeenCalledTimes(2);
    await answer(true);
    expect(selected()).toBe(t("appearanceTab"));
  });

  it("a newer external request during an open confirm switches directly once clean", async () => {
    await openDirtyAi();
    await render({ initialTab: "versions", requestKey: 1 });
    await render({ initialTab: "appearance", requestKey: 2 });
    await answer(true);
    // 第一筆切到版本頁、草稿捨棄；最新那筆乾淨，直接切
    expect(confirmMock).toHaveBeenCalledTimes(1);
    expect(selected()).toBe(t("appearanceTab"));
  });

  it("moves focus to the new tab when an external switch unmounts the focused field", async () => {
    await mount(API_CONFIG, "ai");
    roundInput().focus();
    expect(document.activeElement).toBe(roundInput());
    await render({ initialTab: "versions", requestKey: 1 });
    expect(selected()).toBe(t("versionsTab"));
    expect(document.activeElement).toBe(tabNamed("versionsTab"));
  });

  it("api format saves instantly and survives discarding the other drafts", async () => {
    await openDirtyAi();
    const selects = [...document.querySelectorAll<HTMLSelectElement>(".settings-scroll select")];
    const select = selects.find((element) =>
      [...element.options].some((option) => option.value === "responses"),
    )!;
    await typeInto(select, "responses");
    expect(onPreference).toHaveBeenCalledWith("api_mode", "responses");
    expect(document.querySelector(".settings-unsaved")!.textContent).toBe(
      t("unsavedChanges", { n: 1 }),
    );
    await click(backButton());
    await answer(true);
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(select.value).toBe("responses");
  });
});
