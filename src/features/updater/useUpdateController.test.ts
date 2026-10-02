// @vitest-environment happy-dom

import { act, createElement, useEffect } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import {
  useUpdateController,
  type CheckResult,
  type UpdateControllerOptions,
  type UpdateOffer,
} from "./useUpdateController";
import type { AppConfig } from "../../shared/contracts/backend-contracts";

// 後端錯誤代碼（ui_msg.rs 的 UiMsg）。
const CANNOT_REPLACE = 'TTMSG:{"code":"update_cannot_replace"}';
const UPDATE_CHANGED = 'TTMSG:{"code":"update_changed"}';

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ confirm: vi.fn(async () => true) }));
vi.mock("../settings/update-config", () => ({ updateConfig: vi.fn() }));

const offer: UpdateOffer = {
  version: "0.3.0",
  current_version: "0.2.0",
  notes: null,
  pub_date: null,
  level: "patch",
  skipped: false,
};
const available = (value: UpdateOffer = offer): CheckResult => ({ status: "available", offer: value });
const none: CheckResult = { status: "none" };
const failed: CheckResult = { status: "failed", message: "離線" };

const DAY_MS = 24 * 60 * 60 * 1000;

type Api = ReturnType<typeof useUpdateController>;
type Props = UpdateControllerOptions & { onApi: (api: Api) => void };

function Harness(props: Props) {
  const api = useUpdateController(props);
  useEffect(() => {
    props.onApi(api);
  });
  return null;
}

async function flush(times = 8) {
  await act(async () => {
    for (let i = 0; i < times; i += 1) await Promise.resolve();
  });
}

