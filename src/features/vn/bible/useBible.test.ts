import { beforeEach, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";
import { acknowledgeBibleFailure, bibleDraft, generateBible } from "./useBible";

beforeEach(() => {
  vi.mocked(invoke).mockReset();
});

it("joins an in-flight generate instead of sending a second one", async () => {
  let release!: () => void;
  vi.mocked(invoke).mockImplementation((command: string) => {
    if (command === "vn_bible_generate") return new Promise<void>((ok) => (release = ok));
    return Promise.resolve(null);
  });
  const first = generateBible("cave");
  expect(generateBible("cave")).toBe(first);
  expect(bibleDraft("cave")).toEqual({ state: "running" });
  expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === "vn_bible_generate")).toHaveLength(1);
  release();
  await first;
  expect(bibleDraft("cave")).toBeNull();
});

it("keeps a failure until the wizard acknowledges it", async () => {
  vi.mocked(invoke).mockImplementation((command: string) => {
    if (command === "vn_bible_generate") return Promise.reject(new Error("VN_BIBLE_FAILED"));
    return Promise.resolve(null);
  });
  await expect(generateBible("cave")).rejects.toThrow("VN_BIBLE_FAILED");
  expect(bibleDraft("cave")).toMatchObject({ state: "failed" });
  acknowledgeBibleFailure("cave");
  expect(bibleDraft("cave")).toBeNull();
});
