// @vitest-environment happy-dom
// AI 生圖窗的來源下拉：不會生圖的 CLI（claude）不列，舊存檔指到它時照 fallback 走。

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { AppConfig } from "../../shared/contracts/backend-contracts";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (command: string) => {
    if (command === "detect_clis") {
      return [
        { id: "claude", path: "/bin/claude", version: "1" },
        { id: "codex", path: "/bin/codex", version: "1" },
      ];
    }
    if (command === "list_gallery_images") return [];
    return null;
  }),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ confirm: vi.fn(async () => true) }));

import { AiImageDialog } from "./CardImageDialogs";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

let root: Root | null = null;
let host: HTMLDivElement | null = null;

afterEach(() => {
  act(() => root?.unmount());
  host?.remove();
  root = null;
  host = null;
});

async function mount(preferences: Record<string, unknown>) {
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
  const config = { api_keys: {}, preferences, tier_models: {} } as unknown as AppConfig;
  await act(async () => {
    root!.render(
      <AiImageDialog
        world="w"
        characterId="c"
        name="狐狸"
        description=""
        initialPrompt=""
        config={config}
        onPreference={async () => {}}
        onOpenAiSettings={() => {}}
        onPromptUsed={() => {}}
        onPick={() => {}}
        onClose={() => {}}
      />,
    );
  });
  const select = document.querySelector<HTMLSelectElement>(".ai-gen-framing ~ label select")!;
  return { select, options: [...select.options].map((option) => option.value) };
}

describe("AiImageDialog 生圖來源", () => {
  it("偵測到 claude 也不列，能生圖的 CLI 照列", async () => {
    const { options } = await mount({ transport: "api" });
    expect(options).toEqual(["api", "codex"]);
  });

  it("舊存檔指到 claude、聊天來源能生圖：跟聊天來源", async () => {
    const { select } = await mount({ image_source: "claude", transport: "codex" });
    expect(select.value).toBe("codex");
  });

  it("舊存檔指到 claude、聊天來源也是 claude：退回 API", async () => {
    const { select, options } = await mount({ image_source: "claude", transport: "claude" });
    expect(select.value).toBe("api");
    expect(options).not.toContain("claude");
  });
});
