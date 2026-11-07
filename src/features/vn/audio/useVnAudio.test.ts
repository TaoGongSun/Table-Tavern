// @vitest-environment happy-dom
// 切桌不能接著播上一桌的主題曲；標題畫面不能把音效登記成已播。
import { act, createElement, useState } from "react";
import { createRoot, type Root } from "react-dom/client";
import { beforeEach, expect, it, vi } from "vitest";
import type { VnCell } from "../../../shared/contracts/backend-contracts";
import { flatten, type FlatCell } from "../vn-cells";
import { useVnAudio } from "./useVnAudio";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

interface AudioFile {
  kind: string;
  key: string;
  path: string;
}

const gate = vi.hoisted(() => {
  const waiters = new Map<string, Array<(view: { files: AudioFile[] }) => void>>();
  return {
    waiters,
    release(worldId: string, files: AudioFile[]) {
      const pending = waiters.get(worldId) ?? [];
      waiters.set(worldId, []);
      for (const resolve of pending) resolve({ files });
    },
  };
});

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn((command: string, args: { worldId: string }) => {
    if (command !== "vn_audio_read") return Promise.resolve(null);
    return new Promise((resolve) => {
      const pending = gate.waiters.get(args.worldId) ?? [];
      pending.push(resolve);
      gate.waiters.set(args.worldId, pending);
    });
  }),
  convertFileSrc: (path: string) => `asset:${path}`,
}));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));

class FakeAudio {
  src = "";
  loop = false;
  volume = 1;
  currentTime = 0;
  paused = true;
  plays = 0;
  constructor(src?: string) {
    if (src) this.src = src;
    made.push(this);
  }
  play() {
    this.plays += 1;
    this.paused = false;
    return Promise.resolve();
  }
  pause() {
    this.paused = true;
  }
  removeAttribute(name: string) {
    if (name === "src") this.src = "";
  }
  addEventListener() {}
}

const made: FakeAudio[] = [];

const narration = (text: string): VnCell => ({ type: "narration", text });
const sfx = (key: string): VnCell => ({ type: "sfx", query: "door", key });

function formal(cells: VnCell[]): FlatCell[] {
  return flatten([{ event_id: "e1", kind: "narration", cells }]);
}

function Probe({
  worldId,
  active,
  cells,
  flatIndex,
  muted = false,
  bgm = null,
}: {
  worldId: string;
  active: boolean;
  cells: FlatCell[];
  flatIndex: number;
  muted?: boolean;
  bgm?: string[] | null;
}) {
  const { previewSfx } = useVnAudio({
    worldId,
    bgm,
    cells,
    flatIndex,
    lastEventKey: "e1",
    bgmVolume: 70,
    sfxVolume: 100,
    active,
    muted,
  });
  return createElement("button", { type: "button", onClick: () => previewSfx() }, "preview");
}

function Switcher({
  worldId,
  active,
  cells,
  flatIndex,
  muted = false,
}: {
  worldId: string;
  active: boolean;
  cells: FlatCell[];
  flatIndex: number;
  muted?: boolean;
}) {
  return createElement(Probe, { worldId, active, cells, flatIndex, muted });
}

async function mount(node: ReturnType<typeof createElement>) {
  const host = document.createElement("div");
  const root = createRoot(host);
  await act(async () => root.render(node));
  return { root, host };
}

async function renderAt(root: Root, node: ReturnType<typeof createElement>) {
  await act(async () => root.render(node));
}

beforeEach(() => {
  made.length = 0;
  gate.waiters.clear();
  vi.stubGlobal("Audio", FakeAudio);
});

