// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { t } from "../../../i18n";
import { bibleDraft } from "./useBible";
import { BibleTab } from "./BibleTab";

const bible = {
  logline: "舊劇本",
  style: "",
  branching: "none",
  chapter_max: 8,
  chapter_reason: "",
  chapter_lock: false,
  chapters: [],
  secrets: [],
  flags: [],
  endings: [],
};

const state = vi.hoisted(() => ({
  reject: null as ((error: unknown) => void) | null,
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (command: string) => {
    if (command === "vn_bible_read") {
      return {
        spoiler_lock: true,
        bible,
        flagValues: {},
        revealed: [],
        played: false,
        names: {},
        cover: false,
      };
    }
    if (command === "vn_bible_generate") {
      return new Promise((_ok, reject) => {
        state.reject = reject;
      });
    }
    return null;
  }),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ confirm: vi.fn(async () => true) }));

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

let host: HTMLDivElement;
let root: Root;

beforeEach(() => {
  state.reject = null;
  host = document.createElement("div");
  document.body.appendChild(host);
  root = createRoot(host);
});

afterEach(() => {
  act(() => root.unmount());
  host.remove();
});

async function mount(worldId: string) {
  await act(async () => {
    root.render(<BibleTab worldId={worldId} />);
  });
  for (let i = 0; i < 5; i++) {
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });
    if ([...host.querySelectorAll("button")].some((button) => button.textContent === t("vnBibleMaskReroll"))) return;
  }
  throw new Error(host.textContent ?? "mask reroll did not appear");
}

it("clears the drafting failure once the script tab has shown it", async () => {
  await mount("script");
  const reroll = [...host.querySelectorAll("button")].find((button) => button.textContent === t("vnBibleMaskReroll"));
  await act(async () => {
    reroll!.click();
  });
  await act(async () => {
    state.reject!(new Error("VN_BIBLE_FAILED"));
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
  expect(host.textContent).toContain(t("vnBibleFailed"));
  expect(bibleDraft("script")).toBeNull();
});
