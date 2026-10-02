import { describe, expect, it } from "vitest";
import { pickCardShell } from "./card-shell-route";
import { type CardInterface } from "./interface-card";
import { type TranscriptEvent } from "../../shared/contracts/backend-contracts";

function makeCard(name: string, opening: string | null): CardInterface {
  return {
    character_id: name,
    character_name: name,
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
    opening,
  };
}
const card = makeCard("卡", "<UI>開場白</UI>");

function gm(text: string, raw?: string): TranscriptEvent {
  return { ts: "t", speaker_id: "gm", speaker_name: "GM", kind: "narration", text, ...(raw ? { raw } : {}) };
}
function player(text: string): TranscriptEvent {
  return { ts: "t", speaker_id: "", speaker_name: "玩家", kind: "player", text };
}
function system(text: string, gmOnly = false): TranscriptEvent {
  return { ts: "t", speaker_id: "", speaker_name: "系統", kind: "system", text, ...(gmOnly ? { gm_only: true } : {}) };
}

// 較舊那則 GM 原文畫得出來、最新那則畫不出來：近 10 則掃 raw 會拿到「上回合畫面」
const events: TranscriptEvent[] = [gm("上回合", "<UI>上回合畫面</UI>"), gm("最新正文沒有標籤")];
const tableTree = { World: { Time: "黃昏" } };

function pick(refactorShell: string | null, over: Partial<Parameters<typeof pickCardShell>[0]> = {}) {
  return pickCardShell({ tableMode: null, refactorShell, events, tableTree, cardInterfaces: [card], ...over });
}
const shellOf = (...args: Parameters<typeof pick>) => pick(...args)?.shell ?? null;

describe("pickCardShell", () => {
  const skeleton = "<UI>骨架 {{World.Time}} {{本回合.正文}}</UI>";
  const emptyTable: TranscriptEvent[] = [];
  const directFirst: TranscriptEvent[] = [gm("請選擇身份", "<UI>選角開場</UI>")];

  // 沒有骨架（沒重構過、或重構判定不接管）照原卡畫面〔作者裁決 2026-10-02〕
  describe.each([
    ["null 桌", null, null],
    ["interface 桌骨架 null", "interface", null],
    ["interface 桌骨架空字串", "interface", ""],
    ["interface 桌骨架純空白", "interface", " \n\t "],
  ])("%s：原卡畫面", (_label, mode, shell) => {
    it("空桌退回卡片開場白，本樓是第 0 樓、名字是卡名", () => {
      const picked = pick(shell, { tableMode: mode, events: emptyTable });
      expect(picked?.shell).toContain("開場白");
      expect(picked?.current).toEqual({ id: 0, name: "卡", text: "<UI>開場白</UI>" });
    });

    it("最新一則畫得出來就用它", () => {
      const picked = pick(shell, { tableMode: mode, events: directFirst });
      expect(picked?.shell).toContain("選角開場");
      expect(picked?.current).toEqual({ id: 0, name: "GM", text: "<UI>選角開場</UI>" });
    });

    it("近十則：命中較舊那則，本樓是它的原始樓號", () => {
      const picked = pick(shell, { tableMode: mode, events });
      expect(picked?.shell).toContain("上回合畫面");
      expect(picked?.current).toEqual({ id: 0, name: "GM", text: "<UI>上回合畫面</UI>" });
    });
  });

  describe("有效骨架", () => {
    it("空桌退回卡片開場白", () => {
      expect(shellOf(skeleton, { tableMode: "interface", events: emptyTable })).toContain("開場白");
    });

    it("最新 GM 原文卡腳本自己畫得出來就不塞骨架", () => {
      const picked = pick(skeleton, { tableMode: "interface", events: directFirst });
      expect(picked?.shell).toContain("選角開場");
      expect(picked?.shell).not.toContain("骨架");
      expect(picked?.current.text).toBe("<UI>選角開場</UI>");
    });

    it("填骨架：本樓是最新那則，交給卡片的是填值後的合成文字", () => {
      const picked = pick(skeleton, { tableMode: "interface", events });
      expect(picked?.shell).toContain("骨架 黃昏 最新正文沒有標籤");
      expect(picked?.shell).not.toContain("上回合畫面");
      expect(picked?.current).toEqual({ id: 1, name: "GM", text: "<UI>骨架 黃昏 最新正文沒有標籤</UI>" });
    });

    it("骨架沒過卡的顯示腳本：退回近 10 則掃 raw", () => {
      const shell = shellOf("<Other>{{World.Time}}</Other>", { tableMode: "interface" });
      expect(shell).toContain("上回合畫面");
      expect(shell).not.toContain("黃昏");
    });

    it("骨架內文含 HTML 片段（甚至 <html 字樣）仍照常填值過卡腳本", () => {
      const shell = shellOf("<UI><i>{{World.Time}}</i> 註：<html> 標記 {{本回合.正文}}</UI>", {
        tableMode: "interface",
      });
      expect(shell).toContain("<i>黃昏</i>");
      expect(shell).toContain("最新正文沒有標籤");
      expect(shell).not.toContain("上回合畫面");
    });
  });

  describe("樓號", () => {
    it("玩家／GM／system 混合：樓號是原始位置，gm_only 不算一樓", () => {
      const mixed = [
        gm("開場", "<UI>第一畫面</UI>"),
        system("登場：某人私設", true),
        player("我走進去"),
        system("換幕提示"),
        gm("沒有標籤"),
        player("再一句"),
      ];
      const picked = pick(null, { events: mixed });
      expect(picked?.current).toEqual({ id: 0, name: "GM", text: "<UI>第一畫面</UI>" });

      const later = [...mixed, gm("新的", "<UI>新畫面</UI>")];
      // gm_only 那則不算：樓號 0 GM、1 玩家、2 system、3 GM、4 玩家、5 GM
      expect(pick(null, { events: later })?.current.id).toBe(5);
    });

    it("gm_only 事件的原文不會被拿來畫殼", () => {
      expect(pick(null, { events: [system("<UI>私設</UI>", true)] })?.current.text).toBe("<UI>開場白</UI>");
    });

    it("重複原文：取最新命中的那則", () => {
      const same = [gm("a", "<UI>同</UI>"), player("x"), gm("b", "<UI>同</UI>")];
      expect(pick(null, { events: same })?.current.id).toBe(2);
    });

    it("近十則邊界：第 10 則命中，第 11 則不納入", () => {
      const tail = Array.from({ length: 9 }, (_, i) => gm(`無標籤${i}`));
      expect(pick(null, { events: [gm("x", "<UI>第十則</UI>"), ...tail] })?.current.id).toBe(0);
      expect(pick(null, { events: [gm("x", "<UI>第十一則</UI>"), gm("無"), ...tail] })).toBeNull();
    });

    it("多卡 opening：名字取命中那張的卡名", () => {
      const silent = makeCard("沒開場", null);
      const other = makeCard("第二張", "<UI>第二張開場</UI>");
      const picked = pick(null, { events: [], cardInterfaces: [silent, other] });
      expect(picked?.current).toEqual({ id: 0, name: "第二張", text: "<UI>第二張開場</UI>" });
    });
  });

  it("characters 模式與玩法標記未知一律不顯示殼", () => {
    expect(pick(skeleton, { tableMode: "characters" })).toBeNull();
    expect(pick(skeleton, { tableMode: undefined })).toBeNull();
    expect(pick(null, { tableMode: "characters" })).toBeNull();
    expect(pick(null, { tableMode: undefined })).toBeNull();
  });
});
