import { describe, expect, it } from "vitest";
import { pickCardShell, skeletonYamlTags } from "./card-shell-route";
import { type CardInterface } from "./interface-card";
import { setLang } from "../../i18n";
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
// 帶狀態快照的 GM 樓：每則 GM 事件都存下那一刻的狀態樹
function gmAt(text: string, tree: Record<string, unknown>, raw?: string): TranscriptEvent {
  return { ...gm(text, raw), state: { table: {}, tree } };
}
function player(text: string): TranscriptEvent {
  return { ts: "t", speaker_id: "", speaker_name: "玩家", kind: "player", text };
}
function system(text: string, gmOnly = false): TranscriptEvent {
  return { ts: "t", speaker_id: "", speaker_name: "系統", kind: "system", text, ...(gmOnly ? { gm_only: true } : {}) };
}

// 較舊那則 GM 原文畫得出來、最新那則畫不出來（但存了狀態快照）：沒骨架時近 10 則掃 raw 會拿到「上回合畫面」
const events: TranscriptEvent[] = [
  gm("上回合", "<UI>上回合畫面</UI>"),
  gmAt("最新正文沒有標籤", { World: { Time: "黃昏" } }),
];

function pick(refactorShell: string | null, over: Partial<Parameters<typeof pickCardShell>[0]> = {}) {
  return pickCardShell({ tableMode: null, refactorShell, events, cardInterfaces: [card], ...over });
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

    it("本樓用自己那一刻的狀態快照填骨架，與交給卡片的那一樓是同一份文字", () => {
      const picked = pick(skeleton, { tableMode: "interface", events });
      expect(picked?.shell).toContain("骨架 黃昏 最新正文沒有標籤");
      expect(picked?.shell).not.toContain("上回合畫面");
      expect(picked?.current).toEqual({ id: 1, name: "GM", text: "<UI>骨架 黃昏 最新正文沒有標籤</UI>" });
      expect(picked?.floors[1].message).toBe(picked?.current.text);
      // 重構前就畫得出殼的舊樓維持原文
      expect(picked?.floors[0].message).toBe("<UI>上回合畫面</UI>");
    });

    it("歷史樓各自用自己的快照：兩個 GM 樓的值不同", () => {
      const history = [
        gmAt("第一則", { World: { Time: "清晨" } }),
        player("嗯"),
        gmAt("第二則", { World: { Time: "黃昏" } }),
      ];
      const picked = pick(skeleton, { tableMode: "interface", events: history });
      expect(picked?.floors.map((floor) => floor.message)).toEqual([
        "<UI>骨架 清晨 第一則</UI>",
        "嗯",
        "<UI>骨架 黃昏 第二則</UI>",
      ]);
      expect(picked?.current.id).toBe(2);
    });

    it("system 與玩家樓不合成；最新一則是 system 時本樓仍是前一個 GM 樓", () => {
      const withSystem = [
        gmAt("正文", { World: { Time: "清晨" } }),
        player("我說"),
        { ...system("換幕提示"), state: { table: {}, tree: { World: { Time: "午夜" } } } },
      ];
      const picked = pick(skeleton, { tableMode: "interface", events: withSystem });
      expect(picked?.floors[1].message).toBe("我說");
      expect(picked?.floors[2].message).toBe("換幕提示");
      expect(picked?.current).toEqual({ id: 0, name: "GM", text: "<UI>骨架 清晨 正文</UI>" });
    });

    it("帶標頭代碼的系統樓給照語系組好的全文；沒名字的玩家樓退回玩家稱呼", () => {
      const history: TranscriptEvent[] = [
        { ...player("我說"), speaker_name: "" },
        { ...system(""), marker: { type: "gm_call", name: "" } },
        { ...system("夜裡才醒。"), marker: { type: "card_arrival", name: "貓頭鷹" } },
        gm("畫面", "<UI>上回合畫面</UI>"),
      ];
      const picked = pick(null, { events: history });
      expect(picked?.floors.map((floor) => [floor.name, floor.message])).toEqual([
        ["玩家", "我說"],
        ["系統", "GM 請「玩家」發言"],
        ["系統", "（角色回歸）〈貓頭鷹〉\n公開設定：\n夜裡才醒。"],
        ["GM", "<UI>上回合畫面</UI>"],
      ]);
    });

    it("帶標頭代碼的 GM 樓填骨架時正文是照語系組好的全文；切語系標頭跟著變", () => {
      const summary: TranscriptEvent = {
        ...gmAt("摘要本文", { World: { Time: "清晨" } }),
        marker: { type: "scene_summary" },
      };
      const picked = pick(skeleton, { tableMode: "interface", events: [summary] });
      expect(picked?.floors[0].message).toBe("<UI>骨架 清晨 【前情提要】\n摘要本文</UI>");
      expect(picked?.current).toEqual({ id: 0, name: "GM", text: "<UI>骨架 清晨 【前情提要】\n摘要本文</UI>" });
      setLang("en");
      try {
        const english = pick(skeleton, { tableMode: "interface", events: [summary] });
        expect(english?.current.text).toBe("<UI>骨架 清晨 Previously:\n摘要本文</UI>");
        expect(english?.floors[0].message).toBe(english?.current.text);
      } finally {
        setLang("zh-TW");
      }
    });

    it("沒有快照的 GM 樓退回原文", () => {
      const noSnapshot = [gm("上回合", "<UI>上回合畫面</UI>"), gm("沒有快照")];
      const picked = pick(skeleton, { tableMode: "interface", events: noSnapshot });
      expect(picked?.floors[1].message).toBe("沒有快照");
      expect(picked?.current.id).toBe(0);
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

    it("沒重構過的桌：帶快照的樓也照原文，行為不變", () => {
      const picked = pick(null, { events });
      expect(picked?.floors.map((floor) => floor.message)).toEqual([
        "<UI>上回合畫面</UI>",
        "最新正文沒有標籤",
      ]);
    });

    it("YAML 容器只取自實際解析 YAML 的那支腳本：容器內照 YAML 寫、正文與別張卡的容器照原樣", () => {
      const yamlCard: CardInterface = {
        ...card,
        scripts: [
          {
            ...card.scripts[0],
            find_regex: "/<UI>([\\s\\S]*?)<\\/UI>/s",
            replace_string: card.scripts[0].replace_string + '<script src="js-yaml.min.js"></script>',
          },
        ],
      };
      const otherCard: CardInterface = {
        ...card,
        character_id: "c2",
        scripts: [{ ...card.scripts[0], find_regex: "/<Note>([\\s\\S]*?)<\\/Note>/s" }],
      };
      expect(skeletonYamlTags([yamlCard, otherCard])).toEqual(["UI"]);
      expect(skeletonYamlTags([card])).toEqual([]);
      const picked = pick('<UI>地点: "{{World.Time}}"</UI><Note>{{World.Time}}</Note>\n{{本回合.正文}}', {
        tableMode: "interface",
        cardInterfaces: [yamlCard, otherCard],
        events: [gmAt("甲\n乙", { World: { Time: '他說"走"' } })],
      });
      expect(picked?.current.text).toBe('<UI>地点: "他說\\"走\\""</UI><Note>他說"走"</Note>\n甲\n乙');
    });

    it("一支 regex 同時抓正文與狀態區塊時，正文照原樣、只有 YAML 結構的容器照 YAML 寫", () => {
      const composite: CardInterface = {
        ...card,
        scripts: [
          {
            ...card.scripts[0],
            find_regex: "/<maintext>([\\s\\S]*?)<\\/maintext>[\\s\\S]*?<Status_block>([\\s\\S]*?)<\\/Status_block>/s",
            replace_string: "```html\n<!DOCTYPE html><body>$1|$2</body>\n```<script src=\"js-yaml.min.js\"></script>",
          },
        ],
      };
      const shell = "<maintext>\n正文：{{本回合.正文}}\n</maintext>\n<Status_block>\n状态栏:\n  地点: {{World.Place}}\n</Status_block>";
      const picked = pick(shell, {
        tableMode: "interface",
        cardInterfaces: [composite],
        events: [gmAt("他說：「走」", { World: { Place: "a # b" } })],
      });
      expect(picked?.current.text).toBe(
        '<maintext>\n正文：他說：「走」\n</maintext>\n<Status_block>\n状态栏:\n  地点: "a # b"\n</Status_block>',
      );
    });

    it("最新 GM 樓後面接了十個以上玩家樓，仍以它為本樓（玩家樓不擠掉殼）", () => {
      const many = [gmAt("正文", { World: { Time: "清晨" } }), ...Array.from({ length: 12 }, (_, i) => player(`第${i}句`))];
      const picked = pick(skeleton, { tableMode: "interface", events: many });
      expect(picked?.current).toEqual({ id: 0, name: "GM", text: "<UI>骨架 清晨 正文</UI>" });
      expect(picked?.floors).toHaveLength(13);
      // 沒重構過的桌同理：GM 原文畫得出殼就不會因為後面一串玩家樓而消失
      const plain = [gm("舊", "<UI>畫面</UI>"), ...Array.from({ length: 12 }, (_, i) => player(`第${i}句`))];
      expect(pick(null, { events: plain })?.current.id).toBe(0);
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

describe("pickCardShell：MVU 卡", () => {
  // DongeonMaster 型：開場畫面走 <开局> 那支、狀態欄走占位那支
  const mvuCard: CardInterface = {
    character_id: "c",
    character_name: "迷宮",
    scripts: [
      {
        name: "开局",
        find_regex: "<开局>",
        replace_string: "```html\n<!DOCTYPE html><body>开局畫面</body>\n```",
        trim_strings: [],
        min_depth: null,
        max_depth: null,
      },
      {
        name: "迷宫之书",
        find_regex: "<StatusPlaceHolderImpl/>",
        replace_string: "```html\n<!DOCTYPE html><body>迷宫之书</body>\n```",
        trim_strings: [],
        min_depth: null,
        max_depth: null,
      },
    ],
    unsupported: null,
    opening: "那么，你付出了什么代价？\n<开局>",
    mvu: true,
  };
  const opening: TranscriptEvent = {
    ...gmAt("那么，你付出了什么代价？", { 基础信息: { 时间: "未知" } }),
    speaker_id: "",
    opening: true,
  };
  opening.raw = "那么，你付出了什么代价？\n<开局>";
  const reply = (text: string, time: string): TranscriptEvent => ({
    ...gmAt(text, { 基础信息: { 时间: time } }),
    speaker_id: "",
  });
  const mvuPick = (list: TranscriptEvent[], liveTree: Record<string, string | Record<string, string>> = {}) =>
    pickCardShell({
      tableMode: null,
      refactorShell: null,
      events: list,
      cardInterfaces: [mvuCard],
      mvu: { liveTree: liveTree as never, userName: "阿濤" },
    });

  it("character 層身分是實際產生殼的卡：A 卡先列、B 卡產生殼時取 B；產不出同一份殼（腳本接力）就空字串；世界書卡固定 world", () => {
    const other = (id: string, find: string): CardInterface => ({
      ...mvuCard,
      character_id: id,
      mvu: id === "B",
      scripts: [{ ...mvuCard.scripts[1], find_regex: find, replace_string: `\`\`\`html\n<!DOCTYPE html><body>${id}殼</body>\n\`\`\`` }],
      opening: null,
    });
    const a = other("A", "<NoMatch/>");
    const b = other("B", "<StatusPlaceHolderImpl/>");
    const list = [gmAt("正文夠長了", { 基础信息: { 时间: "x" } })];
    const owner = (cards: CardInterface[]) =>
      pickCardShell({
        tableMode: null,
        refactorShell: null,
        events: list.map((event) => ({ ...event, speaker_id: "" })),
        cardInterfaces: cards,
        mvu: { liveTree: { 基础信息: { 时间: "x" } } as never, userName: "阿濤" },
      })?.mvu?.characterId;
    expect(owner([a, b])).toBe("B");
    expect(owner([b, a])).toBe("B");
    expect(owner([a, { ...b, character_id: "" }])).toBe("world");
    // 兩張卡各自都產不出同一份殼（A 的輸出給 B 接著轉）：不猜
    const relay = (id: string, find: string, replace: string): CardInterface => ({
      ...b,
      character_id: id,
      scripts: [{ ...b.scripts[0], find_regex: find, replace_string: replace }],
    });
    const first = relay("R1", "<StatusPlaceHolderImpl/>", "<Mid/>");
    const second = relay("R2", "<Mid/>", "```html\n<!DOCTYPE html><body>接力殼</body>\n```");
    expect(owner([first, second])).toBe("");
  });

  it("身分跟著選殼命中的那段文字走：A 的舊樓與 B 的最新樓產出同一份殼時取 B，不因重掃舊樓取 A", () => {
    const sameShell = "```html\n<!DOCTYPE html><body>同一份殼</body>\n```";
    const make = (id: string, find: string, mvu: boolean): CardInterface => ({
      ...mvuCard,
      character_id: id,
      mvu,
      scripts: [{ ...mvuCard.scripts[1], find_regex: find, replace_string: sameShell }],
      opening: null,
    });
    const a = make("A", "<OldTag/>", false);
    const b = make("B", "<StatusPlaceHolderImpl/>", true);
    const oldFloor: TranscriptEvent = { ...gmAt("舊樓", { 基础信息: { 时间: "a" } }), speaker_id: "", raw: "舊樓正文 <OldTag/>" };
    const latest: TranscriptEvent = { ...gmAt("最新樓正文", { 基础信息: { 时间: "b" } }), speaker_id: "" };
    for (const cards of [[a, b], [b, a]]) {
      const picked = pickCardShell({
        tableMode: null,
        refactorShell: null,
        events: [oldFloor, latest],
        cardInterfaces: cards,
        mvu: { liveTree: { 基础信息: { 时间: "b" } } as never, userName: "阿濤" },
      });
      expect(picked?.current.id).toBe(1);
      expect(picked?.mvu?.characterId).toBe("B");
    }
  });

  it("空桌與開場樓：开局畫面不被占位搶走（開場樓不補占位）", () => {
    expect(mvuPick([], { 基础信息: { 时间: "未知" } })?.shell).toContain("开局畫面");
    const picked = mvuPick([opening], { 基础信息: { 时间: "未知" } });
    expect(picked?.shell).toContain("开局畫面");
    expect(picked?.shell).not.toContain("迷宫之书");
    expect(picked?.floors[0].message).not.toContain("StatusPlaceHolderImpl");
  });

  it("GM 回覆補占位，選殼與讀訊息用同一份文字；殼後面的玩家句不影響活狀態", () => {
    const picked = mvuPick([opening, player("我選恐惧"), reply("迷宫开始运转了", "第1日"), player("下一步")], {
      基础信息: { 时间: "第1日（手改）" },
    });
    expect(picked?.shell).toContain("迷宫之书");
    expect(picked?.current.id).toBe(2);
    expect(picked?.current.text).toBe("迷宫开始运转了\n\n<StatusPlaceHolderImpl/>");
    expect(picked?.floors[2].message).toBe(picked?.current.text);
    expect(picked?.floors[3].message).toBe("下一步");
    const mvu = picked!.mvu!;
    expect(mvu.currentId).toBe(2);
    expect(mvu.states[mvu.floorState[2]].stat_data).toEqual({ 基础信息: { 时间: "第1日（手改）" } });
    expect(mvu.states[mvu.floorState[0]].stat_data).toEqual({ 基础信息: { 时间: "未知" } });
  });

  it("沒有開場、第一樓就是 GM 回覆：照補占位（上游不排除第 0 樓）", () => {
    const picked = mvuPick([reply("迷宫开始运转了", "第1日")], { 基础信息: { 时间: "第1日" } });
    expect(picked?.shell).toContain("迷宫之书");
    expect(picked?.current.id).toBe(0);
    expect(picked?.floors[0].message).toBe("迷宫开始运转了\n\n<StatusPlaceHolderImpl/>");
  });

  it("中止的 GM 回覆（截斷）也補占位、成為活的本樓", () => {
    const aborted = { ...reply("迷宫开始运转到一半", "第1日"), truncated: true };
    const picked = mvuPick([opening, player("繼續"), aborted], { 基础信息: { 时间: "第1日（手改）" } });
    expect(picked?.current.id).toBe(2);
    expect(picked!.mvu!.states[picked!.mvu!.floorState[2]].stat_data).toEqual({ 基础信息: { 时间: "第1日（手改）" } });
  });

  it("gm_only 事件不佔樓號：狀態來源與公開樓號同一套索引", () => {
    const picked = mvuPick([opening, system("私設", true), reply("迷宫开始运转了", "第1日"), reply("第二天到了", "第2日")], {
      基础信息: { 时间: "第2日" },
    });
    expect(picked?.floors).toHaveLength(3);
    const mvu = picked!.mvu!;
    expect(mvu.floorState).toHaveLength(3);
    expect(mvu.states[mvu.floorState[1]].stat_data).toEqual({ 基础信息: { 时间: "第1日" } });
    expect(picked?.current.id).toBe(2);
  });

  it("沒有 stat_data 的樓不補占位；非 MVU 卡、重構骨架桌不給 MVU 快照", () => {
    const empty = mvuPick([opening, { ...reply("迷宫开始运转了", "x"), state: undefined }], {});
    expect(empty?.floors[1].message).toBe("迷宫开始运转了");
    expect(empty?.shell).toContain("开局畫面");
    expect(
      pickCardShell({ tableMode: null, refactorShell: null, events, cardInterfaces: [card], mvu: { liveTree: {}, userName: "阿濤" } })
        ?.mvu,
    ).toBeNull();
    const skeletonPick = pickCardShell({
      tableMode: "interface",
      refactorShell: "<StatusPlaceHolderImpl/>",
      events: [reply("迷宫开始运转了", "第1日")],
      cardInterfaces: [mvuCard],
      mvu: { liveTree: {}, userName: "阿濤" },
    });
    expect(skeletonPick?.mvu ?? null).toBeNull();
  });
});
