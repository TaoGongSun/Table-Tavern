import { describe, expect, it, vi } from "vitest";
import { t } from "../../i18n";
import { gameMenuItems, gameMoreItems, type GameMenuItem } from "./GameMenu";

function actions(replay: boolean) {
  return {
    replay,
    busy: false,
    nextDisabled: false,
    open: vi.fn(),
    close: vi.fn(),
    openMore: vi.fn(),
    onTitle: vi.fn(),
    onNextChapter: vi.fn(),
    onShowChat: vi.fn(),
    onOpenScript: vi.fn(),
    onExportCard: vi.fn(),
    onExportWork: vi.fn(),
    onHide: vi.fn(),
    onLobby: vi.fn(),
  };
}

const labels = (items: GameMenuItem[]) => items.map((item) => item.label);
const sections = (items: GameMenuItem[]) => items.map((item) => item.section === true);

describe("game menu", () => {
  it("groups eight items on a play table", () => {
    const items = gameMenuItems(actions(false));
    expect(labels(items)).toEqual([
      t("vnSaves"),
      t("vnProgressPanel"),
      t("vnGallery"),
      t("vnNextChapter"),
      t("vnTitleSettings"),
      t("vnMenuMore"),
      t("vnBackToTitle"),
      t("backToLobby"),
    ]);
    expect(sections(items)).toEqual([false, false, false, true, true, false, true, false]);
    const play = actions(false);
    gameMenuItems(play).find((item) => item.key === "progress")!.onSelect();
    expect(play.open).toHaveBeenCalledWith("progress");
  });

  it("keeps chapter select, endings and hide on a replay table, without end-chapter or more", () => {
    const items = gameMenuItems(actions(true));
    const names = labels(items);
    expect(names).toEqual([
      t("vnSaves"),
      t("vnWorkChapterSelect"),
      t("vnEndingsList"),
      t("vnGallery"),
      t("vnTitleSettings"),
      t("vnHideUi"),
      t("vnBackToTitle"),
      t("backToLobby"),
    ]);
    expect(names).not.toContain(t("vnNextChapter"));
    expect(names).not.toContain(t("vnMenuMore"));
    expect(names).not.toContain(t("vnProgressPanel"));
    expect(names).not.toContain(t("vnBibleTab"));
    expect(sections(items)).toEqual([false, false, false, false, true, false, true, false]);
  });

  it("puts hide first in the more submenu and the script right after chat view", () => {
    const play = actions(false);
    const items = gameMoreItems(play);
    expect(labels(items)).toEqual([
      t("vnHideUi"),
      t("vnShowChat"),
      t("vnBibleTab"),
      t("vnCardExport"),
      t("vnWorkExport"),
    ]);
    items.find((item) => item.key === "script")!.onSelect();
    expect(play.onOpenScript).toHaveBeenCalledTimes(1);
  });
});
