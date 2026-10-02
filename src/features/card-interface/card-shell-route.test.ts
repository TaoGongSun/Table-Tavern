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
  it.each([
    ["<!DOCTYPE html>", "<!DOCTYPE html><html><body>舊殼 {{World.Time}}</body></html>"],
    ["<html", "<html><body>舊殼 {{World.Time}}</body></html>"],
    ["大小寫不分", "<!doctype HTML><HTML><body>舊殼 {{World.Time}}</body></HTML>"],
    ["前置空白", "  \n\t<html><body>舊殼 {{World.Time}}</body></html>"],
  ])("舊整頁 HTML 產物（%s）當作沒有重構殼，退回近 10 則掃 raw", (_label, legacy) => {
    const shell = pick(legacy);
    expect(shell).toBe(pick(null));
    expect(shell).toContain("上回合畫面");
    expect(shell).not.toContain("舊殼");
  });

  it("合法骨架內文含 HTML 片段（甚至 <html 字樣）仍照常填值過卡腳本", () => {
    const shell = pick("<UI><i>{{World.Time}}</i> 註：<html> 標記 {{本回合.正文}}</UI>");
    expect(shell).toContain("<i>黃昏</i>");
    expect(shell).toContain("最新正文沒有標籤");
    expect(shell).not.toContain("上回合畫面");
  });

  it("骨架沒過卡的顯示腳本：退回近 10 則掃 raw", () => {
    const shell = pick("<Other>{{World.Time}}</Other>");
    expect(shell).toContain("上回合畫面");
    expect(shell).not.toContain("黃昏");
  });

  it("開場 direct-first：最新 GM 原文卡腳本自己畫得出來就不塞骨架", () => {
    const shell = pick("<UI>骨架 {{World.Time}}</UI>", { events: [gm("請選擇身份", "<UI>選角開場</UI>")] });
    expect(shell).toContain("選角開場");
    expect(shell).not.toContain("骨架");
  });

  it("空桌退回卡片開場白", () => {
    expect(pick("<UI>骨架</UI>", { events: [] })).toContain("開場白");
  });

  it("characters 模式與玩法標記未知一律不顯示殼", () => {
    expect(pick("<UI>{{World.Time}}</UI>", { tableMode: "characters" })).toBeNull();
    expect(pick("<UI>{{World.Time}}</UI>", { tableMode: undefined })).toBeNull();
    expect(pick(null, { tableMode: "characters" })).toBeNull();
  });
});
