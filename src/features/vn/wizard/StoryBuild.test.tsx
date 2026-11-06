// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { MutableRefObject } from "react";
import { t } from "../../../i18n";
import { bibleDraft, generateBible } from "../bible/useBible";
import { StoryBuild } from "./StoryBuild";

const state = vi.hoisted(() => ({
  fail: false,
  gate: null as Promise<void> | null,
  bible: null as { logline: string } | null,
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (command: string) => {
    if (command === "vn_bible_read") {
      return { spoiler_lock: true, bible: state.bible, flagValues: {}, revealed: [], played: false, names: {}, cover: false };
    }
    if (command === "vn_bible_generate") {
      if (state.fail) throw new Error("VN_BIBLE_FAILED");
      return state.gate;
    }
    return null;
  }),
}));
vi.mock("./StorySpec", () => ({ StorySpec: () => null }));

import { invoke } from "@tauri-apps/api/core";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

let host: HTMLDivElement;
let root: Root;
const flushRef = { current: null } as MutableRefObject<(() => Promise<boolean>) | null>;
const generates = () => vi.mocked(invoke).mock.calls.filter(([command]) => command === "vn_bible_generate");

beforeEach(() => {
  state.fail = false;
  state.gate = null;
  state.bible = null;
  vi.mocked(invoke).mockClear();
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
    root.render(
      <StoryBuild worldId={worldId} spoilerLock onState={() => {}} onDirtyChange={() => {}} flushRef={flushRef} />,
    );
  });
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

it("waits on the in-flight generate instead of sending another", async () => {
  let release!: () => void;
  state.gate = new Promise<void>((ok) => (release = ok));
  await mount("cave");
  expect(generates()).toHaveLength(1);
  act(() => root.unmount());
  root = createRoot(host);
  await mount("cave");
  expect(generates()).toHaveLength(1);
  expect(document.querySelector('.vn-build-progress [data-state="running"]')?.textContent).toContain(t("vnBuildBible"));
  release();
  await act(async () => {
    await state.gate;
  });
});

it("shows a stored failure and does not generate until retry", async () => {
  state.fail = true;
  await generateBible("fort").catch(() => {});
  const sent = generates().length;
  await mount("fort");
  expect(generates()).toHaveLength(sent);
  expect(document.querySelector(".vn-bible-failed")?.textContent).toContain(t("vnBibleRetry"));
  expect(bibleDraft("fort")).toBeNull();
  await act(async () => {
    [...document.querySelectorAll("button")].find((button) => button.textContent === t("vnBibleRetry"))!.click();
  });
  expect(generates()).toHaveLength(sent + 1);
});

it("waits on a reroll already running while the old bible is still there", async () => {
  let release!: () => void;
  state.gate = new Promise<void>((ok) => (release = ok));
  state.bible = { logline: "舊劇本" };
  const job = generateBible("keep");
  await mount("keep");
  expect(generates()).toHaveLength(1);
  expect(document.querySelector(".vn-bible-failed")).toBeNull();
  expect(document.querySelector('.vn-build-progress [data-state="running"]')?.textContent).toContain(t("vnBuildBible"));
  release();
  await act(async () => {
    await job;
  });
});

it("shows a stored reroll failure while the old bible is still there", async () => {
  state.fail = true;
  state.bible = { logline: "舊劇本" };
  await generateBible("old").catch(() => {});
  const sent = generates().length;
  await mount("old");
  expect(generates()).toHaveLength(sent);
  expect(document.querySelector(".vn-bible-failed")?.textContent).toContain(t("vnBibleRetry"));
  expect(bibleDraft("old")).toBeNull();
});