it("plays the next table's theme only after that table's catalog loads", async () => {
  const props = { active: true, cells: [] as FlatCell[], flatIndex: 0 };
  const { root } = await mount(createElement(Switcher, { worldId: "A", ...props }));
  await act(async () => {
    gate.release("A", [{ kind: "bgm", key: "theme", path: "A/theme.mp3" }]);
  });
  expect(made.some((audio) => audio.src === "asset:A/theme.mp3" && !audio.paused)).toBe(true);

  await renderAt(root, createElement(Switcher, { worldId: "B", ...props }));
  expect(made.some((audio) => audio.src.includes("B/theme"))).toBe(false);
  expect(made.some((audio) => audio.src.includes("A/theme") && !audio.paused)).toBe(false);

  await act(async () => {
    gate.release("B", [{ kind: "bgm", key: "theme", path: "B/theme.mp3" }]);
  });
  expect(made.filter((audio) => audio.src === "asset:B/theme.mp3" && !audio.paused)).toHaveLength(1);
});

it("does not play or mark a sound effect until playback is active", async () => {
  const cells = formal([sfx("door"), narration("a")]);
  const files = [{ kind: "sfx", key: "door", path: "sfx-door.mp3" }];
  function Harness({ active, generation }: { active: boolean; generation: number }) {
    const [shown] = useState(cells);
    return createElement(Probe, {
      worldId: "A",
      active,
      cells: generation === 0 ? shown : cells.slice(),
      flatIndex: 1,
    });
  }
  const { root } = await mount(createElement(Harness, { active: false, generation: 0 }));
  await act(async () => {
    gate.release("A", files);
  });
  expect(made.some((audio) => audio.src.includes("sfx-door"))).toBe(false);

  await renderAt(root, createElement(Harness, { active: true, generation: 0 }));
  const playing = made.filter((audio) => audio.src === "asset:sfx-door.mp3");
  expect(playing).toHaveLength(1);
  expect(playing[0].plays).toBe(1);

  await renderAt(root, createElement(Harness, { active: true, generation: 1 }));
  expect(made.filter((audio) => audio.src === "asset:sfx-door.mp3")).toHaveLength(1);
});

it("does not play effects passed while skipping and plays the one beside the stop", async () => {
  const cells = formal([sfx("passed"), narration("mid"), sfx("here"), narration("land")]);
  const files = [
    { kind: "sfx", key: "passed", path: "sfx-passed.mp3" },
    { kind: "sfx", key: "here", path: "sfx-here.mp3" },
  ];
  const { root } = await mount(
    createElement(Probe, { worldId: "A", active: false, cells, flatIndex: 1 }),
  );
  await act(async () => {
    gate.release("A", files);
  });
  expect(made.some((audio) => audio.src.includes("sfx-"))).toBe(false);

  await renderAt(root, createElement(Probe, { worldId: "A", active: false, cells, flatIndex: 3 }));
  expect(made.some((audio) => audio.src.includes("sfx-"))).toBe(false);

  await renderAt(root, createElement(Probe, { worldId: "A", active: true, cells, flatIndex: 3 }));
  const played = made.filter((audio) => audio.src.startsWith("asset:sfx-"));
  expect(played.map((audio) => audio.src)).toEqual(["asset:sfx-here.mp3"]);
  expect(played[0].plays).toBe(1);
});

it("mutes the playing track and restores the slider level", async () => {
  const frames: FrameRequestCallback[] = [];
  const raf = vi.spyOn(window, "requestAnimationFrame").mockImplementation((fn) => {
    frames.push(fn);
    return frames.length;
  });
  const props = { active: true, cells: [] as FlatCell[], flatIndex: 0 };
  const { root } = await mount(createElement(Switcher, { worldId: "A", ...props, muted: false }));
  await act(async () => {
    gate.release("A", [{ kind: "bgm", key: "theme", path: "A/theme.mp3" }]);
  });
  const playing = () => made.find((audio) => audio.src === "asset:A/theme.mp3" && !audio.paused);
  expect(playing()?.volume).toBe(0.7);

  await renderAt(root, createElement(Switcher, { worldId: "A", ...props, muted: true }));
  expect(playing()?.volume).toBe(0);

  await renderAt(root, createElement(Switcher, { worldId: "A", ...props, muted: false }));
  expect(playing()?.volume).toBe(0.7);
  raf.mockRestore();
});

