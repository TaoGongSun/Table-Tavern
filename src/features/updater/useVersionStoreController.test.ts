// @vitest-environment happy-dom

import { act, createElement, useEffect } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  useVersionStoreController,
  type VersionList,
  type VersionStoreControllerOptions,
} from "./useVersionStoreController";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

type Api = ReturnType<typeof useVersionStoreController>;

const versions: VersionList = {
  total_bytes: 1,
  versions: [
    {
      version: "0.2.0",
      size: 1,
      format_version: 1,
      usable: true,
      current: true,
      previous: false,
    },
    {
      version: "0.1.0",
      size: 1,
      format_version: 1,
      usable: true,
      current: false,
      previous: true,
    },
  ],
};

function Harness(props: VersionStoreControllerOptions & { onApi: (api: Api) => void }) {
  const api = useVersionStoreController(props);
  useEffect(() => {
    props.onApi(api);
  });
  return null;
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

  async function render(props: VersionStoreControllerOptions & { onApi: (api: Api) => void }) {
    if (!root) {
      host = document.createElement("div");
      document.body.appendChild(host);
      root = createRoot(host);
    }
    await act(async () => {
      root?.render(createElement(Harness, props));
    });
  }

  it("loads lists only after the initial load, and rolls back the recorded previous version", async () => {
    const calls: string[] = [];
    const box: { api: Api | null } = { api: null };
    const invokeImpl = async (command: string) => {
      calls.push(command);
      if (command === "list_versions") return versions;
      if (command === "list_world_backups") return { backups: [], total_bytes: 0 };
      return undefined;
    };
    await render({
      initialLoadReady: false,
      responding: false,
      invokeImpl,
      onApi: (next) => {
        box.api = next;
      },
    });
    expect(calls).toEqual([]);

    await render({
      initialLoadReady: true,
      responding: true,
      invokeImpl,
      onApi: (next) => {
        box.api = next;
      },
    });
    expect(calls).toEqual(["list_versions", "list_world_backups"]);
    act(() => {
      box.api?.requestRollbackPrevious();
    });
    expect(box.api?.phase).toEqual({ kind: "waiting", version: "0.1.0" });
    expect(calls).not.toContain("rollback_install");

    await render({
      initialLoadReady: true,
      responding: false,
      invokeImpl,
      onApi: (next) => {
        box.api = next;
      },
    });
    await act(async () => {
      await Promise.resolve();
    });
    expect(calls).toContain("rollback_install");
  });

  it("does not roll back when no row is marked previous", async () => {
    const calls: string[] = [];
    const box: { api: Api | null } = { api: null };
    await render({
      initialLoadReady: true,
      responding: false,
      invokeImpl: async (command: string) => {
        calls.push(command);
        if (command === "list_versions") {
          return { ...versions, versions: versions.versions.map((row) => ({ ...row, previous: false })) };
        }
        return { backups: [], total_bytes: 0 };
      },
      onApi: (next) => {
        box.api = next;
      },
    });
    await act(async () => {
      await Promise.resolve();
    });
    act(() => {
      box.api?.requestRollbackPrevious();
    });
    expect(calls).not.toContain("rollback_install");
    expect(box.api?.phase.kind).toBe("idle");
  });

  it("deletes a version and a world backup, then reloads", async () => {
    const calls: string[] = [];
    const box: { api: Api | null } = { api: null };
    await render({
      initialLoadReady: true,
      responding: false,
      invokeImpl: async (command: string, args?: Record<string, unknown>) => {
        calls.push(command);
        if (command === "delete_version") calls.push(String(args?.version));
        if (command === "delete_world_backup") calls.push(`${args?.worldId}:${args?.kind}`);
        if (command === "list_versions") return versions;
        if (command === "rollback_preview") {
          return { will_be_readonly: [{ id: "w", name: "霧港" }], maybe_readonly: [] };
        }
        return { backups: [], total_bytes: 0 };
      },
      onApi: (next) => {
        box.api = next;
      },
    });
    await act(async () => {
      await box.api?.deleteVersion("0.1.0");
      await box.api?.deleteBackup("01ARZ3NDEKTSV4RRFFQ69G5FAV", "pre");
      await box.api?.loadPreview("0.1.0");
    });
    expect(calls).toContain("0.1.0");
    expect(calls).toContain("01ARZ3NDEKTSV4RRFFQ69G5FAV:pre");
    expect(box.api?.preview?.will_be_readonly[0].name).toBe("霧港");
  });
});
