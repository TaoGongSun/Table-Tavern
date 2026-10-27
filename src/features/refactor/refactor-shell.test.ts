import yaml from "js-yaml";
import { describe, expect, it } from "vitest";
import {
  bodyRecordPrefix,
  classifyPlaceholder,
  fillSkeletonPlaceholders,
  spreadBodyRecord,
  type StateNode,
} from "./refactor-shell";

describe("fillSkeletonPlaceholders", () => {
  it("值原文放回、不 escape：清單欄位的內層標籤要留給卡的顯示腳本解析", () => {
    const tree: Record<string, StateNode> = { 物品: "<Item>短劍</Item><Item>火把</Item>" };

    expect(fillSkeletonPlaceholders("<Bag>{{物品}}</Bag>", tree)).toBe(
      "<Bag><Item>短劍</Item><Item>火把</Item></Bag>",
    );
  });

  it("巢狀路徑逐層查值", () => {
    const tree: Record<string, StateNode> = { World: { Time: "清晨" }, 亞瑟: { HP: "480/500" } };

    expect(fillSkeletonPlaceholders("{{World.Time}} / {{亞瑟.HP}}", tree)).toBe("清晨 / 480/500");
  });

  it("路徑查不到就換成空字串", () => {
    const tree: Record<string, StateNode> = { World: { Time: "清晨" } };

    expect(fillSkeletonPlaceholders("[{{World.Weather}}][{{Nope.Nothing}}]", tree)).toBe("[][]");
  });

  it("路徑落在分支節點（非葉子）也換成空字串", () => {
    const tree: Record<string, StateNode> = { World: { Time: "清晨" } };

    expect(fillSkeletonPlaceholders("[{{World}}]", tree)).toBe("[]");
  });

  it("佔位符內容含換行就不算佔位符，原樣保留", () => {
    const skeleton = "{{World.\nTime}}";
    const tree: Record<string, StateNode> = { World: { Time: "正午" } };

    expect(fillSkeletonPlaceholders(skeleton, tree)).toBe(skeleton);
  });

  it("同一份骨架裡多個佔位符各自替換", () => {
    const tree: Record<string, StateNode> = { a: "1", b: "2" };

    expect(fillSkeletonPlaceholders("{{a}}-{{b}}-{{a}}", tree)).toBe("1-2-1");
  });
});

