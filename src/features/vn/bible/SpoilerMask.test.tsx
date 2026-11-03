// @vitest-environment happy-dom
// 劇透遮罩：鎖住時蓋住內容、重骰後仍不顯示；解鎖要二次確認、同一桌的遮罩一起打開。
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { t } from "../../../i18n";

const backend = vi.hoisted(() => ({ locked: true, played: false, calls: [] as string[] }));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (command: string, args: Record<string, unknown>) => {
    backend.calls.push(command);
    if (command === "vn_spoiler_set") backend.locked = args.locked as boolean;
    if (command === "vn_bible_read") {
      return {
        version: 1,
        spoiler_lock: backend.locked,
        bible: null,
        flagValues: {},
        revealed: [],
        played: backend.played,
        names: {},
      };
    }
    return null;
  }),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ confirm: vi.fn(async () => true) }));

import { confirm } from "@tauri-apps/plugin-dialog";
import { SpoilerMask } from "./SpoilerMask";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

let root: Root;
let host: HTMLElement;

beforeEach(() => {
  backend.locked = true;
  backend.played = false;
  backend.calls = [];
  host = document.createElement("div");
  document.body.appendChild(host);
  root = createRoot(host);
});

afterEach(() => {
  act(() => root.unmount());
  host.remove();
});

async function settle() {
  for (let i = 0; i < 4; i += 1) {
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });
  }
}

const button = (label: string) =>
  [...host.querySelectorAll("button")].find((item) => item.textContent === label);

describe("SpoilerMask", () => {
  it("hides content, rerolls without showing it, and unlocks every mask of the table after confirming", async () => {
    const onReroll = vi.fn(async () => {});
    await act(async () =>
      root.render(
        <>
          <SpoilerMask worldId="w1" onReroll={onReroll}>
            <p className="secret">A</p>
          </SpoilerMask>
          <SpoilerMask worldId="w1">
            <p className="secret">B</p>
          </SpoilerMask>
        </>,
      ),
    );
    await settle();
    expect(host.querySelectorAll(".secret")).toHaveLength(0);
    await act(async () => button(t("vnSpoilerReroll"))!.click());
    await settle();
    expect(onReroll).toHaveBeenCalledTimes(1);
    expect(host.textContent).toContain(t("vnSpoilerRerolled"));
    expect(host.querySelectorAll(".secret")).toHaveLength(0);

    vi.mocked(confirm).mockResolvedValueOnce(false);
    await act(async () => button(t("vnSpoilerUnlock"))!.click());
    await settle();
    expect(backend.calls).not.toContain("vn_spoiler_set");
    await act(async () => button(t("vnSpoilerUnlock"))!.click());
    await settle();
    expect([...host.querySelectorAll(".secret")].map((item) => item.textContent)).toEqual(["A", "B"]);
  });

  it("played tables refuse the reroll with the new-table hint", async () => {
    backend.played = true;
    const onReroll = vi.fn(async () => {});
    await act(async () =>
      root.render(
        <SpoilerMask worldId="w2" onReroll={onReroll}>
          <p>secret</p>
        </SpoilerMask>,
      ),
    );
    await settle();
    await act(async () => button(t("vnSpoilerReroll"))!.click());
    expect(onReroll).not.toHaveBeenCalled();
    expect(host.textContent).toContain(t("vnBiblePlayed"));
  });
});
