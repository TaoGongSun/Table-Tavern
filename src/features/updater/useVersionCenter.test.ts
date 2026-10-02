// @vitest-environment happy-dom

import { act, createElement, useEffect } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useVersionCenter, type VersionCenterOptions } from "./useVersionCenter";
import type { CheckResult } from "./useUpdateController";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock("@tauri-apps/api/app", () => ({ getVersion: vi.fn(async () => "0.2.0") }));
vi.mock("@tauri-apps/plugin-dialog", () => ({
  confirm: vi.fn(async () => true),
  message: vi.fn(async () => {}),
}));
vi.mock("../settings/update-config", () => ({ updateConfig: vi.fn() }));

type Api = ReturnType<typeof useVersionCenter>;
type Props = VersionCenterOptions & { onApi: (api: Api) => void };

const available: CheckResult = {
  status: "available",
  offer: {
    version: "0.3.0",
    current_version: "0.2.0",
    notes: null,
    pub_date: null,
    level: "patch",
    skipped: false,
  },
};
const versions = {
  total_bytes: 1,
  versions: [
    {
      version: "0.1.0",
      size: 1,
      format_version: 1,
      usable: true,
      eligible: true,
      current: false,
      previous: true,
    },
  ],
};

function Harness(props: Props) {
  const api = useVersionCenter(props);
  useEffect(() => {
    props.onApi(api);
  });
  return null;
}

async function flush(times = 10) {
  await act(async () => {
    for (let i = 0; i < times; i += 1) await Promise.resolve();
  });
}

describe("useVersionCenter", () => {
  let root: Root | null = null;
  let host: HTMLDivElement | null = null;

  afterEach(() => {
    act(() => {
      root?.unmount();
    });
    host?.remove();
    root = null;
    host = null;
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

  function setup(handle: (command: string) => unknown) {
    const box: { api: Api | null } = { api: null };
    const calls: string[] = [];
    const props: Props = {
      configLoaded: true,
      initialLoadReady: true,
      preferences: {},
      responding: false,
      stopResponse: () => {},
      onConfig: () => {},
      readVersion: async () => "0.2.0",
      listenImpl: async () => () => {},
      askRollback: async () => true,
      invokeImpl: async (command) => {
        calls.push(command);
        const handled = handle(command);
        if (handled !== undefined) return handled;
        if (command === "update_check") return available;
        if (command === "list_versions") return versions;
        if (command === "list_world_backups") return { backups: [], total_bytes: 0 };
        if (command === "rollback_preview") {
          return { will_be_readonly: [], maybe_readonly: [], scan_failed: false };
        }
        return null;
      },
      onApi: (next) => {
        box.api = next;
      },
    };
    return { box, calls, props };
  }

  it("reads the version list only after post-launch has finished", async () => {
    let settle: () => void = () => {};
    const { calls, props } = setup((command) =>
      command === "update_post_launch"
        ? new Promise<void>((resolve) => {
            settle = resolve;
          })
        : undefined,
    );
    await render(props);
    await flush();
    expect(calls).toContain("update_post_launch");
    expect(calls).not.toContain("list_versions");
    await act(async () => {
      settle();
    });
    await flush();
    expect(calls.indexOf("list_versions")).toBeGreaterThan(calls.indexOf("update_post_launch"));
  });

  it("refreshes the version list after a download completes", async () => {
    const { box, calls, props } = setup((command) => {
      if (command === "update_download") return { version: "0.3.0", rollback_ready: true };
      if (command === "update_install") return new Promise(() => {});
      return undefined;
    });
    await render(props);
    await flush();
    const before = calls.filter((command) => command === "list_versions").length;
    await act(async () => {
      box.api?.update.requestInstall();
    });
    await flush();
    expect(calls.filter((command) => command === "list_versions").length).toBe(before + 1);
    expect(calls.indexOf("update_download")).toBeLessThan(calls.lastIndexOf("list_versions"));
  });

  it("an update in flight blocks rollback, and a rollback in flight blocks update", async () => {
    const { box, calls, props } = setup((command) => {
      if (command === "update_download") return new Promise(() => {});
      return undefined;
    });
    await render(props);
    await flush();
    act(() => {
      box.api?.update.requestInstall();
    });
    expect(box.api?.busy).toBe(true);
    await act(async () => {
      await box.api?.store.startRollback("0.1.0");
    });
    expect(calls).not.toContain("rollback_preview");
    act(() => {
      root?.unmount();
    });
    root = null;

    const second = setup((command) => {
      if (command === "rollback_preview") return new Promise(() => {});
      return undefined;
    });
    await render(second.props);
    await flush();
    act(() => {
      void second.box.api?.store.startRollback("0.1.0");
    });
    expect(second.box.api?.busy).toBe(true);
    act(() => {
      second.box.api?.update.requestInstall();
    });
    await flush();
    expect(second.calls).not.toContain("update_download");
    expect(second.box.api?.update.phase.kind).toBe("available");
  });

  it("a recheck after a replaced version blocks rollback", async () => {
    let checks = 0;
    const { box, calls, props } = setup((command) => {
      if (command === "update_check") {
        checks += 1;
        return checks === 1 ? undefined : new Promise(() => {});
      }
      if (command === "update_download") return Promise.reject("要更新的版本已經換了");
      return undefined;
    });
    await render(props);
    await flush();
    await act(async () => {
      box.api?.update.requestInstall();
    });
    await flush();
    expect(box.api?.update.phase.kind).toBe("rechecking");
    expect(box.api?.busy).toBe(true);
    await act(async () => {
      await box.api?.store.startRollback("0.1.0");
    });
    expect(calls).not.toContain("rollback_preview");
  });
});
