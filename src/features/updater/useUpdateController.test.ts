// @vitest-environment happy-dom

import { act, createElement, useEffect } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import {
  useUpdateController,
  type UpdateControllerOptions,
  type UpdateOffer,
} from "./useUpdateController";
import type { AppConfig } from "../../shared/contracts/backend-contracts";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock("../settings/update-config", () => ({ updateConfig: vi.fn() }));

const offer: UpdateOffer = {
  version: "0.3.0",
  current_version: "0.2.0",
  notes: null,
  pub_date: null,
  level: "patch",
  skipped: false,
};

const DAY_MS = 24 * 60 * 60 * 1000;

type Api = ReturnType<typeof useUpdateController>;

function Harness(props: UpdateControllerOptions & { onApi: (api: Api) => void }) {
  const api = useUpdateController(props);
  useEffect(() => {
    props.onApi(api);
  });
  return null;
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

  async function render(props: UpdateControllerOptions & { onApi: (api: Api) => void }) {
    if (!root) {
      host = document.createElement("div");
      document.body.appendChild(host);
      root = createRoot(host);
    }
    await act(async () => {
      root?.render(createElement(Harness, props));
    });
  }

  it("checks once at startup and again after 24 hours, and posts launch once", async () => {
    vi.useFakeTimers();
    const calls: string[] = [];
    const box: { api: Api | null } = { api: null };
    const invokeImpl = vi.fn(async (command: string) => {
      calls.push(command);
      if (command === "update_check") return null;
      return undefined;
    });
    await render({
      configLoaded: true,
      initialLoadReady: true,
      preferences: {},
      responding: false,
      stopResponse: () => {},
      onConfig: () => {},
      invokeImpl,
      intervalMs: DAY_MS,
      onApi: (next) => {
        box.api = next;
      },
    });
    expect(calls.filter((command) => command === "update_post_launch")).toEqual(["update_post_launch"]);
    expect(calls.filter((command) => command === "update_check")).toEqual(["update_check"]);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(DAY_MS);
    });
    expect(calls.filter((command) => command === "update_check")).toHaveLength(2);
    expect(box.api?.phase?.kind).toBe("idle");
  });

  it("does not check when auto-check is off, but still runs post-launch", async () => {
    const calls: string[] = [];
    await render({
      configLoaded: true,
      initialLoadReady: true,
      preferences: { update_auto_check: false },
      responding: false,
      stopResponse: () => {},
      onConfig: () => {},
      invokeImpl: async (command) => {
        calls.push(command);
        return null;
      },
      onApi: () => {},
    });
    expect(calls).toEqual(["update_post_launch"]);
  });

  it("waits to post launch until settings and the world list have both loaded", async () => {
    const calls: string[] = [];
    const invokeImpl = async (command: string) => {
      calls.push(command);
      return null;
    };
    await render({
      configLoaded: true,
      initialLoadReady: false,
      preferences: {},
      responding: false,
      stopResponse: () => {},
      onConfig: () => {},
      invokeImpl,
      onApi: () => {},
    });
    expect(calls).not.toContain("update_post_launch");
    await render({
      configLoaded: true,
      initialLoadReady: true,
      preferences: {},
      responding: false,
      stopResponse: () => {},
      onConfig: () => {},
      invokeImpl,
      onApi: () => {},
    });
    expect(calls.filter((command) => command === "update_post_launch")).toEqual([
      "update_post_launch",
    ]);
  });

  it("waits out an AI response and continues after it stops, without calling stop itself", async () => {
    const calls: string[] = [];
    const installArgs: unknown[] = [];
    const stop = vi.fn();
    const box: { api: Api | null } = { api: null };
    const props = {
      configLoaded: true,
      initialLoadReady: true,
      preferences: {},
      responding: true,
      stopResponse: stop,
      onConfig: () => {},
      invokeImpl: async (command: string, args?: Record<string, unknown>) => {
        calls.push(command);
        if (command === "update_check") return offer;
        if (command === "update_download") return { version: offer.version, rollback_ready: true };
        if (command === "update_install") installArgs.push(args);
        return undefined;
      },
      listenImpl: async (_event: string, handler: (event: { payload: { downloaded: number; total: number | null } }) => void) => {
        handler({ payload: { downloaded: 4, total: 8 } });
        return () => {};
      },
      onApi: (next: Api) => {
        box.api = next;
      },
    };
    await render(props);
    await act(async () => {
      await Promise.resolve();
    });
    expect(box.api?.phase?.kind).toBe("available");
    act(() => {
      box.api?.requestInstall();
    });
    expect(box.api?.phase).toEqual({ kind: "waiting", offer });
    expect(stop).not.toHaveBeenCalled();
    expect(calls).not.toContain("update_download");

    await render({ ...props, responding: false });
    await act(async () => {
      await Promise.resolve();
    });
    expect(calls).toContain("update_download");
    expect(calls).toContain("update_install");
    expect(installArgs).toEqual([{ version: "0.3.0" }]);
    expect(stop).not.toHaveBeenCalled();
  });

  it("checks once across rerenders and does not reset the 24 hour timer", async () => {
    vi.useFakeTimers();
    const invokeMock = vi.mocked(invoke);
    invokeMock.mockReset();
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "update_check") return null;
      return undefined;
    });
    const props = {
      configLoaded: true,
      initialLoadReady: true,
      preferences: {},
      responding: false,
      stopResponse: () => {},
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
    expect(checks()).toBe(1);

    await render(props);
    await render(props);
    expect(checks()).toBe(1);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(60 * 60 * 1000);
    });
    expect(checks()).toBe(2);
    expect(invokeMock.mock.calls.filter((call) => call[0] === "update_post_launch")).toHaveLength(1);
  });

  it("does not reload config when the download fails", async () => {
    const commands: string[] = [];
    const box: { api: Api | null } = { api: null };
    await render({
      configLoaded: true,
      initialLoadReady: true,
      preferences: {},
      responding: false,
      stopResponse: () => {},
      onConfig: () => {},
      invokeImpl: async (command: string) => {
        commands.push(command);
        if (command === "update_check") return offer;
        if (command === "update_download") throw "下載失敗";
        return undefined;
      },
      listenImpl: async () => () => {},
      onApi: (next) => {
        box.api = next;
      },
    });
    await act(async () => {
      await Promise.resolve();
    });
    await act(async () => {
      box.api?.requestInstall();
      for (let i = 0; i < 8; i += 1) await Promise.resolve();
    });
    expect(box.api?.phase).toEqual({ kind: "error", message: "下載失敗" });
    expect(commands).not.toContain("read_config");
    expect(commands).not.toContain("update_install");
  });

  it("reloads config after a failed install so a cleared skip shows up", async () => {
    const fresh = { preferences: {} } as AppConfig;
    const configs: AppConfig[] = [];
    const box: { api: Api | null } = { api: null };
    await render({
      configLoaded: true,
      initialLoadReady: true,
      preferences: {},
      responding: false,
      stopResponse: () => {},
      onConfig: (config) => {
        configs.push(config);
      },
      invokeImpl: async (command: string) => {
        if (command === "update_check") return offer;
        if (command === "update_download") return { version: offer.version, rollback_ready: true };
        if (command === "update_install") throw "無法自動替換";
        if (command === "read_config") return fresh;
        return undefined;
      },
      listenImpl: async () => () => {},
      onApi: (next) => {
        box.api = next;
      },
    });
    await act(async () => {
      await Promise.resolve();
    });
    expect(box.api?.phase?.kind).toBe("available");
    await act(async () => {
      box.api?.requestInstall();
      for (let i = 0; i < 8; i += 1) await Promise.resolve();
    });
    expect(configs).toEqual([fresh]);
    expect(box.api?.phase).toEqual({ kind: "error", message: "無法自動替換" });
  });

  it("writes the skipped version into preferences", async () => {
    const saved: Record<string, unknown>[] = [];
    const box: { api: Api | null } = { api: null };
    const config = { preferences: { update_skipped_version: "0.3.0" } } as never;
    await render({
      configLoaded: true,
      initialLoadReady: true,
      preferences: {},
      responding: false,
      stopResponse: () => {},
      onConfig: () => {},
      invokeImpl: async () => null,
      savePreference: async (patch) => {
        saved.push(patch);
        return config;
      },
      onApi: (next) => {
        box.api = next;
      },
    });
    await act(async () => {
      await box.api?.skip("0.3.0");
    });
    expect(saved).toEqual([{ preferences: { update_skipped_version: "0.3.0" } }]);
    expect(box.api?.phase?.kind).toBe("idle");
  });
});
