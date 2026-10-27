// 三張真卡（TestCards/，gitignore、本機限定）的取材與對照指令：source-zod.test.ts（對上游預期）與
// quickjs-entry.test.ts（產生 Node 基準給 cargo 比 QuickJS）共用。
import { existsSync, readFileSync } from "node:fs";
import YAML from "yaml";

type Table = Record<string, any>;

export const cardsDir = new URL("../../../../TestCards/", import.meta.url);

/** DongeonMaster 的第一位冒險者（沒有就用測試名，指令會先建） */
export function dongeonAdventurer(start: Table): string {
  return Object.keys(start.冒险者 ?? {})[0] ?? "测试冒险者";
}

/** 每張卡一批對照指令（開局後的 stat_data → 訊息） */
export const REAL_CARDS: { file: string; message: (start: Table) => string }[] = [
  {
    file: "DongeonMaster.png",
    message: (start) => {
      const adventurer = dongeonAdventurer(start);
      const create = start.冒险者?.[adventurer] ? "" : `_.insert('冒险者', '${adventurer}', {});`;
      return `${create}_.add('冒险者.${adventurer}.生理与心理数值.恶堕值', 500);_.set('基础信息.时间', 7);_.set('基础信息.已覆灭小队数', 'x');_.insert('冒险者.${adventurer}.穿着', '披风', 'y');`;
    },
  },
  {
    file: "bcd368cacbc56b2cc29010e0cbad5f09.png",
    message: () =>
      "_.insert('便器数据库', 'T999', {基础档案: {姓名: 'a'}});_.set('便器数据库.T001.基础档案.评级', '普通');_.set('便器数据库.T001.配种信息.开发阶段', 9);_.set('用户数据.积分', 'abc');_.set('用户数据.积分', '250');",
  },
  {
    file: "HeroTrainingUnderSide_stats.png",
    message: () => {
      const items = Object.fromEntries(Array.from({ length: 11 }, (_, i) => [`杂物${i + 1}`, { 描述: "x", 数量: 1 }]));
      const inventory = JSON.stringify({ 金龙: { 描述: "钱", 数量: 5 }, 破布: { 描述: "y", 数量: 0 }, ...items });
      return `_.set('Player.HP', '12000/10000');_.set('Player.MP', 500);_.set('Player.Inventory', ${inventory});_.add('World.Invasion', 500);_.set('World.Roll20', 'abc');_.insert('Heroes', '新勇者', {Name: 'A'});_.set('Player._hidden', 1);`;
    },
  },
];

export const hasRealCards = REAL_CARDS.every(({ file }) => existsSync(new URL(file, cardsDir)));

/** 原卡的「变量结构」腳本與 [initvar] 條目（initvar 已去標籤、{{user}} 換成「玩家」並解析成物件） */
export function realCard(file: string): { schema: string; initvar: Table } {
  const bytes = readFileSync(new URL(file, cardsDir));
  let offset = 8;
  let json = "";
  while (offset < bytes.length) {
    const length = bytes.readUInt32BE(offset);
    const type = bytes.toString("latin1", offset + 4, offset + 8);
    if (type === "tEXt") {
      const chunk = bytes.subarray(offset + 8, offset + 8 + length);
      const split = chunk.indexOf(0);
      const key = chunk.toString("latin1", 0, split);
      if (key === "ccv3" || (key === "chara" && json === "")) json = Buffer.from(chunk.toString("latin1", split + 1), "base64").toString("utf8");
    }
    offset += 12 + length;
  }
  const data = (JSON.parse(json) as { data: Table }).data;
  const scripts = data.extensions.tavern_helper.scripts as Table[];
  const schema = scripts.find((script) => String(script.content).includes("registerMvuSchema"))!.content as string;
  const entries = data.character_book.entries as Table[];
  const raw = entries.find((entry) => /initvar/i.test(String(entry.comment ?? entry.name)))!.content as string;
  const initvar = YAML.parse(raw.replace(/<\/?Initial_Variables>/g, "").replace(/\{\{user\}\}/g, "玩家")) as Table;
  return { schema, initvar };
}
