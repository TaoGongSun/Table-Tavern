// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { t } from "../../i18n";

const backend = vi.hoisted(() => ({ latest: null as unknown, total: null as unknown }));

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
  estimated_rounds: 0,
  unreported: 0,
  in_use: true,
};

function report(latest: unknown) {
  return {
    worlds: [{ id: "w1", name: "桌", rounds: 2 }],
    rows: [row],
    total: backend.total ?? row,
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

async function render(latest: unknown, total: unknown = null) {
  backend.latest = latest;
  backend.total = total;
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

describe("UsageTab 已省", () => {
  const priced = { ...row, source: "claude", priced_tokens: 30_000, saved_tokens: 15_000, saved_usd: 0.05 };

  it("有輪次用估計係數算：第一眼標「約」", async () => {
    await render(null, { ...priced, estimated_rounds: 1 });
    const headline = host.querySelector(".usage-headline")?.textContent ?? "";
    expect(headline).toContain(t("usageSavedHeadlineApprox", { pct: "50" }));
  });

  it("全是新紀錄：不標「約」", async () => {
    await render(null, priced);
    const headline = host.querySelector(".usage-headline")?.textContent ?? "";
    expect(headline).toContain(t("usageSavedHeadline", { pct: "50" }));
    expect(headline).not.toContain(t("usageSavedHeadlineApprox", { pct: "50" }));
  });
});
