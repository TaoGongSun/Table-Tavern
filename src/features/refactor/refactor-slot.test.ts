import { describe, expect, it } from "vitest";
import { assembleRefactorOutcome, mergeRefactorInterfaces, type RefactorInterface } from "./refactor-review";
import { sourceContainerRoles } from "./refactor-slot";

const rule = (kind: string) => ({ kind, update: "replace", inject: "turn" });

// RPGImmortal 兩條來源原文（節錄，容器結構照原卡）
const FORMAT_SOURCE = `<insturction name="格式维持铁律">
rule:
  - LLM创作的对话内容应该放在<maintext></maintext>标签中.
  - 正文之后必须从 <BGM_list> 选择一首BGM，放在<BGM></BGM>标签中
  - LLM创作的状态栏应该放在 <Status_block></Status_block> 标签
formate: |-
<maintext>
\${LLM输出的角色对话内容}
</maintext>
<BGM>xxx.mp3</BGM>
 <Status_block>
\${Status_block中必须输出行动选项！后面不许再输出 <user>和<char>的对话信息 }
</Status_block>
</insturction>`;

const STATUS_SOURCE = `<instruction name="状态栏生成规则">
- **行动选项生成4个可选项**
\`\`\`yaml
<Status_block>
状态栏:
  地点: "\${📍 地点}"
  行动选项:
    选项:
      - "1. \${🎲 判定}"
</Status_block>
</instruction>`;

const DEFINED = `<Status_block>
状态栏:
  地点: "📍 {{地點}}"
  行动选项:
    选项:
      - "1. 🎲{{行動選項.判定}}"
</Status_block>`;

function statusbar(uid: string): RefactorInterface {
  return {
    state_fields: { 地點: "揚州", 行動選項: { 判定: "" } },
    source_uids: [uid],
    raw: "美化状态栏",
    shell: DEFINED,
    rules: { 地點: rule("text"), "行動選項.判定": rule("text") },
    guide: "- 地點：格式如「揚州上空」。\n- 行動選項.判定：寫「【技能檢定】＋行動」。",
  };
}

function format(uid: string, option: string): RefactorInterface {
  return {
    state_fields: { BGM: "", [option]: "" },
    source_uids: [uid],
    raw: "格式增强",
    shell: `<maintext>\n{{本回合.正文}}\n</maintext>\n<BGM>{{BGM}}</BGM>\n <Status_block>\n{{${option}}}\n</Status_block>`,
    rules: { BGM: rule("text"), [option]: rule("text") },
    guide: `每回合必報：\n- BGM：只填一個音檔名稱。\n- ${option}：本回合給玩家的行動選項，一個字串。`,
  };
}

const ROLES = { "0": sourceContainerRoles(STATUS_SOURCE), "4": sourceContainerRoles(FORMAT_SOURCE) };

describe("sourceContainerRoles", () => {
  it("只認頂層、不帶屬性的容器；同一標籤有一處固定文字就算定義", () => {
    expect(sourceContainerRoles(FORMAT_SOURCE)).toEqual({
      placeholder: ["maintext", "Status_block"],
      defining: ["BGM"],
    });
    expect(sourceContainerRoles(STATUS_SOURCE)).toEqual({ placeholder: [], defining: ["Status_block"] });
    // 容器內的標籤不往下找：外層 <A> 有固定文字，內層 <B> 不列
    expect(sourceContainerRoles("<A>標題\n<B>${x}</B>\n</A>")).toEqual({ placeholder: [], defining: ["A"] });
  });
});