describe("useUpdateController", () => {
  let root: Root | null = null;
  let host: HTMLDivElement | null = null;

  afterEach(() => {
    act(() => {
      root?.unmount();
    });
    host?.remove();
    root = null;
    host = null;
    vi.useRealTimers();
  });

  async function render(props: Props) {
    if (!root) {
      host = document.createElement("div");
      document.body.appendChild(host);
      root = createRoot(host);
    }
    await act(async () => {
      root?.render(createElement(Harness, props));
    });
  }

  /** 檢查依序回 checks 裡的結果；用完就一直回最後一個。 */
  function setup(overrides: Partial<Props> & { checks?: CheckResult[] } = {}) {
    const box: { api: Api | null } = { api: null };
    const calls: { command: string; args?: Record<string, unknown> }[] = [];
    const checks = overrides.checks ?? [available()];
    let checkIndex = 0;
    const { invokeImpl: custom, ...rest } = overrides;
    const props: Props = {
      configLoaded: true,
      initialLoadReady: true,
      preferences: {},
      responding: false,
      onConfig: () => {},
      listenImpl: async () => () => {},
      invokeImpl: async (command, args) => {
        calls.push({ command, args });
        if (command === "update_check") {
          const result = checks[Math.min(checkIndex, checks.length - 1)];
          checkIndex += 1;
          return result;
        }
        if (command === "update_download") return { version: offer.version, rollback_ready: true };
        return undefined;
      },
      onApi: (next) => {
        box.api = next;
      },
      ...rest,
    };
    if (custom) {
      props.invokeImpl = (command, args) => {
        calls.push({ command, args });
        return custom(command, args);
      };
    }
    const commands = () => calls.map((call) => call.command);
    return { box, calls, commands, props };
  }

  it("checks once at startup and again after 24 hours, and posts launch once", async () => {
    vi.useFakeTimers();
    const { box, commands, props } = setup({ checks: [none], intervalMs: DAY_MS });
    await render(props);
    expect(commands().filter((command) => command === "update_post_launch")).toHaveLength(1);
    expect(commands().filter((command) => command === "update_check")).toHaveLength(1);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(DAY_MS);
    });
    expect(commands().filter((command) => command === "update_check")).toHaveLength(2);
    expect(box.api?.phase.kind).toBe("idle");
  });

  it("does not check when auto-check is off, but still runs post-launch", async () => {
    const { commands, props } = setup({ preferences: { update_auto_check: false } });
    await render(props);
    expect(commands()).toEqual(["update_post_launch"]);
  });

  it("settles launch only after post-launch finishes, even when it fails", async () => {
    let finish: (value?: unknown) => void = () => {};
    const box: { api: Api | null } = { api: null };
    const { props } = setup({
      initialLoadReady: false,
      invokeImpl: async (command) => {
        if (command === "update_post_launch") {
          return new Promise((_resolve, reject) => {
            finish = reject;
          });
        }
        return none;
      },
      onApi: (next) => {
        box.api = next;
      },
    });
    await render(props);
    expect(box.api?.launchSettled).toBe(false);
    await render({ ...props, initialLoadReady: true });
    expect(box.api?.launchSettled).toBe(false);
    await act(async () => {
      finish("壞了");
      await Promise.resolve();
    });
    await flush();
    expect(box.api?.launchSettled).toBe(true);
  });

  it("checks once across rerenders and does not reset the 24 hour timer", async () => {
    vi.useFakeTimers();
    const invokeMock = vi.mocked(invoke);
    invokeMock.mockReset();
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "update_check") return none;
      return undefined;
    });
    const props: Props = {
      configLoaded: true,
      initialLoadReady: true,
      preferences: {},
      responding: false,
      onConfig: () => {},
      intervalMs: DAY_MS,
      onApi: () => {},
    };
    const checks = () => invokeMock.mock.calls.filter((call) => call[0] === "update_check").length;
    await render(props);
    expect(checks()).toBe(1);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(23 * 60 * 60 * 1000);
    });
    await render(props);
    await render(props);
    expect(checks()).toBe(1);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(60 * 60 * 1000);
    });
    expect(checks()).toBe(2);
  });

  it("none clears the offer, failed keeps it, and an automatic failure shows nothing", async () => {
    vi.useFakeTimers();
    const { box, props } = setup({ checks: [available(), failed, none], intervalMs: DAY_MS });
    await render(props);
    await flush();
    expect(box.api?.offer?.version).toBe("0.3.0");

    await act(async () => {
      await vi.advanceTimersByTimeAsync(DAY_MS);
    });
    expect(box.api?.offer?.version).toBe("0.3.0");
    expect(box.api?.checkError).toBeNull();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(DAY_MS);
    });
    expect(box.api?.offer).toBeNull();
    expect(box.api?.phase.kind).toBe("idle");
  });

  it("a manual failure shows the reason and keeps the offer; a manual none clears it", async () => {
    const { box, props } = setup({ checks: [available(), failed, none] });
    await render(props);
    await flush();
    await act(async () => {
      await box.api?.checkNow();
    });
    expect(box.api?.checkError).toBe("離線");
    expect(box.api?.offer?.version).toBe("0.3.0");

    await act(async () => {
      await box.api?.checkNow();
    });
    expect(box.api?.checkError).toBeNull();
    expect(box.api?.offer).toBeNull();
    expect(box.api?.checked).toBe(true);
  });

  it("ignores check results while its own flow runs or while a rollback is blocking it", async () => {
    const newer = { ...offer, version: "0.4.0" };
    let blocked = false;
    const { box, props } = setup({
      checks: [available(), none, available(newer)],
      responding: true,
      isBlocked: () => blocked,
    });
    await render(props);
    await flush();
    act(() => {
      box.api?.requestInstall();
    });
    expect(box.api?.phase).toEqual({ kind: "waiting", offer });
    await act(async () => {
      await box.api?.checkNow();
    });
    expect(box.api?.phase).toEqual({ kind: "waiting", offer });

    act(() => {
      box.api?.cancelWait();
    });
    expect(box.api?.phase).toEqual({ kind: "available", offer });
    blocked = true;
    await act(async () => {
      await box.api?.checkNow();
    });
    expect(box.api?.offer?.version).toBe("0.3.0");
  });

  it("waits out an AI response and continues after it stops", async () => {
    const { box, calls, commands, props } = setup({ responding: true });
    await render(props);
    await flush();
    act(() => {
      box.api?.requestInstall();
    });
    expect(box.api?.phase).toEqual({ kind: "waiting", offer });
    expect(commands()).not.toContain("update_download");

    await render({ ...props, responding: false });
    await flush();
    expect(commands()).toContain("update_install");
    expect(calls.find((call) => call.command === "update_install")?.args).toEqual({ version: "0.3.0" });
  });

  it("cancelling a wait goes back to the offer and nothing downloads", async () => {
    const { box, commands, props } = setup({ responding: true });
    await render(props);
    await flush();
    act(() => {
      box.api?.requestInstall();
    });
    act(() => {
      box.api?.cancelWait();
    });
    await render({ ...props, responding: false });
    await flush();
    expect(box.api?.phase).toEqual({ kind: "available", offer });
    expect(commands()).not.toContain("update_download");
  });

  it("asks before installing without a rollback point, and cancel lets the player press update again", async () => {
    const ready = [false, true];
    let downloads = 0;
    const ask = vi.fn(async () => false);
    const downloaded = vi.fn();
    const { box, commands, props } = setup({
      askNoRollback: ask,
      onDownloaded: downloaded,
      invokeImpl: async (command) => {
        if (command === "update_check") return available();
        if (command === "update_download") {
          const result = { version: offer.version, rollback_ready: ready[downloads] };
          downloads += 1;
          return result;
        }
        if (command === "update_install") return new Promise(() => {});
        return undefined;
      },
    });
    const installs = () => commands().filter((command) => command === "update_install").length;
    await render(props);
    await flush();
    await act(async () => {
      box.api?.requestInstall();
    });
    await flush();
    expect(ask).toHaveBeenCalledTimes(1);
    expect(box.api?.phase).toEqual({ kind: "available", offer });
    expect(installs()).toBe(0);
    expect(downloaded).toHaveBeenCalledTimes(1);

    // 第二次下載回 rollback_ready 為真：不沿用上次的假，直接安裝、不再問。
    act(() => {
      box.api?.requestInstall();
    });
    await flush();
    expect(ask).toHaveBeenCalledTimes(1);
    expect(installs()).toBe(1);
    expect(downloaded).toHaveBeenCalledTimes(2);
  });

  it("continues to install when the player accepts the missing rollback point", async () => {
    const { box, commands, props } = setup({
      askNoRollback: async () => true,
      invokeImpl: async (command) => {
        if (command === "update_check") return available();
        if (command === "update_download") return { version: offer.version, rollback_ready: false };
        return undefined;
      },
    });
    await render(props);
    await flush();
    await act(async () => {
      box.api?.requestInstall();
    });
    await flush();
    expect(commands()).toContain("update_install");
  });

  it("a failed download keeps the offer for retry and does not reload config", async () => {
    let attempts = 0;
    const { box, commands, props } = setup({
      invokeImpl: async (command) => {
        if (command === "update_check") return available();
        if (command === "update_download") {
          attempts += 1;
          if (attempts === 1) throw "下載失敗";
          return { version: offer.version, rollback_ready: true };
        }
        return undefined;
      },
    });
    await render(props);
    await flush();
    await act(async () => {
      box.api?.requestInstall();
    });
    await flush();
    expect(box.api?.phase).toEqual({ kind: "error", offer, message: "下載失敗" });
    expect(box.api?.offer).toEqual(offer);
    expect(commands()).not.toContain("read_config");

    await act(async () => {
      box.api?.requestInstall();
    });
    await flush();
    expect(commands()).toContain("update_install");
  });

  it("reloads config after a failed install so a cleared skip shows up", async () => {
    const fresh = { preferences: {} } as AppConfig;
    const configs: AppConfig[] = [];
    const { box, props } = setup({
      onConfig: (config) => {
        configs.push(config);
      },
      invokeImpl: async (command) => {
        if (command === "update_check") return available();
        if (command === "update_download") return { version: offer.version, rollback_ready: true };
        if (command === "update_install") throw CANNOT_REPLACE;
        if (command === "read_config") return fresh;
        return undefined;
      },
    });
    await render(props);
    await flush();
    await act(async () => {
      box.api?.requestInstall();
    });
    await flush();
    expect(configs).toEqual([fresh]);
    expect(box.api?.phase).toEqual({ kind: "error", offer, message: CANNOT_REPLACE });
  });

  it("skip writes the version and unskip clears it, keeping the offer either way", async () => {
    const saved: Record<string, unknown>[] = [];
    const { box, props } = setup({
      savePreference: async (patch) => {
        saved.push(patch);
        return { preferences: {} } as never;
      },
    });
    await render(props);
    await flush();
    await act(async () => {
      await box.api?.skip("0.3.0");
    });
    await act(async () => {
      await box.api?.unskip();
    });
    expect(saved).toEqual([
      { preferences: { update_skipped_version: "0.3.0" } },
      { preferences: { update_skipped_version: null } },
    ]);
    expect(box.api?.offer).toEqual(offer);
  });

  it("reminds only from the startup automatic check", async () => {
    vi.useFakeTimers();
    const feature = { ...offer, level: "feature" as const };
    const { box, props } = setup({ checks: [none, available(feature)], intervalMs: DAY_MS });
    await render(props);
    await flush();
    expect(box.api?.reminder).toBeNull();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(DAY_MS);
    });
    expect(box.api?.offer?.version).toBe("0.3.0");
    expect(box.api?.reminder).toBeNull();
    await act(async () => {
      await box.api?.checkNow();
    });
    expect(box.api?.reminder).toBeNull();
  });

  it("the startup check picks banner, dialog or nothing from level, skip and reminded", async () => {
    const cases: [UpdateOffer["level"], Record<string, unknown>, string | null][] = [
      ["feature", {}, "banner"],
      ["format", {}, "dialog"],
      ["patch", {}, null],
      ["feature", { update_skipped_version: "0.3.0" }, null],
      ["format", { update_reminded_version: "0.3.0" }, null],
    ];
    for (const [level, preferences, expected] of cases) {
      const { box, props } = setup({ checks: [available({ ...offer, level })], preferences });
      await render(props);
      await flush();
      expect(box.api?.reminder?.kind ?? null).toBe(expected);
      act(() => {
        root?.unmount();
      });
      root = null;
    }
  });

  it("a failed startup check reminds nothing and writes nothing", async () => {
    const save = vi.fn();
    const { box, props } = setup({ checks: [failed], savePreference: save });
    await render(props);
    await flush();
    expect(box.api?.reminder).toBeNull();
    expect(save).not.toHaveBeenCalled();
  });

  it("writes the reminded version once per run, only when asked after drawing", async () => {
    const saved: Record<string, unknown>[] = [];
    const { box, props } = setup({
      checks: [available({ ...offer, level: "feature" })],
      savePreference: async (patch) => {
        saved.push(patch);
        return { preferences: {} } as never;
      },
    });
    await render(props);
    await flush();
    expect(box.api?.reminder?.kind).toBe("banner");
    expect(saved).toEqual([]);
    act(() => {
      box.api?.markReminderShown("0.3.0");
      box.api?.markReminderShown("0.3.0");
    });
    await flush();
    expect(saved).toEqual([{ preferences: { update_reminded_version: "0.3.0" } }]);
    act(() => {
      box.api?.dismissReminder();
    });
    expect(box.api?.reminder).toBeNull();
  });

  it("downloads only the version the player saw; a replaced slot reshows the latest offer", async () => {
    const formatB = { ...offer, version: "0.4.0", level: "format" as const };
    const { box, calls, commands, props } = setup({
      responding: true,
      checks: [available(), available(formatB)],
      invokeImpl: async (command, args) => {
        if (command === "update_check") {
          return commands().filter((item) => item === "update_check").length === 1
            ? available()
            : available(formatB);
        }
        if (command === "update_download") {
          if (args?.version !== "0.4.0") throw UPDATE_CHANGED;
          return { version: "0.4.0", rollback_ready: true };
        }
        return undefined;
      },
    });
    await render(props);
    await flush();
    act(() => {
      box.api?.requestInstall();
    });
    // 等待中的檢查把後端槽換成 B，前端不採用。
    await act(async () => {
      await box.api?.checkNow();
    });
    expect(box.api?.phase).toEqual({ kind: "waiting", offer });

    await render({ ...props, responding: false });
    await flush();
    expect(calls.find((call) => call.command === "update_download")?.args).toEqual({
      version: "0.3.0",
    });
    expect(commands()).not.toContain("update_install");
    expect(box.api?.phase).toEqual({ kind: "available", offer: formatB });

    await act(async () => {
      box.api?.requestInstall();
    });
    await flush();
    expect(calls.find((call) => call.command === "update_install")?.args).toEqual({
      version: "0.4.0",
    });
  });

  it("the 24 hour check does not hit the network while a flow is running", async () => {
    vi.useFakeTimers();
    const { box, commands, props } = setup({
      intervalMs: DAY_MS,
      invokeImpl: async (command) => {
        if (command === "update_check") return available();
        if (command === "update_download") return new Promise(() => {});
        return undefined;
      },
    });
    await render(props);
    await flush();
    act(() => {
      box.api?.requestInstall();
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(DAY_MS);
    });
    expect(commands().filter((command) => command === "update_check")).toHaveLength(1);
  });

  it("none clears the startup reminder, and an offer for another version clears it too", async () => {
    vi.useFakeTimers();
    const feature = { ...offer, level: "feature" as const };
    const first = setup({ checks: [available(feature), none], intervalMs: DAY_MS });
    await render(first.props);
    await flush();
    expect(first.box.api?.reminder?.kind).toBe("banner");
    await act(async () => {
      await vi.advanceTimersByTimeAsync(DAY_MS);
    });
    expect(first.box.api?.reminder).toBeNull();
    act(() => {
      root?.unmount();
    });
    root = null;

    const second = setup({
      checks: [available(feature), available(feature), available({ ...feature, version: "0.4.0" })],
      intervalMs: DAY_MS,
    });
    await render(second.props);
    await flush();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(DAY_MS);
    });
    expect(second.box.api?.reminder?.offer.version).toBe("0.3.0");
    await act(async () => {
      await vi.advanceTimersByTimeAsync(DAY_MS);
    });
    expect(second.box.api?.offer?.version).toBe("0.4.0");
    expect(second.box.api?.reminder).toBeNull();
  });

  it("a recheck after a replaced version stays busy, blocks a second press, and late results keep the flow consistent", async () => {
    const b = { ...offer, version: "0.4.0" };
    const pending: ((result: CheckResult) => void)[] = [];
    let blockedChecks = false;
    const { box, commands, props } = setup({
      invokeImpl: async (command, args) => {
        if (command === "update_check") {
          if (!blockedChecks) return available();
          return new Promise<CheckResult>((resolve) => {
            pending.push(resolve);
          });
        }
        if (command === "update_download") {
          if (args?.version !== "0.4.0") throw UPDATE_CHANGED;
          return new Promise(() => {});
        }
        return undefined;
      },
    });
    await render(props);
    await flush();
    blockedChecks = true;
    await act(async () => {
      box.api?.requestInstall();
    });
    await flush();
    expect(box.api?.phase).toEqual({ kind: "rechecking", offer });
    expect(box.api?.isFlowBusy()).toBe(true);

    // 重新檢查①還沒回來：再按更新不會再下載、也不會開第二次重新檢查。
    act(() => {
      box.api?.requestInstall();
    });
    await flush();
    expect(commands().filter((command) => command === "update_download")).toHaveLength(1);
    expect(pending).toHaveLength(1);

    // 期間另一個手動檢查回 B：流程忙碌，不採用。
    void box.api?.checkNow();
    await flush();
    pending[1]?.(available(b));
    await flush();
    expect(box.api?.phase).toEqual({ kind: "rechecking", offer });

    // ①晚到失敗：仍是這一次重新檢查，才套用；沒有任何 B 的下載被解除互斥。
    await act(async () => {
      pending[0]({ status: "failed", message: "離線" });
    });
    await flush();
    expect(box.api?.phase).toEqual({ kind: "error", offer, message: "離線" });
    expect(commands().filter((command) => command === "update_download")).toHaveLength(1);
  });
});