describe("fillSkeletonPlaceholders（YAML 容器）", () => {
  // 照卡片的讀法：抽出 <Status_block> 內文交給 js-yaml 解析（4.3.2；卡片載入的 4.1.0 對這裡全部案例讀到的結果相同）
  const read = (filled: string) => {
    const body = /<Status_block>([\s\S]*?)<\/Status_block>/.exec(filled)?.[1] ?? "";
    return yaml.load(body) as Record<string, Record<string, unknown>>;
  };
  const fill = (skeleton: string, values: Record<string, string>, types: Record<string, string> = {}) =>
    fillSkeletonPlaceholders(skeleton, { s: values }, ["Status_block"], types);
  const block = (...lines: string[]) => ["<Status_block>", "状态栏:", ...lines, "</Status_block>"].join("\n");

  // 每一組值都塞進各種位置，卡讀到的必須等於原值（純數字讀成數字，比對字串形式）
  const tricky = [
    "北境驿站",
    '他說"走"\\後院',
    "It's 雪",
    "北境 # 後院",
    "key: value",
    "- 開頭像清單",
    "true",
    "null",
    "",
    "  前後空白  ",
    "第一行\n第二行",
    "320",
    "-20",
    "emoji 🐎 與 \t tab",
    // 會被 YAML 隱式轉型的字面：日期、科學記號、16 進位、無限大、null 寫法、YAML 1.1 布林
    "2026-10-03",
    "2026-10-03 12:00",
    "1e3",
    "0x1F",
    ".inf",
    "~",
    "Null",
    "yes",
    "<<",
  ];

  it.each(tricky)("雙引號、單引號、純量、夾在固定文字裡：值 %j 原樣讀回", (value) => {
    const filled = fill(
      block(
        '  雙引號: "{{s.v}}"',
        "  單引號: '{{s.v}}'",
        "  純量: {{s.v}}",
        "  前綴: 📍 {{s.v}}",
        '  前後綴: "🌾 {{s.v}}石"',
        "  前綴後綴: 第{{s.v}}號 # 固定註解",
      ),
      { v: value },
    );
    const read_ = read(filled).状态栏;
    expect(read_.雙引號).toBe(value);
    expect(read_.單引號).toBe(value);
    expect(read_.純量).toBe(value);
    expect(read_.前綴).toBe(`📍 ${value}`);
    expect(read_.前後綴).toBe(`🌾 ${value}石`);
    expect(read_.前綴後綴).toBe(`第${value}號`);
  });

  it("區塊純量：值原文放進去，多行值每行補縮排", () => {
    for (const value of ["true", "第一行\n第二行", "北境 # 後院", '"引號"']) {
      const filled = fill(block("  日志: |-", "    {{s.v}}", "  詩: |", "    {{s.v}}"), { v: value });
      expect(read(filled).状态栏.日志).toBe(value);
      expect(read(filled).状态栏.詩).toBe(`${value}\n`);
    }
  });

  it("佔位符是整個值的多行：YAML 結構照結構插入、文字寫成區塊", () => {
    const filled = fill(block("  人物列表: {{s.人}}", "  日志: {{s.日}}"), {
      人: "- 名字: 林远\n  身份: 驿丞\n- 名字: 贺铮\n  身份: 老驿长",
      日: "甲\n乙: 不是鍵",
    });
    expect(read(filled).状态栏).toEqual({
      人物列表: [
        { 名字: "林远", 身份: "驿丞" },
        { 名字: "贺铮", 身份: "老驿长" },
      ],
      日志: "甲\n乙: 不是鍵",
    });
  });

  it("整行只有佔位符：清單照原樣插在這個縮排", () => {
    const filled = fill(block("  人物列表:", "    {{s.人}}"), { 人: "- 林远\n- 贺铮" });
    expect(read(filled).状态栏.人物列表).toEqual(["林远", "贺铮"]);
    const one = fill(block("  人物列表:", "    {{s.人}}"), { 人: "- 林远" });
    expect(read(one).状态栏.人物列表).toEqual(["林远"]);
  });

  it("行內清單照卡的結構填：list 欄 `[{{x}}]` 填「甲, 乙」讀回兩個元素；引號內的元素照引號規則跳脫", () => {
    const filled = fill(
      block("  人物: [{{s.v}}]", '  对象: ["{{s.q}}", 固定]', "  表: {甲: {{s.n}}}"),
      { v: "甲, 乙", q: '說"走"', n: "1" },
      { "s.v": "list" },
    );
    expect(read(filled).状态栏).toEqual({ 人物: ["甲", "乙"], 对象: ['說"走"', "固定"], 表: { 甲: "1" } });
  });

  it("行內集合的元素套同一份型別／跳脫契約：含 # 與 ISO 日期的元素讀回原字串", () => {
    const filled = fill(
      block("  人物: [{{s.v}}]", "  表: {地: {{s.d}}, 註: {{s.c}}}", "  單: [{{s.d}}, 固定]"),
      { v: '"北境 # 後院", "2026-10-03"', d: "2026-10-03", c: "北境 # 後院" },
      { "s.v": "list" },
    );
    expect(read(filled).状态栏).toEqual({
      人物: ["北境 # 後院", "2026-10-03"],
      表: { 地: "2026-10-03", 註: "北境 # 後院" },
      單: ["2026-10-03", "固定"],
    });
  });

  it("集合片段只認 list 欄：引號與巢狀照 YAML 解析，非 list 欄的 `a, b` 是一個字串不拆", () => {
    const skeleton = block("  人物: [{{s.v}}]");
    const list = { "s.v": "list" };
    const cases: [string, Record<string, string>, unknown[]][] = [
      ['"a, b", c', list, ["a, b", "c"]],
      ["a, b", {}, ["a, b"]],
      ['"a, b", c', {}, ['"a, b", c']],
      // 全形逗號不是 YAML 分隔符號：list 欄也是一個元素
      ["甲，乙", list, ["甲，乙"]],
      ["甲，乙", {}, ["甲，乙"]],
      // 元素保留 YAML 讀到的型別；巢狀集合照結構
      ["300, true, 甲", list, [300, true, "甲"]],
      ["[a, b], {c: d}", list, [["a", "b"], { c: "d" }]],
      // 值本身已是序列：取它的元素，不多包一層
      ["[a, b]", list, ["a", "b"]],
      ["- 林远\n- 贺铮", list, ["林远", "贺铮"]],
      // 普通多行文字不是序列：一個字串元素，保留換行
      ["第一行\n第二行", list, ["第一行\n第二行"]],
      ["甲\n乙: 不是鍵", list, ["甲\n乙: 不是鍵"]],
      ["", list, []],
      // 解析不了的片段整份當一個字串元素
      ['"未閉合, 甲', list, ['"未閉合, 甲']],
      // 循環 alias、展開過大的 alias、YAML 無法照原樣表示的值：整份原值當一個字串元素
      ["&a [*a]", list, ["&a [*a]"]],
      ["&a [x, *a]", list, ["&a [x, *a]"]],
      [".inf, 1", list, [".inf, 1"]],
      // 非循環 alias 照展開後的值寫回
      ["&x 甲, *x", list, ["甲", "甲"]],
      // 合併鍵與 __proto__ 不污染原型，讀回自有屬性
      ["{<<: {a: 1}}, {__proto__: {polluted: 1}}", list, [{ "<<": { a: 1 } }, { ["__proto__"]: { polluted: 1 } }]],
    ];
    for (const [value, types, expected] of cases) {
      expect(read(fill(skeleton, { v: value }, types)).状态栏.人物).toEqual(expected);
    }
  });

  it("行內集合的單一元素：遵守型別表，含逗號或換行的值整個加引號不拆", () => {
    const skeleton = block("  單: [{{s.v}}]", "  前: [{{s.v}}, 固定]", "  表: {值: {{s.v}}}");
    const number = read(fill(skeleton, { v: "300" }, { "s.v": "number" })).状态栏;
    expect(number).toEqual({ 單: [300], 前: [300, "固定"], 表: { 值: 300 } });
    const text = read(fill(skeleton, { v: "300" })).状态栏;
    expect(text).toEqual({ 單: ["300"], 前: ["300", "固定"], 表: { 值: "300" } });
    for (const value of ["a, b", "甲\n乙", "[x]", "{y}"]) {
      expect(read(fill(skeleton, { v: value })).状态栏).toEqual({ 單: [value], 前: [value, "固定"], 表: { 值: value } });
    }
  });

  it("集合片段：alias 炸彈不展開成巨量輸出，整份原值當一個字串元素", () => {
    const levels = ["&a [x, x, x, x, x, x, x, x, x, x]"];
    for (let n = 1; n < 8; n++) levels.push(`&${"a".repeat(n + 1)} [${Array(10).fill(`*${"a".repeat(n)}`).join(", ")}]`);
    const value = levels.join(", ");
    const filled = fill(block("  人物: [{{s.v}}]"), { v: value }, { "s.v": "list" });
    expect(filled.length).toBeLessThan(value.length * 2);
    expect(read(filled).状态栏.人物).toEqual([value]);
  });

  it("集合片段解析不帶繼承屬性：__proto__ 是自有鍵，Object.prototype 不被污染", () => {
    const filled = fill(block("  人物: [{{s.v}}]"), { v: "{__proto__: {polluted: 1}}" }, { "s.v": "list" });
    expect(({} as Record<string, unknown>).polluted).toBeUndefined();
    const element = (read(filled).状态栏.人物 as Record<string, unknown>[])[0];
    expect(Object.keys(element)).toEqual(["__proto__"]);
  });

  it("行內集合的前後綴元素：整個元素一起表示，換行與指示字元原樣讀回", () => {
    const filled = fill(
      block("  表: [前綴{{s.v}}, 固定]", "  尾: [{{s.v}}後綴]", "  夾: {地: 第{{s.v}}號, 註: 固定}"),
      { v: "甲\n乙, [丙]" },
    );
    expect(read(filled).状态栏).toEqual({
      表: ["前綴甲\n乙, [丙]", "固定"],
      尾: ["甲\n乙, [丙]後綴"],
      夾: { 地: "第甲\n乙, [丙]號", 註: "固定" },
    });
    // 前後綴組成的值即使像數字也讀回字串
    expect(read(fill(block("  表: [1{{s.v}}]"), { v: "2" })).状态栏.表).toEqual(["12"]);
  });

  it("巨集混合元素：巨集 token 原樣保留、整個元素雙引號表示，動態值換行保留（行內清單、行內 mapping、區塊清單、區塊 mapping）", () => {
    const filled = fill(
      block("  n: [{{user}}前綴{{s.v}}]", "  m: {k: {{user}}前綴{{s.v}}, 固定: 值}", "  l:", "    - {{user}}前綴{{s.v}}", "  k: {{user}}前綴{{s.v}}"),
      { v: "甲\n乙" },
    );
    const literal = "{{user}}前綴甲\n乙";
    expect(read(filled).状态栏).toEqual({ n: [literal], m: { k: literal, 固定: "值" }, l: [literal], k: literal });
    const oneLine = fill(block("  n: [{{user}}前綴{{s.v}}]", "  k: {{user}}前綴{{s.v}}"), { v: "甲, 乙" });
    expect(read(oneLine).状态栏).toEqual({ n: ["{{user}}前綴甲, 乙"], k: "{{user}}前綴甲, 乙" });
  });

  it("行內集合的引號元素保留換行：單引號內含換行時改寫成雙引號純量", () => {
    const filled = fill(block("  对象: ['{{s.v}}', 固定]", '  雙: ["{{s.v}}", 固定]', "  混: ['第{{s.v}}號']"), {
      v: "甲\n乙's",
    });
    expect(read(filled).状态栏).toEqual({ 对象: ["甲\n乙's", "固定"], 雙: ["甲\n乙's", "固定"], 混: ["第甲\n乙's號"] });
    const oneLine = fill(block("  对象: ['{{s.v}}', 固定]"), { v: "乙's, 丙" });
    expect(read(oneLine).状态栏.对象).toEqual(["乙's, 丙", "固定"]);
  });

  it("行內集合：一般純量中間的引號不當引號開頭，行內註解原樣保留", () => {
    const filled = fill(block("  表: [it's {{s.v}}, 固定] # 註解 {{s.v}}"), { v: "雪" });
    expect(filled).toContain("# 註解 {{s.v}}");
    expect(read(filled).状态栏.表).toEqual(["it's 雪", "固定"]);
  });

  it("原卡欄位型別：number／bool 欄讀回數字／布林（算術欄可直接算），字串欄讀回字串；一般欄與行內集合一致", () => {
    const types = { "s.糧": "number", "s.開": "bool" };
    const filled = fillSkeletonPlaceholders(
      block("  粮草: {{s.糧}}", "  开仓: {{s.開}}", "  编号: {{s.号}}", "  表: {粮: {{s.糧}}, 号: {{s.号}}}"),
      { s: { 糧: "300", 開: "true", 号: "300" } },
      ["Status_block"],
      types,
    );
    const status = read(filled).状态栏 as Record<string, unknown>;
    expect(status).toEqual({ 粮草: 300, 开仓: true, 编号: "300", 表: { 粮: 300, 号: "300" } });
    expect((status.粮草 as number) - 20).toBe(280);
    // number 欄的值不是合法數字時照字串寫，不會讀成別的東西
    const odd = fillSkeletonPlaceholders(block("  粮草: {{s.糧}}"), { s: { 糧: "三百" } }, ["Status_block"], types);
    expect(read(odd).状态栏.粮草).toBe("三百");
  });

  it("容器開頭是註解或文件標記仍認得是 YAML", () => {
    for (const head of ["# 固定註解", "---", "--- # 註解"]) {
      const filled = fillSkeletonPlaceholders(
        `<Status_block>\n${head}\n状态栏:\n  地点: {{s.v}}\n</Status_block>`,
        { s: { v: "北境 # 後院" } },
        ["Status_block"],
      );
      expect(read(filled).状态栏.地点).toBe("北境 # 後院");
    }
  });

  it("YAML 容器裡的正文槽照正文原樣填，不加引號、不轉跳脫", () => {
    const filled = fillSkeletonPlaceholders(
      "<Status_block>\n状态栏:\n  地点: {{s.v}}\n正文：{{本回合.正文}}\n</Status_block>",
      { s: { v: "仓房" }, 本回合: { 正文: "他說：「走」" } },
      ["Status_block"],
    );
    expect(filled).toContain("正文：他說：「走」");
  });

  it("只裝正文槽的容器不當 YAML 容器", () => {
    const filled = fillSkeletonPlaceholders(
      "<maintext>\n{{本回合.正文}}\n</maintext>\n<Status_block>\n状态栏:\n  地点: {{s.v}}\n</Status_block>",
      { s: { v: "a # b" }, 本回合: { 正文: "甲: 乙\n丙" } },
      ["maintext", "Status_block"],
    );
    expect(filled).toContain("<maintext>\n甲: 乙\n丙\n</maintext>");
    expect(read(filled).状态栏.地点).toBe("a # b");
  });

  it("固定矩陣與白名單逐字保留", () => {
    const filled = fill(block('  地图: "▓▓░ 固定矩陣 ░▓▓"', "  白名单: [驿站, 马厩]", "  地点: {{s.v}}"), {
      v: "仓房",
    });
    expect(filled).toContain('  地图: "▓▓░ 固定矩陣 ░▓▓"');
    expect(filled).toContain("  白名单: [驿站, 马厩]");
    expect(read(filled).状态栏).toEqual({ 地图: "▓▓░ 固定矩陣 ░▓▓", 白名单: ["驿站", "马厩"], 地点: "仓房" });
  });

  it("YAML 只用在指定容器內：正文槽與其他格式照原樣填", () => {
    const skeleton = [
      "<maintext>",
      "{{本回合.正文}}",
      "</maintext>",
      "<Status_block>",
      "状态栏:",
      "  地点: {{s.v}}",
      "</Status_block>",
      "<Note>{{s.v}}</Note>",
    ].join("\n");
    const filled = fillSkeletonPlaceholders(
      skeleton,
      { s: { v: "北境 # 後院" }, 本回合: { 正文: "甲\n乙" } },
      ["Status_block"],
    );
    expect(filled).toContain("<maintext>\n甲\n乙\n</maintext>");
    expect(filled).toContain("<Note>北境 # 後院</Note>");
    expect(read(filled).状态栏.地点).toBe("北境 # 後院");
  });

  it("沒指定容器時整份照原樣填（別張卡提到 YAML 不影響）", () => {
    expect(fillSkeletonPlaceholders('地点: "{{s.x}}"', { s: { x: 'a"b' } })).toBe('地点: "a"b"');
    expect(
      fillSkeletonPlaceholders("<Status_block>\n地点: {{s.x}}\n</Status_block>", { s: { x: "a # b" } }, ["Other"]),
    ).toBe("<Status_block>\n地点: a # b\n</Status_block>");
  });

  it("狀態葉子優先於巨集名；未知冒號寫法當路徑（查不到就是空值）", () => {
    expect(fillSkeletonPlaceholders("{{Time}}／{{time}}", { Time: "黃昏" })).toBe("黃昏／{{time}}");
    expect(fillSkeletonPlaceholders("[{{World:Missing}}]", { World: { Missing: "x" } })).toBe("[]");
    expect(fillSkeletonPlaceholders("{{getvar::x}}", {})).toBe("{{getvar::x}}");
  });

  it("原卡巨集原樣保留，不當狀態路徑清空", () => {
    const filled = fill(block('  名字: "👤 {{user}}"', "  对象: {{char}}", "  地点: {{s.v}}"), { v: "仓房" });
    expect(read(filled).状态栏).toEqual({ 名字: "👤 {{user}}", 对象: "{{char}}", 地点: "仓房" });
    expect(fillSkeletonPlaceholders("{{user}} 在 {{s.v}}，{{random::甲::乙}}", { s: { v: "仓房" } })).toBe(
      "{{user}} 在 仓房，{{random::甲::乙}}",
    );
  });

  it("{{user}}／{{char}} 給了名字就照值的位置規則填：名字含引號、換行、冒號、井號時卡照樣讀回原名", () => {
    const names = ['他說"走"\\後院', "It's 雪", "北境 # 後院", "key: value", "- 開頭", "true", "12", "甲\n乙", "{{char}}$1"];
    for (const user of names) {
      const filled = fillSkeletonPlaceholders(
        block(
          '  名字: "👤 {{user}}"',
          "  对象: {{char}}",
          "  單: '{{ User }}的'",
          "  純: {{user}}前綴{{s.v}}",
          "  n: [{{user}}, {{s.v}}]",
          "  m: {k: {{user}}}",
          "  l:",
          "    - {{user}}",
          "  b: |",
          "    {{user}}",
          "  {{user}}: 鍵",
        ),
        { s: { v: "仓房" } },
        ["Status_block"],
        {},
        {},
        { user, char: user },
      );
      expect(read(filled).状态栏).toEqual({
        名字: `👤 ${user}`,
        对象: user,
        單: `${user}的`,
        純: `${user}前綴仓房`,
        n: [user, "仓房"],
        m: { k: user },
        l: [user],
        b: `${user}\n`,
        [user]: "鍵",
      });
    }
  });

  it("名字是空的、或狀態樹真有 user 葉子時：巨集保留字面／狀態值優先", () => {
    expect(fillSkeletonPlaceholders("{{user}}與{{char}}", {}, [], {}, {}, { user: " ", char: "" })).toBe("{{user}}與{{char}}");
    expect(fillSkeletonPlaceholders("{{user}}與{{char}}", { user: "樹上的值" }, [], {}, {}, { user: "旅人", char: "莫拉" })).toBe(
      "樹上的值與莫拉",
    );
  });
});