describe("mergeRefactorInterfaces：佔位容器讓位給定義容器", () => {
  const expectedShell = `<maintext>\n{{本回合.正文}}\n</maintext>\n<BGM>{{BGM}}</BGM>\n ${DEFINED}`;

  it("RPGImmortal：外框條自長的葉子 `行動選項` 撞定義條的分支——不再整組丟，只留一個 Status_block", () => {
    const merged = mergeRefactorInterfaces([statusbar("0"), format("4", "行動選項")], [], ROLES);
    expect(merged.conflict).toBeNull();
    expect(merged.interface?.shell).toBe(expectedShell);
    expect(merged.interface?.state_fields).toEqual({ 地點: "揚州", 行動選項: { 判定: "" }, BGM: "" });
    expect(Object.keys(merged.interface?.rules ?? {}).sort()).toEqual(["BGM", "地點", "行動選項.判定"].sort());
    expect(merged.interface?.guide).not.toContain("- 行動選項：");
    expect(merged.interface?.guide).toContain("- BGM：只填一個音檔名稱。");
    expect(merged.interface?.guide).toContain("- 行動選項.判定：");
    expect(merged.interface?.source_uids).toEqual(["0", "4"]);
  });

  it("名字沒撞（簡體 `行动选项`）：不再多印第二個 Status_block，多出來的欄位與規則一起剪掉", () => {
    const merged = mergeRefactorInterfaces([format("4", "行动选项"), statusbar("0")], [], ROLES);
    expect(merged.conflict).toBeNull();
    expect(merged.interface?.shell).toBe(expectedShell);
    expect(merged.interface?.shell?.match(/<Status_block>/g)).toHaveLength(1);
    expect(merged.interface?.state_fields).not.toHaveProperty("行动选项");
    expect(merged.interface?.rules).not.toHaveProperty("行动选项");
  });

  it("外框條 uid 比定義條小：順序與 uid 無關，兩條都套上", () => {
    const roles = { "1": ROLES["4"], "5": ROLES["0"] };
    const merged = mergeRefactorInterfaces([statusbar("5"), format("1", "行動選項")], [], roles);
    expect(merged.conflict).toBeNull();
    expect(merged.interface?.shell).toBe(expectedShell);
    expect(merged.interface?.source_uids).toEqual(["1", "5"]);
  });

  it("來源原文的容器有固定標題（`<A>背包：列出物品</A>`）就是定義、不是佔位：骨架同名容器不讓位，欄位照留", () => {
    const list: RefactorInterface = {
      state_fields: { 背包: "" },
      source_uids: ["1"],
      raw: "背包",
      shell: "<A>{{背包}}</A>",
      rules: { 背包: rule("list") },
    };
    const coins: RefactorInterface = {
      state_fields: { 金幣: 0 },
      source_uids: ["2"],
      raw: "金幣",
      shell: "<A>金幣：{{金幣}}</A>",
      rules: { 金幣: rule("number") },
    };
    const roles = { "1": sourceContainerRoles("<A>背包：列出物品</A>"), "2": sourceContainerRoles("<A>金幣：${數量}</A>") };
    const merged = mergeRefactorInterfaces([list, coins], [], roles);
    expect(merged.interface?.state_fields).toEqual({ 背包: "", 金幣: 0 });
    expect(merged.interface?.shell).toBe("<A>{{背包}}</A>\n<A>金幣：{{金幣}}</A>");
  });

  it("巢狀路徑剪掉後不留空父節點", () => {
    const frame: RefactorInterface = {
      state_fields: { 選項: { 甲: "" }, 共用: { 時間: "" } },
      source_uids: ["4"],
      raw: "外框",
      shell: "<maintext>{{本回合.正文}} {{共用.時間}}</maintext>\n<Status_block>{{選項.甲}}</Status_block>",
      rules: { "選項.甲": rule("text"), "共用.時間": rule("text") },
    };
    const merged = mergeRefactorInterfaces([statusbar("0"), frame], [], ROLES);
    expect(merged.conflict).toBeNull();
    expect(merged.interface?.state_fields).toEqual({ 地點: "揚州", 行動選項: { 判定: "" }, 共用: { 時間: "" } });
    expect(merged.interface?.rules).not.toHaveProperty("選項.甲");
  });

  it("佔位容器外也用到的路徑保留", () => {
    const frame: RefactorInterface = {
      state_fields: { 共用: { 時間: "" } },
      source_uids: ["4"],
      raw: "外框",
      shell: "<maintext>{{本回合.正文}} {{共用.時間}}</maintext>\n<Status_block>{{共用.時間}}</Status_block>",
      rules: { "共用.時間": rule("text") },
    };
    const merged = mergeRefactorInterfaces([statusbar("0"), frame], [], ROLES);
    expect(merged.conflict).toBeNull();
    expect(merged.interface?.state_fields).toHaveProperty("共用.時間");
    expect(merged.interface?.shell?.match(/<Status_block>/g)).toHaveLength(1);
  });

  it("輕防：來源被外層包住、說明文字有空容器而誤判佔位方，骨架多行多欄位的真狀態欄不讓位、欄位不剪", () => {
    const wrapped = "rule: 状态栏放在 <Status_block></Status_block> 中间\n<format>\n<Status_block>\n粮草: ${粮草}\n马匹: ${马匹}\n</Status_block>\n</format>";
    const weather = "<Status_block>\n天气: ${天气}\n</Status_block>";
    const roles = { "1": sourceContainerRoles(weather), "2": sourceContainerRoles(wrapped) };
    expect(roles["2"].placeholder).toEqual(["Status_block"]);
    const y: RefactorInterface = {
      state_fields: { 天氣: "晴" }, source_uids: ["1"], raw: "y",
      shell: "<Status_block>\n天气: {{天氣}}\n</Status_block>", rules: { 天氣: rule("text") }, guide: "- 天氣：晴雨",
    };
    const x: RefactorInterface = {
      state_fields: { 糧草: "300", 馬匹: "40" }, source_uids: ["2"], raw: "x",
      shell: "<Status_block>\n粮草: {{糧草}}\n马匹: {{馬匹}}\n</Status_block>",
      rules: { 糧草: rule("number"), 馬匹: rule("number") }, guide: "- 糧草：數字\n- 馬匹：數字",
    };
    const merged = mergeRefactorInterfaces([y, x], [], roles);
    expect(merged.interface?.state_fields).toEqual({ 天氣: "晴", 糧草: "300", 馬匹: "40" });
    expect(merged.interface?.shell?.match(/<Status_block>/g)).toHaveLength(2);
    expect(merged.interface?.guide).toContain("- 糧草：數字");
  });

  it.each([
    ["多行固定文字＋單一路徑", "<Status_block>\n標題一\n標題二：{{x}}\n</Status_block>", { x: "" }],
    ["一行固定文字＋兩個不同路徑", "<Status_block>標題：{{x}} {{y}}</Status_block>", { x: "", y: "" }],
  ])("輕防邊界：%s不讓位", (_, shell, state) => {
    const frame: RefactorInterface = {
      state_fields: state, source_uids: ["4"], raw: "外框", shell,
      rules: Object.fromEntries(Object.keys(state).map((key) => [key, rule("text")])),
    };
    const merged = mergeRefactorInterfaces([statusbar("0"), frame], [], ROLES);
    expect(merged.interface?.state_fields).toMatchObject(state);
    expect(merged.interface?.shell?.match(/<Status_block>/g)).toHaveLength(2);
  });

  it("輕防邊界：同一路徑出現兩次算一個，照樣讓位", () => {
    const frame: RefactorInterface = {
      state_fields: { x: "" }, source_uids: ["4"], raw: "外框",
      shell: "<Status_block>標題：{{x}}／{{x}}</Status_block>", rules: { x: rule("text") },
    };
    const merged = mergeRefactorInterfaces([statusbar("0"), frame], [], ROLES);
    expect(merged.interface?.state_fields).not.toHaveProperty("x");
    expect(merged.interface?.shell?.match(/<Status_block>/g)).toHaveLength(1);
  });

  it("輕防：佔位容器裡巢狀 `<B>{{bgm}}</B>{{y}}`（兩個路徑）不讓位", () => {
    const frame: RefactorInterface = {
      state_fields: { bgm: "", y: "" }, source_uids: ["4"], raw: "外框",
      shell: "<Status_block><B>{{bgm}}</B>{{y}}</Status_block>",
      rules: { bgm: rule("text"), y: rule("text") },
    };
    const merged = mergeRefactorInterfaces([statusbar("0"), frame], [], ROLES);
    expect(merged.interface?.state_fields).toMatchObject({ bgm: "", y: "" });
    expect(merged.interface?.shell?.match(/<Status_block>/g)).toHaveLength(2);
  });

  it("佔位容器含正文槽、定義容器沒有：不換（換了正文就沒地方放）", () => {
    const frame: RefactorInterface = {
      state_fields: { BGM: "" },
      source_uids: ["4"],
      raw: "外框",
      shell: "<Status_block>{{本回合.正文}}</Status_block>\n<BGM>{{BGM}}</BGM>",
      rules: { BGM: rule("text") },
    };
    const merged = mergeRefactorInterfaces([statusbar("0"), frame], [], ROLES);
    expect(merged.interface?.shell).toBe(`${DEFINED}\n${frame.shell}`);
  });

  it("guide 刪行：去掉開頭 -、*、反引號後完整路徑緊接冒號才刪", () => {
    const frame = format("4", "行動選項");
    frame.guide = [
      "- 行動選項：一",
      "* `行動選項`：二",
      "- **行動選項**: 三",
      "行動選項:四",
      "- 行動選項值：保留",
      "- 說明裡提到行動選項：保留",
      "- BGM：保留",
    ].join("\n");
    const merged = mergeRefactorInterfaces([statusbar("0"), frame], [], ROLES);
    const guide = merged.interface?.guide ?? "";
    expect(guide).not.toMatch(/[：:]\s*[一二三四]$/m);
    expect(guide).toContain("- 行動選項值：保留");
    expect(guide).toContain("- 說明裡提到行動選項：保留");
    expect(guide).toContain("- BGM：保留");
  });

  it("帶空白的正文槽 `{{ 本回合.正文 }}` 也算正文槽：定義容器沒有正文槽就不換", () => {
    const frame: RefactorInterface = {
      state_fields: { BGM: "" },
      source_uids: ["4"],
      raw: "外框",
      shell: "<Status_block>{{ 本回合.正文 }}</Status_block>\n<BGM>{{BGM}}</BGM>",
      rules: { BGM: rule("text") },
    };
    const merged = mergeRefactorInterfaces([statusbar("0"), frame], [], ROLES);
    expect(merged.interface?.shell).toBe(`${DEFINED}\n${frame.shell}`);
  });

  it.each(["行動選項", "行动选项"])("模型在佔位容器加自編標籤（`%s：{{…}}`）仍讓位", (option) => {
    const frame = format("4", option);
    frame.shell = frame.shell!.replace(`{{${option}}}`, `行动选项：{{${option}}}`);
    const merged = mergeRefactorInterfaces([statusbar("0"), frame], [], ROLES);
    expect(merged.conflict).toBeNull();
    expect(merged.interface?.shell).toBe(`<maintext>\n{{本回合.正文}}\n</maintext>\n<BGM>{{BGM}}</BGM>\n ${DEFINED}`);
    expect(merged.interface?.state_fields).toEqual({ 地點: "揚州", 行動選項: { 判定: "" }, BGM: "" });
  });

  it("兩邊同名同路徑、規則不同（佔位那條 text、定義那條 list）：留定義那條的值與規則", () => {
    const definer: RefactorInterface = {
      state_fields: { 行動選項: "- 1. 判定" },
      source_uids: ["0"],
      raw: "美化",
      shell: "<Status_block>\n行动选项:\n{{行動選項}}\n</Status_block>",
      rules: { 行動選項: rule("list") },
      guide: "- 行動選項：四行清單，每行一個選項。",
    };
    const merged = mergeRefactorInterfaces([definer, format("4", "行動選項")], [], ROLES);
    expect(merged.conflict).toBeNull();
    expect(merged.interface?.state_fields).toEqual({ 行動選項: "- 1. 判定", BGM: "" });
    expect(merged.interface?.rules).toEqual({ 行動選項: rule("list"), BGM: rule("text") });
    expect(merged.interface?.shell?.match(/<Status_block>/g)).toHaveLength(1);
    // 佔位條自己剪掉的路徑即使定義條也保留，也不算混合行：佔位條那行刪、定義條那行留
    expect(merged.interface?.guide).toContain("- 行動選項：四行清單，每行一個選項。");
    expect(merged.interface?.guide).not.toContain("- 行動選項：本回合給玩家的行動選項，一個字串。");
  });

  it("guide 混合行與多行說明：同一行還提到保留路徑就整行留著，只刪全是被剪路徑的行", () => {
    const frame = format("4", "行動選項");
    frame.guide = [
      "每回合必報：",
      "- 行動選項：給四個選項。BGM：必須從曲庫選。",
      "- 行動選項：給四個選項。",
      "  每個選項 20 字左右。",
      "- BGM：只填檔名。",
    ].join("\n");
    const merged = mergeRefactorInterfaces([statusbar("0"), frame], [], ROLES);
    const guide = merged.interface?.guide ?? "";
    expect(guide).toContain("- 行動選項：給四個選項。BGM：必須從曲庫選。");
    expect(guide).not.toMatch(/^- 行動選項：給四個選項。$/m);
    expect(guide).toContain("每回合必報：");
    expect(guide).toContain("  每個選項 20 字左右。");
    expect(guide).toContain("- BGM：只填檔名。");
  });

  it("guide 混合行保護看全部候選保留的路徑：佔位方順帶提到定義方的欄位就整行留著", () => {
    const frame = format("4", "行動選項");
    frame.guide = "- 行動選項：更新。地點：禁止休息時自動換。\n- 行動選項：更新。";
    const merged = mergeRefactorInterfaces([statusbar("0"), frame], [], ROLES);
    const guide = merged.interface?.guide ?? "";
    expect(guide).toContain("- 行動選項：更新。地點：禁止休息時自動換。");
    expect(guide).not.toMatch(/^- 行動選項：更新。$/m);
  });

  it("拿掉定義容器後只收那個接縫的空行，骨架其他地方的空行原樣", () => {
    const definer = statusbar("0");
    definer.shell = `<Top>{{地點}}</Top>\n\n\n<Mid>固定</Mid>\n${DEFINED}\n\n<End>尾</End>`;
    const merged = mergeRefactorInterfaces([definer, format("4", "行動選項")], [], ROLES);
    expect(merged.interface?.shell).toContain("<Top>{{地點}}</Top>\n\n\n<Mid>固定</Mid>\n\n<End>尾</End>");
  });

  it("沒傳容器角色：維持舊合併（相容匯入的重構卡）", () => {
    expect(mergeRefactorInterfaces([statusbar("0"), format("4", "行動選項")]).conflict).toBe("行動選項");
  });

  it("接線：assembleRefactorOutcome 帶容器角色才讓位，不帶維持舊行為", () => {
    const parts = { characters: [], interfaces: [statusbar("0"), format("4", "行動選項")], entries: [] };
    expect(assembleRefactorOutcome({ ...parts, containerRoles: ROLES }).interface?.shell).toBe(
      `<maintext>\n{{本回合.正文}}\n</maintext>\n<BGM>{{BGM}}</BGM>\n ${DEFINED}`,
    );
    expect(assembleRefactorOutcome(parts).interface).toBeNull();
  });
});
