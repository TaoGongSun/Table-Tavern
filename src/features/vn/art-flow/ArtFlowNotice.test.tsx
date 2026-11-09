// @vitest-environment happy-dom
import { act } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import { t } from "../../../i18n";
import { ArtFlowNotice } from "./ArtFlowNotice";

const backend = vi.hoisted(() => ({ calls: [] as string[], started: false }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async (command) => {
  backend.calls.push(command);
  if (command === "read_config") return { preferences: { vn_sprite_source: "codex", "cli_connected:agy": true } };
  if (command === "detect_clis") return [{ id: "agy" }];
  if (command === "vn_art_start") backend.started = true;
  if (command === "vn_art_status") return { art: { started: backend.started, appeared: [] }, source: "agy", ready: 1, total: 3, backgrounds: 1, faces: 8, pause: null, pending: 0 };
  return null;
}) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));
(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
it("preselects the only connected source without generating, then starts only on click", async () => {
  const host = document.createElement("div"); const root = createRoot(host);
  await act(async () => root.render(<ArtFlowNotice worldId="w" />));
  expect(host.querySelectorAll("option")).toHaveLength(1);
  expect(host.querySelector("select")?.value).toBe("agy");
  expect(backend.calls).not.toContain("vn_art_start");
  expect(host.textContent).toContain(t("vnArtEstimate", { source: "Gemini CLI", n: 2, bg: 1, faces: 8 }));
  await act(async () => [...host.querySelectorAll("button")].find((b) => b.textContent === t("vnArtBegin"))!.click());
  expect(backend.calls.filter((name) => name === "vn_art_start")).toHaveLength(1);
  const startAt = backend.calls.indexOf("vn_art_start");
  expect(backend.calls[startAt - 1]).toBe("update_config");
  await act(async () => root.unmount());
});
