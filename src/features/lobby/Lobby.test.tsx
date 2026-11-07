// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import { t } from "../../i18n";
import type { WorldMeta } from "../../shared/contracts/backend-contracts";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async () => null) }));
vi.mock("../vn/work/useWorkDrop", () => ({ useWorkDrop: () => {} }));
vi.mock("./useTableCover", () => ({ useTableCover: () => ({ ref: { current: null }, images: null }) }));

import { Lobby } from "./Lobby";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const world = (id: string, table_kind: "chat" | "vn" = "chat", setup: WorldMeta["setup"] = null): WorldMeta => ({
  id,
  name: id,
  read_only: false,
  needs_repair: false,
  table_kind,
  replay: false,
  setup,
});

const worlds = [
  world("chat-1"),
  world("vn-1", "vn", "story"),
  world("chat-2"),
  world("vn-2", "vn"),
  world("chat-3"),
  world("chat-4"),
  world("chat-5"),
  world("chat-6"),
];

describe("Lobby table wall", () => {
  let root: Root | null = null;
  let host: HTMLDivElement | null = null;

  afterEach(() => {
    act(() => root?.unmount());
    host?.remove();
    root = null;
    host = null;
  });

  function render(tables: WorldMeta[], rooms: Record<string, string> = {}, activeRoom: string | null = null) {
    if (!host) {
      host = document.createElement("div");
      document.body.appendChild(host);
      root = createRoot(host);
    }
    act(() => root?.render(
      <Lobby
        worlds={tables}
        busy={false}
        notices={null}
        appVersion={null}
        updateDot={false}
        onOpenSettings={() => {}}
        onOpenVersions={() => {}}
        onNewTable={() => {}}
        onGenerateTable={() => {}}
        onVnTable={() => {}}
        onImportWork={() => {}}
        onPickWork={() => {}}
        onEnterTable={() => {}}
        onRenameTable={() => {}}
        onDeleteTable={() => {}}
        rooms={rooms}
        activeRoom={activeRoom}
        onOpenRoom={() => {}}
        onOpenRoomFrom={() => {}}
        onReopenRoom={() => {}}
        onEndRoom={() => {}}
      />,
    ));
  }

  const tableNames = () => [...host!.querySelectorAll(".table-card-name")].map((node) => node.textContent);
  const ribbon = () => host!.querySelector(".table-cover-ribbon");
  const filters = () => [...host!.querySelectorAll<HTMLButtonElement>(".lobby-filter")];
  const selectFilter = (name: string) => {
    const button = filters().find((item) => item.textContent === name)!;
    act(() => button.click());
  };

  it("pins the active room, gives its ribbon priority, then restores the original order", () => {
    render(worlds, { "vn-1": "ABCD" }, "vn-1");
    expect(tableNames()).toEqual(["vn-1", "chat-1", ...worlds.slice(2).map((item) => item.name)]);
    expect(ribbon()?.getAttribute("data-state")).toBe("room");
    expect(ribbon()?.textContent).toBe(t("lobbyRibbonRoom", { code: "ABCD" }));
    expect(host!.querySelectorAll('.table-cover-ribbon[data-state="room"]')).toHaveLength(1);
    render(worlds, { "vn-1": "ABCD" }, null);
    expect(tableNames()).toEqual(worlds.map((item) => item.name));
    expect(host!.querySelector('.table-cover-ribbon[data-state="room"]')).toBeNull();
    expect(ribbon()?.getAttribute("data-state")).toBe("setup");
  });

  it("shows filter chips at eight tables and hides them below eight", () => {
    render(worlds.slice(0, 7));
    expect(filters()).toHaveLength(0);
    render(worlds, { "chat-2": "ABCD" });
    expect(filters().map((item) => item.textContent)).toEqual([
      t("lobbyFilterAll"), t("lobbyFilterChat"), t("lobbyFilterVn"), t("lobbyFilterRoomed"),
    ]);
    render(worlds.slice(0, 7));
    expect(filters()).toHaveLength(0);
  });

  it("hides chips whose bucket is empty, and the whole row when only one bucket is left", () => {
    render(worlds);
    expect(filters().map((item) => item.textContent)).toEqual([t("lobbyFilterAll"), t("lobbyFilterChat"), t("lobbyFilterVn")]);
    render(Array.from({ length: 8 }, (_, index) => world(`solo-${index}`)));
    expect(filters()).toHaveLength(0);
  });

  it("filters by kind or past room while preserving active-room priority", () => {
    render(worlds, { "chat-3": "ABCD", "vn-2": "EFGH" }, "chat-3");
    selectFilter(t("lobbyFilterVn"));
    expect(tableNames()).toEqual(["vn-1", "vn-2"]);
    selectFilter(t("lobbyFilterRoomed"));
    expect(tableNames()).toEqual(["chat-3", "vn-2"]);
    selectFilter(t("lobbyFilterChat"));
    expect(tableNames()).toEqual(["chat-3", "chat-1", "chat-2", "chat-4", "chat-5", "chat-6"]);
    selectFilter(t("lobbyFilterAll"));
    expect(tableNames()).toEqual(["chat-3", ...worlds.filter((item) => item.id !== "chat-3").map((item) => item.name)]);
  });

  it("keeps the four entry cards in intent order and shows back to room while active", () => {
    render(worlds, { "chat-3": "ABCD" }, "chat-3");
    const titles = [...host!.querySelectorAll(".lobby-new-text b")].map((node) => node.textContent);
    expect(titles).toEqual([t("newTable"), t("genTableBtn"), t("vnEntryTitle"), t("roomBack")]);
    render(worlds);
    expect(host!.querySelectorAll(".lobby-new-text b")[3].textContent).toBe(t("roomOpen"));
  });
});
