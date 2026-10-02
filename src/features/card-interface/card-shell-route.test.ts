import { describe, expect, it } from "vitest";
import { pickCardShell } from "./card-shell-route";
import { type CardInterface } from "./interface-card";
import { type TranscriptEvent } from "../../shared/contracts/backend-contracts";

const card: CardInterface = {
  character_id: "c1",
  character_name: "卡",
  scripts: [
    {
      name: "殼",
      find_regex: "/<UI>([\\s\\S]*?)<\\/UI>/s",
      replace_string: "```html\n<!DOCTYPE html><body>$1</body>\n```",
      trim_strings: [],
      min_depth: null,
      max_depth: null,
    },
  ],
  unsupported: null,
  opening: "<UI>開場白</UI>",
};

function gm(text: string, raw?: string): TranscriptEvent {
  return { ts: "t", speaker_id: "gm", speaker_name: "GM", kind: "narration", text, ...(raw ? { raw } : {}) };
}

// 較舊那則 GM 原文畫得出來、最新那則畫不出來：退回近 10 則掃 raw 會拿到「上回合畫面」
const events: TranscriptEvent[] = [gm("上回合", "<UI>上回合畫面</UI>"), gm("最新正文沒有標籤")];
const tableTree = { World: { Time: "黃昏" } };

function pick(refactorShell: string | null, over: Partial<Parameters<typeof pickCardShell>[0]> = {}) {
  return pickCardShell({ tableMode: null, refactorShell, events, tableTree, cardInterfaces: [card], ...over });
}

describe("pickCardShell", () => {
  const skeleton = "<UI>骨架 {{World.Time}} {{本回合.正文}}</UI>";
  const emptyTable: TranscriptEvent[] = [];
  const directFirst: TranscriptEvent[] = [gm("請選擇身份", "<UI>選角開場</UI>")];
  const fallback = events;

  // interface 桌：沒骨架（null／空字串／純空白）三條退路一律關掉
  describe.each([
    ["null", null],
    ["空字串", ""],
    ["純空白", " \n\t "],
  ])("interface 桌骨架為 %s", (_label, shell) => {
    it.each([
      ["空桌", emptyTable],
      ["最新一則 direct-first", directFirst],
      ["近十則 fallback", fallback],
    ])("%s：不給面板", (_case, list) => {
      expect(pick(shell, { tableMode: "interface", events: list })).toBeNull();
    });
  });

  describe("interface 桌有效骨架", () => {
    it("空桌退回卡片開場白", () => {
      expect(pick(skeleton, { tableMode: "interface", events: emptyTable })).toContain("開場白");
    });

    it("最新 GM 原文卡腳本自己畫得出來就不塞骨架", () => {
      const shell = pick(skeleton, { tableMode: "interface", events: directFirst });
      expect(shell).toContain("選角開場");
      expect(shell).not.toContain("骨架");
    });

    it("最新正文沒有標籤：填骨架，不拿近十則的上回合畫面", () => {
      const shell = pick(skeleton, { tableMode: "interface", events: fallback });
      expect(shell).toContain("骨架 黃昏 最新正文沒有標籤");
      expect(shell).not.toContain("上回合畫面");
    });

    it("骨架沒過卡的顯示腳本：退回近 10 則掃 raw", () => {
      const shell = pick("<Other>{{World.Time}}</Other>", { tableMode: "interface" });
      expect(shell).toContain("上回合畫面");
      expect(shell).not.toContain("黃昏");
    });

    it("骨架內文含 HTML 片段（甚至 <html 字樣）仍照常填值過卡腳本", () => {
      const shell = pick("<UI><i>{{World.Time}}</i> 註：<html> 標記 {{本回合.正文}}</UI>", {
        tableMode: "interface",
      });
      expect(shell).toContain("<i>黃昏</i>");
      expect(shell).toContain("最新正文沒有標籤");
      expect(shell).not.toContain("上回合畫面");
    });
  });

  it("characters 模式與玩法標記未知一律不顯示殼", () => {
    expect(pick(skeleton, { tableMode: "characters" })).toBeNull();
    expect(pick(skeleton, { tableMode: undefined })).toBeNull();
    expect(pick(null, { tableMode: "characters" })).toBeNull();
    expect(pick(null, { tableMode: undefined })).toBeNull();
  });
});