describe("classifyPlaceholder", () => {
  const leaves = new Set(["Time", "World.Time", "状态栏.地点"]);
  const isLeaf = (path: string) => leaves.has(path);
  it.each([
    ["本回合.正文", "body"],
    ["user", "macro"],
    ["Char", "macro"],
    ["lastMessageId", "macro"],
    ["random::a::b", "macro"],
    ["roll:1d6", "macro"],
    ["getvar::hp", "macro"],
    ["Time", "path"],
    ["time", "macro"],
    ["World:Missing", "path"],
    ["状态栏.地点", "path"],
    ["人物列表", "path"],
  ])("%s → %s", (token, kind) => {
    expect(classifyPlaceholder(token, isLeaf)).toBe(kind);
  });
});

describe("fillSkeletonPlaceholders（記錄式正文槽）", () => {
  const body = (text: string): Record<string, StateNode> => ({ 本回合: { 正文: text } });
  const fill = (skeleton: string, text: string) => fillSkeletonPlaceholders(skeleton, body(text));

  it("`欄位|{{本回合.正文}}`：正文每段一筆記錄、丟空白行，已帶前綴的行不重補", () => {
    const skeleton = "<mainPage>\nmainText|{{本回合.正文}}\nnpc|村田\n</mainPage>";
    expect(fill(skeleton, "第一段\n\n第二段\r\n第三段\nmainText|第四段")).toBe(
      "<mainPage>\nmainText|第一段\nmainText|第二段\nmainText|第三段\nmainText|第四段\nnpc|村田\n</mainPage>",
    );
    expect(fill("  mainText| {{ 本回合.正文 }}  ", "甲\n乙")).toBe("  mainText| 甲\n  mainText| 乙");
  });

  it("不是記錄式就逐字照現狀：多佔位、非行尾、標籤包住、其他分隔、全空正文", () => {
    const tree = { ...body("甲\n乙"), x: "X" };
    for (const skeleton of [
      "mainText|{{本回合.正文}}|{{x}}",
      "mainText|{{本回合.正文}}。",
      "<maintext>{{本回合.正文}}</maintext>",
      "{{本回合.正文}}",
      "正文：{{本回合.正文}}",
      "x|{{x}}",
    ]) {
      const plain = skeleton.replace("{{本回合.正文}}", "甲\n乙").replace("{{x}}", "X");
      expect(fillSkeletonPlaceholders(skeleton, tree)).toBe(plain);
    }
    expect(fill("mainText|{{本回合.正文}}", "")).toBe("mainText|");
    expect(fill("mainText|{{本回合.正文}}", "\n \n")).toBe("mainText|\n \n");
  });

  it("YAML 容器裡的正文槽不套記錄式規則", () => {
    const skeleton = "<Status_block>\n状态栏:\n  說明: x|{{本回合.正文}}\n</Status_block>";
    expect(bodyRecordPrefix(skeleton, ["Status_block"])).toBeNull();
    expect(bodyRecordPrefix("<p>\nmainText|{{本回合.正文}}\n</p>")).toBe("mainText|");
    expect(spreadBodyRecord("mainText|", "a\n\nb")).toBe("mainText|a\nmainText|b");
    expect(spreadBodyRecord("mainText|", "  mainText|a\nb")).toBe("mainText|a\nmainText|b");
  });
});

