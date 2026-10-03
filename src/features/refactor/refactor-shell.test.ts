import yaml from "js-yaml";
import { describe, expect, it } from "vitest";
import { classifyPlaceholder, fillSkeletonPlaceholders, type StateNode } from "./refactor-shell";

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
