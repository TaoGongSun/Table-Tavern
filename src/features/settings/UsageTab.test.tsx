// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { t } from "../../i18n";

const backend = vi.hoisted(() => ({ latest: null as unknown }));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string) => (cmd === "usage_report" ? report(backend.latest) : null)),
}));

import { UsageTab } from "./UsageTab";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const row = {
  source: "agy",
  model: "gemini",
  rounds: 2,
  cache_rounds: 2,
  prompt_tokens: 30_000,
  cached_tokens: 0,
  output_tokens: 100,
  hit_rate: 0,
  observed_rounds: 2,
  observed_prompt_tokens: 30_000,
  cost_usd: null,
  cost_partial: false,
  saved_tokens: 0,
  priced_tokens: 0,
  saved_usd: null,
  saved_partial: false,
  unreported: 0,
  in_use: true,
};

function report(latest: unknown) {
  return {
    worlds: [{ id: "w1", name: "桌", rounds: 2 }],
    rows: [row],
    total: row,
    ping: { ...row, rounds: 0 },
    caches: [],
    events: 0,
    latest,
  };
}

let host: HTMLDivElement;
let root: Root;

beforeEach(() => {
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
});

afterEach(() => {
  act(() => root.unmount());
  host.remove();
});

async function render(latest: unknown) {
  backend.latest = latest;
  await act(async () => root.render(<UsageTab currentWorld="w1" />));
  return host.querySelector(".usage-latest");
}

describe("UsageTab 最近一輪", () => {
  it("有回報、值為 0：只說這次沒有快取，不推測原因、不亮紅燈", async () => {
    const line = await render({
      ts: "2026-10-06 12:00:00",
      mode: "resume",
      cache: "zero",
      cache_reason: null,
      event: null,
      reason: null,
      reported: true,
    });
    expect(line?.textContent).toContain(t("usageCacheZero"));
    expect(line?.textContent).not.toContain(t("usageCacheReasonSkipped"));
    expect(line?.classList.contains("usage-bad")).toBe(false);
  });

  it("首輪：新桌／新線，配上第一次開線", async () => {
    const line = await render({
      ts: "2026-10-06 12:00:00",
      mode: "resume",
      cache: "not-expected",
      cache_reason: null,
      event: null,
      reason: "first-turn",
      reported: true,
    });
    expect(line?.textContent).toContain(t("usageCacheNotExpected"));
    expect(line?.textContent).toContain(t("usageCacheNotExpectedWhy"));
    expect(line?.textContent).toContain(t("usageReasonFirstTurn"));
  });

  it("claude 讀寫皆 0 照舊紅燈並說明 CLI 毛病", async () => {
    const line = await render({
      ts: "2026-10-06 12:00:00",
      mode: "resume",
      cache: "zero",
      cache_reason: "skipped",
      event: null,
      reason: null,
      reported: true,
    });
    expect(line?.textContent).toContain(t("usageCacheReasonSkipped"));
    expect(line?.classList.contains("usage-bad")).toBe(true);
  });
});
