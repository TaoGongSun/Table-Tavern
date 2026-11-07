// @vitest-environment happy-dom
import { act } from "react";
import { createRoot } from "react-dom/client";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { t } from "../../i18n";
import type { AudioEntry, VnEndings } from "../../shared/contracts/backend-contracts";
import { EndingScreen } from "./EndingScreen";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const backend = vi.hoisted(() => ({
  entries: [] as AudioEntry[],
  fail: false,
  endings: { endings: [{ id: "good", title: "好結局", reached: true }], current: "good" } as VnEndings,
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (command: string) => {
    if (command === "vn_audio_read") {
      if (backend.fail) throw new Error("boom");
      return { files: [], entries: backend.entries };
    }
    if (command === "vn_endings") return backend.endings;
    return null;
  }),
}));

const settle = () => act(async () => new Promise((resolve) => setTimeout(resolve, 0)));

function clip(kind: "bgm" | "sfx", author: string, license: string, source_url: string): AudioEntry {
  return { kind, key: author, title: author, author, license, source_url };
}

async function render() {
  const host = document.createElement("div");
  await act(async () => {
    createRoot(host).render(
      <EndingScreen worldId="w" endingId="good" disabled={false} onReplay={() => {}} onNewPlaythrough={() => {}} onExport={() => {}} onEndings={() => {}} onTitle={() => {}} />,
    );
  });
  await settle();
  return host;
}

const lineText = (host: HTMLElement) => [...host.querySelectorAll(".vn-credits-lines p")].map((node) => node.textContent);

describe("EndingScreen credits", () => {
  beforeEach(() => {
    backend.entries = [];
    backend.fail = false;
  });

  it("shows one line per kind under the reached count and above the buttons", async () => {
    backend.entries = [
      clip("bgm", "Kevin MacLeod", "https://creativecommons.org/licenses/by/4.0/", "https://www.incompetech.com/a"),
      clip("sfx", "Ann", "Creative Commons 0", "https://freesound.org/people/ann"),
    ];
    const host = await render();
    expect(lineText(host)).toEqual([
      t("vnCreditsLineMusic", { authors: "Kevin MacLeod", hosts: "incompetech.com", licenses: "CC BY 4.0" }),
      t("vnCreditsLineSfx", { authors: "Ann", hosts: "freesound.org", licenses: "CC0" }),
    ]);
    const order = [...host.querySelector(".vn-ending-screen")!.children].map((node) => node.className);
    expect(order.indexOf("vn-credits-lines")).toBe(order.indexOf("vn-chapter-note") + 1);
    expect(order.indexOf("vn-chapter-actions")).toBe(order.indexOf("vn-credits-lines") + 1);
  });

  it("shows a single music line when there are no sound effects", async () => {
    backend.entries = [clip("bgm", "Kevin MacLeod", "Attribution", "https://incompetech.com/a")];
    const host = await render();
    expect(lineText(host)).toEqual([
      t("vnCreditsLineMusic", { authors: "Kevin MacLeod", hosts: "incompetech.com", licenses: "CC BY" }),
    ]);
  });

  it("omits the block when there are no entries or the read fails", async () => {
    expect((await render()).querySelector(".vn-credits-lines")).toBeNull();
    backend.fail = true;
    backend.entries = [clip("bgm", "Kevin MacLeod", "Attribution", "https://incompetech.com/a")];
    expect((await render()).querySelector(".vn-credits-lines")).toBeNull();
  });
});
