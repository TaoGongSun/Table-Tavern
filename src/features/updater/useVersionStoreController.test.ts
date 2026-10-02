// @vitest-environment happy-dom

import { act, createElement, useEffect } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  useVersionStoreController,
  type VersionList,
  type VersionStoreControllerOptions,
} from "./useVersionStoreController";
import type { RollbackCheck } from "./version-center";

// 後端錯誤代碼（ui_msg.rs 的 UiMsg）。
const CANNOT_REPLACE = 'TTMSG:{"code":"update_cannot_replace"}';
const SIGNATURE_INVALID = 'TTMSG:{"code":"signature_invalid"}';
const VERSION_NOT_FOUND = 'TTMSG:{"code":"version_not_found"}';

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

type Api = ReturnType<typeof useVersionStoreController>;
type Props = VersionStoreControllerOptions & { onApi: (api: Api) => void };

const versions: VersionList = {
  total_bytes: 1,
  versions: [
    {
      version: "0.2.0",
      size: 1,
      format_version: 1,
      usable: true,
      eligible: false,
      current: true,
      previous: false,
    },
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
const emptyPreview = { will_be_readonly: [], maybe_readonly: [], scan_failed: false };

function Harness(props: Props) {
  const api = useVersionStoreController(props);
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

describe("useVersionStoreController", () => {
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

  function setup(overrides: Partial<Props> = {}, list: VersionList = versions) {
    const box: { api: Api | null } = { api: null };
    const calls: string[] = [];
    const asked: RollbackCheck[] = [];
    const props: Props = {
      initialLoadReady: true,
      responding: false,
      onConfig: () => {},
      askRollback: async (_version, check) => {
        asked.push(check);
        return true;
      },
      invokeImpl: async (command: string, args?: Record<string, unknown>) => {
        calls.push(command);
        if (command === "list_versions") return list;
        if (command === "list_world_backups") return { backups: [], total_bytes: 0 };
        if (command === "rollback_preview") return emptyPreview;
        if (command === "rollback_install") calls.push(String(args?.version));
        return undefined;
      },
      onApi: (next) => {
        box.api = next;
      },
      ...overrides,
    };
    return { box, calls, asked, props };
  }

  it("loads lists only once the launch has settled", async () => {
    const { calls, props } = setup({ initialLoadReady: false });
    await render(props);
    expect(calls).toEqual([]);
    await render({ ...props, initialLoadReady: true });
    expect(calls).toEqual(["list_versions", "list_world_backups"]);
  });

  it("one-click previous previews and confirms first, then waits out an AI response", async () => {
    const { box, calls, asked, props } = setup({ responding: true });
    await render(props);
    await flush();
    await act(async () => {
      await box.api?.startRollbackPrevious();
    });
    expect(asked).toEqual([{ kind: "preview", preview: emptyPreview }]);
    expect(box.api?.phase).toEqual({ kind: "waiting", version: "0.1.0" });
    expect(calls).not.toContain("rollback_install");

    await render({ ...props, responding: false });
    await flush();
    expect(calls).toContain("rollback_install");
    expect(calls).toContain("0.1.0");
  });

  it("one-click previous needs the previous row to be eligible", async () => {
    const list = {
      ...versions,
      versions: versions.versions.map((row) => ({ ...row, eligible: false })),
    };
    const { box, calls, props } = setup({}, list);
    await render(props);
    await flush();
    await act(async () => {
      await box.api?.startRollbackPrevious();
    });
    expect(calls).not.toContain("rollback_preview");
    expect(box.api?.phase.kind).toBe("idle");
  });

  it("an ineligible target only reaches a close-only dialog, even if the dialog says yes", async () => {
    const { box, calls, asked, props } = setup({
      invokeImpl: async (command: string) => {
        calls.push(command);
        if (command === "list_versions") return versions;
        if (command === "rollback_preview") throw SIGNATURE_INVALID;
        return { backups: [], total_bytes: 0 };
      },
    });
    await render(props);
    await flush();
    await act(async () => {
      await box.api?.startRollback("0.1.0");
    });
    expect(asked).toEqual([{ kind: "invalid", message: SIGNATURE_INVALID }]);
    expect(calls).not.toContain("rollback_install");
    expect(box.api?.phase.kind).toBe("idle");
  });

  it("declining the confirm dialog ends the flow", async () => {
    const { box, calls, props } = setup({ askRollback: async () => false });
    await render(props);
    await flush();
    await act(async () => {
      await box.api?.startRollback("0.1.0");
    });
    expect(calls).not.toContain("rollback_install");
    expect(box.api?.phase.kind).toBe("idle");
  });

  it("cancelling a wait goes back to idle without installing", async () => {
    const { box, calls, props } = setup({ responding: true });
    await render(props);
    await flush();
    await act(async () => {
      await box.api?.startRollback("0.1.0");
    });
    act(() => {
      box.api?.cancelWait();
    });
    await render({ ...props, responding: false });
    await flush();
    expect(box.api?.phase.kind).toBe("idle");
    expect(calls).not.toContain("rollback_install");
  });

  it("does not start while blocked by the update flow", async () => {
    const { box, calls, props } = setup({ isBlocked: () => true });
    await render(props);
    await flush();
    await act(async () => {
      await box.api?.startRollback("0.1.0");
    });
    expect(calls).not.toContain("rollback_preview");
    expect(box.api?.isFlowBusy()).toBe(false);
  });

  it("a failed rollback keeps the version in the error and reloads config for the skip key", async () => {
    const fresh = { preferences: { update_skipped_version: "0.2.0" } };
    const configs: unknown[] = [];
    const { box, props } = setup({
      onConfig: (config) => {
        configs.push(config);
      },
      invokeImpl: async (command: string) => {
        if (command === "list_versions") return versions;
        if (command === "rollback_preview") return emptyPreview;
        if (command === "rollback_install") throw CANNOT_REPLACE;
        if (command === "read_config") return fresh;
        return { backups: [], total_bytes: 0 };
      },
    });
    await render(props);
    await flush();
    await act(async () => {
      await box.api?.startRollback("0.1.0");
    });
    expect(box.api?.phase).toEqual({ kind: "error", version: "0.1.0", message: CANNOT_REPLACE });
    expect(configs).toEqual([fresh]);
  });

  it("a failed list load is an error with a retry, not an empty store", async () => {
    let fail = true;
    const { box, props } = setup({
      invokeImpl: async (command: string) => {
        if (command === "list_versions") {
          if (fail) throw "讀不到";
          return versions;
        }
        return { backups: [], total_bytes: 0 };
      },
    });
    await render(props);
    await flush();
    expect(box.api?.loadError).toBe("讀不到");
    expect(box.api?.versions).toBeNull();
    fail = false;
    await act(async () => {
      await box.api?.refresh();
    });
    expect(box.api?.loadError).toBeNull();
    expect(box.api?.versions).toEqual(versions);
  });

  it("deletes a version and a world backup, then reloads, and reports a failed delete", async () => {
    const calls: string[] = [];
    const { box, props } = setup({
      invokeImpl: async (command: string, args?: Record<string, unknown>) => {
        calls.push(command);
        if (command === "delete_version") {
          calls.push(String(args?.version));
          if (args?.version === "0.0.9") throw VERSION_NOT_FOUND;
        }
        if (command === "delete_world_backup") calls.push(`${args?.worldId}:${args?.kind}`);
        if (command === "list_versions") return versions;
        return { backups: [], total_bytes: 0 };
      },
    });
    await render(props);
    await flush();
    calls.length = 0;
    await act(async () => {
      await box.api?.deleteVersion("0.1.0");
      await box.api?.deleteBackup("01ARZ3NDEKTSV4RRFFQ69G5FAV", "pre");
    });
    expect(calls).toEqual([
      "delete_version",
      "0.1.0",
      "list_versions",
      "list_world_backups",
      "delete_world_backup",
      "01ARZ3NDEKTSV4RRFFQ69G5FAV:pre",
      "list_versions",
      "list_world_backups",
    ]);
    let failure: unknown = null;
    await act(async () => {
      await box.api?.deleteVersion("0.0.9").catch((reason: unknown) => {
        failure = reason;
      });
    });
    expect(failure).toBe(VERSION_NOT_FOUND);
  });
});