it("previewSfx plays one landed effect and does nothing without one", async () => {
  const props = { active: true, cells: [] as FlatCell[], flatIndex: 0 };
  const withFile = await mount(createElement(Switcher, { worldId: "A", ...props }));
  await act(async () => {
    gate.release("A", [
      { kind: "bgm", key: "theme", path: "A/theme.mp3" },
      { kind: "sfx", key: "door", path: "sfx-door.mp3" },
    ]);
  });
  const before = made.length;
  await act(async () => {
    withFile.host.querySelector("button")?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
  });
  const previews = made.slice(before).filter((audio) => audio.src === "asset:sfx-door.mp3");
  expect(previews).toHaveLength(1);
  expect(previews[0].plays).toBe(1);

  made.length = 0;
  const without = await mount(createElement(Switcher, { worldId: "B", ...props }));
  await act(async () => {
    gate.release("B", [{ kind: "bgm", key: "theme", path: "B/theme.mp3" }]);
  });
  const count = made.length;
  await act(async () => {
    without.host.querySelector("button")?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
  });
  expect(made).toHaveLength(count);
});

it("zeroes both tracks when muted during a crossfade and restores the fade level", async () => {
  const frames: FrameRequestCallback[] = [];
  const raf = vi.spyOn(window, "requestAnimationFrame").mockImplementation((fn) => {
    frames.push(fn);
    return frames.length;
  });
  const clock = vi.spyOn(performance, "now").mockReturnValue(5_000);
  const props = { active: true, cells: [] as FlatCell[], flatIndex: 0 };
  const { root } = await mount(createElement(Probe, { worldId: "A", ...props }));
  await act(async () => {
    gate.release("A", [
      { kind: "bgm", key: "theme", path: "A/theme.mp3" },
      { kind: "bgm", key: "dark", path: "A/dark.mp3" },
    ]);
  });
  await act(async () => {
    frames[frames.length - 1]?.(6_000);
  });

  await renderAt(root, createElement(Probe, { worldId: "A", ...props, bgm: ["Dark"] }));
  await act(async () => {
    frames[frames.length - 1]?.(5_500);
  });
  const theme = () => made.find((audio) => audio.src === "asset:A/theme.mp3");
  const dark = () => made.find((audio) => audio.src === "asset:A/dark.mp3" && !audio.paused);
  expect(theme()?.volume).toBeCloseTo(0.35);
  expect(dark()?.volume).toBeCloseTo(0.35);

  await renderAt(root, createElement(Probe, { worldId: "A", ...props, bgm: ["Dark"], muted: true }));
  expect(theme()?.volume).toBe(0);
  expect(dark()?.volume).toBe(0);
  await act(async () => {
    frames[frames.length - 1]?.(5_500);
  });
  expect(theme()?.volume).toBe(0);
  expect(dark()?.volume).toBe(0);

  await renderAt(root, createElement(Probe, { worldId: "A", ...props, bgm: ["Dark"], muted: false }));
  await act(async () => {
    frames[frames.length - 1]?.(5_500);
  });
  expect(theme()?.volume).toBeCloseTo(0.35);
  expect(dark()?.volume).toBeCloseTo(0.35);
  raf.mockRestore();
  clock.mockRestore();
});

it("zeroes a preview when muted and pauses it on unmount", async () => {
  const props = { active: true, cells: [] as FlatCell[], flatIndex: 0 };
  const mounted = await mount(createElement(Probe, { worldId: "A", ...props }));
  await act(async () => {
    gate.release("A", [
      { kind: "bgm", key: "theme", path: "A/theme.mp3" },
      { kind: "sfx", key: "door", path: "sfx-door.mp3" },
    ]);
  });
  await act(async () => {
    mounted.host.querySelector("button")?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
  });
  const preview = made.find((audio) => audio.src === "asset:sfx-door.mp3");
  expect(preview?.plays).toBe(1);

  await renderAt(mounted.root, createElement(Probe, { worldId: "A", ...props, muted: true }));
  expect(preview?.volume).toBe(0);

  await act(async () => {
    mounted.root.unmount();
  });
  expect(preview?.paused).toBe(true);
});