describe("fillSkeletonPlaceholders（YAML 鍵位置的佔位符）", () => {
  const read = (filled: string) => {
    const content = /<Status_block>([\s\S]*?)<\/Status_block>/.exec(filled)?.[1] ?? "";
    return yaml.load(content) as Record<string, unknown>;
  };
  const fill = (lines: string[], values: Record<string, string>) =>
    fillSkeletonPlaceholders(["<Status_block>", ...lines, "</Status_block>"].join("\n"), { s: values }, ["Status_block"]);

  // RPGImmortal 骨架的最小重現：地圖類型是鍵，小地圖元素是下一行整行插入的多行清單
  const map = "- \"👹【赤魁】(35,45) 站起身\"\n- \"🪖【李管事】(50,15) 抱著花名冊\"\n";

  it("動態鍵下接多行清單：讀回 {洞府: [...]}（RPGImmortal 回歸）", () => {
    const filled = fill(
      ["状态栏:", "  用户列表:", "    - 用户:", '        名字: " 🗺️小地图"', "        {{s.類型}}:", "           {{s.元素}}"],
      { 類型: "洞府", 元素: map },
    );
    expect(read(filled)).toEqual({
      状态栏: { 用户列表: [{ 用户: { 名字: " 🗺️小地图", 洞府: ["👹【赤魁】(35,45) 站起身", "🪖【李管事】(50,15) 抱著花名冊"] } }] },
    });
  });

  it("容器首行就是動態鍵也認成 YAML 容器", () => {
    expect(read(fill(["{{s.類型}}:", "  {{s.元素}}"], { 類型: "洞府", 元素: map }))).toEqual({
      洞府: ["👹【赤魁】(35,45) 站起身", "🪖【李管事】(50,15) 抱著花名冊"],
    });
  });

  it("鍵裡夾固定文字、清單項的鍵、鍵值都是佔位", () => {
    const filled = fill(
      ["第{{s.n}}层:", "  - a", "NPC_{{s.k}}: v", "列表:", "  - {{s.k}}: {{s.v}}", "    其他: 1", "  - {{s.k}}: {{s.n}}"],
      { n: "3", k: "甲", v: "第一行\n第二行" },
    );
    expect(read(filled)).toEqual({
      第3层: ["a"],
      NPC_甲: "v",
      列表: [{ 甲: "第一行\n第二行", 其他: 1 }, { 甲: "3" }],
    });
  });

  it("引號鍵與區塊指示符讀回原值", () => {
    const filled = fill(['"{{s.q}}": 雙', "'{{s.r}}': 單", "{{s.k}}詩: |", "  {{s.v}}", "{{s.k}}註: # {{s.v}}", "  x: 1"], {
      q: 'a"b\'c',
      r: "it's",
      k: "甲",
      v: "第一行\n第二行",
    });
    expect(read(filled)).toEqual({ [`a"b'c`]: "雙", "it's": "單", 甲詩: "第一行\n第二行\n", 甲註: { x: 1 } });
  });

  it("清單項裡的區塊純量（動態／固定鍵、| 與 >）：同層下一個欄位不被吃進區塊", () => {
    for (const indicator of ["|", ">"]) {
      for (const head of ["{{s.k}}", "固定"]) {
        const filled = fill(
          ["列表:", `  - ${head}: ${indicator}`, "      {{s.text}}", "    sibling: {{s.v}}", "    其他: 1"],
          { k: "固定", text: "第一行\n第二行", v: "a: b" },
        );
        const text = indicator === "|" ? "第一行\n第二行\n" : "第一行 第二行\n";
        expect(read(filled)).toEqual({ 列表: [{ 固定: text, sibling: "a: b", 其他: 1 }] });
      }
    }
  });

  it("巢狀清單項的鍵：`- - {{k}}: v` 鍵名正確、多行對映子層縮排正確", () => {
    const filled = fill(["- - {{s.k}}: {{s.v}}", "  - {{s.k}}: {{s.m}}"], { k: "甲", v: "乙", m: "x: a\ny: b" });
    expect(read(filled)).toEqual([[{ 甲: "乙" }, { 甲: { x: "a", y: "b" } }]]);
  });

  it("無鍵的巢狀清單項填多行值：對映、文字、清單各自讀回", () => {
    const values = { m: "x: a\ny: b", t: "第一行\n第二行", l: "- a\n- b" };
    expect(read(fill(["列表:", "  - - {{s.m}}"], values))).toEqual({ 列表: [[{ x: "a", y: "b" }]] });
    expect(read(fill(["列表:", "  - - {{s.t}}"], values))).toEqual({ 列表: [["第一行\n第二行"]] });
    expect(read(fill(["列表:", "  - - {{s.l}}"], values))).toEqual({ 列表: [[["a", "b"]]] });
    // 沒記號也沒鍵的整行佔位：輸出與改動前逐字相同
    expect(fill(["列表:", "  {{s.l}}", "說明:", "    {{s.t}}"], values)).toBe(
      "<Status_block>\n列表:\n  - a\n  - b\n說明:\n    第一行\n    第二行\n</Status_block>",
    );
  });

  it("清單項裡的鍵填多行對映與清單：讀回巢狀結構", () => {
    const filled = fill(
      ["- 固定: {{s.m}}", "  旁: 1", "- 清單: {{s.l}}", "- {{s.k}}: {{s.m}}"],
      { k: "甲", m: "x: a\ny: b", l: "- a\n- b" },
    );
    expect(read(filled)).toEqual([{ 固定: { x: "a", y: "b" }, 旁: 1 }, { 清單: ["a", "b"] }, { 甲: { x: "a", y: "b" } }]);
  });

  it("鍵是佔位、值是固定文字：值照原樣，型別不變", () => {
    expect(read(fill(["{{s.a}}: 1", "{{s.b}}: true", "{{s.c}}: null", "{{s.d}}: 文字 # 註"], { a: "a", b: "b", c: "c", d: "d" }))).toEqual({
      a: 1,
      b: true,
      c: null,
      d: "文字",
    });
  });

  it("特殊鍵值讀回實際鍵名", () => {
    for (const key of ["", "第一行\n第二行", "true", "<<", "  前後空白  ", "a: b", "x #y", "null", "123"]) {
      const parsed = read(fill(["{{s.k}}: 值", "固定: 1"], { k: key }));
      expect(Object.keys(parsed)).toEqual([key.trim(), "固定"]);
      expect(parsed[key.trim()]).toBe("值");
    }
  });

  it("不誤判成鍵：引號值裡的冒號、行內集合、引號清單項輸出不變", () => {
    const values = { k: "甲", v: "乙", t: "子時" };
    expect(fill(['- "{{s.k}}: {{s.v}}"'], values)).toBe('<Status_block>\n- "甲: 乙"\n</Status_block>');
    expect(read(fill(['時間: "{{s.t}}: 現在"'], values))).toEqual({ 時間: "子時: 現在" });
    expect(read(fill(["- \"{{s.t}}: 現在\""], values))).toEqual(["子時: 現在"]);
    expect(read(fill(["m: { {{s.k}}: v }"], values))).toEqual({ m: { 甲: "v" } });
  });

  it("註解行的值換行換空白、不影響結構", () => {
    expect(fill(["# 注意 {{s.v}}", "a: 1"], { v: "x\ny" })).toBe("<Status_block>\n# 注意 x y\na: 1\n</Status_block>");
  });
});
