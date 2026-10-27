// 非 zod 桌開局（MVU initCheck，與桌面 initvar-nonzod-overwrite 同語意）：開場白的 `<initvar>` 有套上就整份取代
// 世界書 [initvar] 的初始值，再套開場白本身的指令；區塊全部讀不懂時照世界書的值。
import { describe, expect, it } from "vitest";
import { runEval } from "@desktop/features/mvu-engine/value-parser";
import { playCardFromValue } from "../cards/play-card";
import { createRuntime, initializeVariables } from "./runtime";

const card = playCardFromValue("json", {
  spec: "chara_card_v2",
  spec_version: "2.0",
  data: {
    name: "艾琳",
    first_mes: "開場",
    extensions: {
      tavern_helper: {
        scripts: [{ type: "script", enabled: true, name: "MVU", id: "mvu-1", content: "import 'https://example.invalid/MagVarUpdate/bundle.js';" }],
      },
    },
    character_book: { name: "書", entries: [{ keys: [], content: "角色:\n  金幣: 10\n  地點: 村莊\n", comment: "[initvar]", enabled: false, insertion_order: 1 }] },
  },
});

async function openWith(opening: string) {
  const runtime = createRuntime({ run: async (op, text) => runEval(op, text), dispose: () => {} }, { user: "阿濤", char: "艾琳" });
  const result = await initializeVariables(runtime, card, opening, (text) => text.replace(/\{\{user\}\}/gi, "阿濤").replace(/\{\{char\}\}/gi, "艾琳"));
  if (!result.ok) throw new Error(result.error);
  return result.opening?.stat_data as Record<string, unknown>;
}

describe("非 zod 桌開場白的 <initvar>", () => {
  it("整份取代世界書初始值（保序、巨集代換），開場白的指令以覆寫值為底", async () => {
    const stat = await openWith("開場<initvar>\n```yaml\n角色:\n  金幣: 999 # 註解\n  名字: '{{user}}'\n```\n</initvar><UpdateVariable>_.set('角色.金幣', 999, 1000);</UpdateVariable>");
    expect(stat).toEqual({ 角色: { 金幣: 1000, 名字: "阿濤" } });
    expect(Object.keys(stat.角色 as object)).toEqual(["金幣", "名字"]);
  });

  it("區塊全部讀不懂：照世界書的值", async () => {
    expect(await openWith("開場<initvar>*&:|*[</initvar>")).toEqual({ 角色: { 金幣: 10, 地點: "村莊" } });
  });
});
