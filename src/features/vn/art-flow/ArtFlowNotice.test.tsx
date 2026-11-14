// @vitest-environment happy-dom
import { act } from "react";
import { createRoot } from "react-dom/client";
import { beforeEach, expect, it, vi } from "vitest";
import { t } from "../../../i18n";
import { CLI_CHANGED_EVENT } from "../../../shared/ui/cli-update";
import { ArtFlowNotice } from "./ArtFlowNotice";

const backend = vi.hoisted(() => ({ calls: [] as string[], started: false, pause: null as { reset: number | null } | null, each: 4 as number | null, ready: 1, failed: 0, loading: false, source: "agy" }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async (command) => {
  backend.calls.push(command);
  if (command === "read_config") return { preferences: { vn_sprite_source: "codex", [`cli_connected:${backend.source}`]: true } };
  if (command === "detect_clis") return [{ id: backend.source }];
  if (command === "vn_art_start") backend.started = true;
  if (command === "vn_art_status" && backend.loading) return new Promise(() => {});
  if (command === "vn_art_status") return { art: { started: backend.started, appeared: [] }, source: backend.source, ready: backend.ready, failed: backend.failed, total: 3, backgrounds: 1, faces: 8, facesPerCharacter: backend.each, pause: backend.pause, pending: 7 };
  return null;
}) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));
(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
beforeEach(() => { backend.calls = []; backend.started = false; backend.pause = null; backend.each = 4; backend.ready = 1; backend.failed = 0; backend.loading = false; backend.source = "agy"; });
it("preselects the only connected source without generating, then starts only on click", async () => {
  const host = document.createElement("div"); const root = createRoot(host);
  await act(async () => root.render(<ArtFlowNotice worldId="w" />));
  expect(host.querySelectorAll("option")).toHaveLength(1);
  expect(host.querySelector("select")?.value).toBe("agy");
  expect(backend.calls).not.toContain("vn_art_start");
  const controls = host.querySelector(".vn-art-controls")!;
  expect(controls.querySelector("select")).not.toBeNull();
  expect(controls.querySelector("button")?.textContent).toBe(t("vnArtBegin"));
  expect(host.textContent).toContain(t("vnArtExpressionsEach", { each: 4, n: 8 }));
  expect(host.textContent).toContain(t("vnArtEstimate", { source: "Gemini CLI", n: 2, bg: 1 }));
  await act(async () => [...host.querySelectorAll("button")].find((b) => b.textContent === t("vnArtBegin"))!.click());
  expect(backend.calls.filter((name) => name === "vn_art_start")).toHaveLength(1);
  const startAt = backend.calls.indexOf("vn_art_start");
  expect(backend.calls[startAt - 1]).toBe("update_config");
  await act(async () => root.unmount());
});

it("shows paused progress and reports quota wait to the wizard rows", async () => {
  backend.started = true;
  backend.pause = { reset: Date.now() + 4 * 3_600_000 };
  const onPause = vi.fn();
  const host = document.createElement("div"); const root = createRoot(host);
  await act(async () => root.render(<ArtFlowNotice worldId="w" onPause={onPause} />));
  expect(host.textContent).toContain(t("vnArtStopped", { n: 1, total: 4 }));
  expect(host.textContent).toContain(t("vnQuotaHours", { n: 4 }));
  expect(onPause).toHaveBeenCalledWith(true);
  expect(host.textContent).toContain(t("vnArtPaused", { n: 3, time: t("vnQuotaHours", { n: 4 }) }));
  expect(host.textContent).toContain(t("vnArtQuotaShort", { time: t("vnQuotaHours", { n: 4 }) }));
  expect(host.querySelector("select")).not.toBeNull();
  await act(async () => root.unmount());
});
it("keeps title status to one summary and retry, with source changes left in the gallery", async () => {
  backend.started = true; backend.pause = { reset: Date.now() + 4 * 3_600_000 };
  const host = document.createElement("div"); const root = createRoot(host);
  await act(async () => root.render(<ArtFlowNotice worldId="w" compact />));
  expect(host.querySelector("select")).toBeNull();
  expect(host.querySelectorAll("button")).toHaveLength(1);
  expect(host.textContent).toContain(t("vnArtQuotaShort", { time: t("vnQuotaHours", { n: 4 }) }));
  expect(backend.calls).not.toContain("read_config");
  backend.pause = null;
  await act(async () => root.unmount());
});
it("uses only the total when counts differ, and does not hard-code four", async () => {
  const host = document.createElement("div");
  backend.each = null;
  let root = createRoot(host);
  await act(async () => root.render(<ArtFlowNotice worldId="w" />));
  expect(host.textContent).toContain(t("vnArtExpressionsTotal", { n: 8 }));
  await act(async () => root.unmount());
  backend.each = 5; root = createRoot(host);
  await act(async () => root.render(<ArtFlowNotice worldId="w" />));
  expect(host.textContent).toContain(t("vnArtExpressionsEach", { each: 5, n: 8 }));
  await act(async () => root.unmount());
});

it.each([
  [false, 1, 0, "vnArtNoSprites"], [true, 1, 0, "vnArtPreparing"],
  [true, 3, 0, null], [false, 3, 0, null], [true, 3, 1, "vnArtFailed"],
] as const)("compact title: started=%s ready=%s failed=%s", async (started, ready, failed, key) => {
  Object.assign(backend, { started, ready, failed });
  const host = document.createElement("div"); const root = createRoot(host);
  await act(async () => root.render(<ArtFlowNotice worldId="w" compact />));
  expect(host.textContent).toBe(key ? t(key, { n: key === "vnArtFailed" ? failed : ready, total: 3, panel: t("vnGallery") }) + (failed ? t("vnArtRetryNow") : "") : "");
  if (failed) {
    await act(async () => host.querySelector("button")!.click());
    expect(backend.calls).toContain("vn_art_retry");
  }
  await act(async () => root.unmount());
});
it("does not flash title counts before the status read", async () => {
  backend.loading = true;
  const host = document.createElement("div"); const root = createRoot(host);
  await act(async () => root.render(<ArtFlowNotice worldId="w" compact />));
  expect(host.textContent).toBe("");
  await act(async () => root.unmount());
});

it("offers a connected Grok CLI as a sprite source and names it in the estimate", async () => {
  backend.source = "grok";
  window.dispatchEvent(new Event(CLI_CHANGED_EVENT));
  const host = document.createElement("div"); const root = createRoot(host);
  await act(async () => root.render(<ArtFlowNotice worldId="w" />));
  expect(host.querySelector("select")?.value).toBe("grok");
  expect(host.textContent).toContain(t("vnArtEstimate", { source: "Grok CLI", n: 2, bg: 1 }));
  await act(async () => root.unmount());
});
