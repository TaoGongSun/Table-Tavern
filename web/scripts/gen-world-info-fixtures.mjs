// 產生世界書對拍案例：用網頁版實作跑 world-info-fixture-cases.ts 的輸入，把預期值寫進
// src/shared/contracts/world-info/。測試只比對、不改寫這些檔。用法（在 web/ 底下）：node scripts/gen-world-info-fixtures.mjs
import { writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { createServer } from "vite";

const WEB = fileURLToPath(new URL("..", import.meta.url));
const OUT = fileURLToPath(new URL("../../src/shared/contracts/world-info/", import.meta.url));

const server = await createServer({ root: WEB, configFile: `${WEB}vite.config.ts`, server: { middlewareMode: true }, appType: "custom", logLevel: "error", optimizeDeps: { noDiscovery: true, entries: [] } });
try {
  const cases = await server.ssrLoadModule("/scripts/world-info-fixture-cases.ts");
  const runner = await server.ssrLoadModule("/src/features/sillytavern/world-info-parity-runner.ts");
  const write = (name, about, items, run) => {
    const body = { _about: about, cases: items.map((input) => ({ ...input, expected: run(input) })) };
    writeFileSync(`${OUT}${name}`, `${JSON.stringify(body, null, 1)}\n`);
    console.log(`${name}: ${items.length} cases`);
  };
  write(
    "scan-cases.json",
    "checkWorldInfo 對拍：條目是物件形原始 JSON（掃描前照 sortByOrder 排序）、chat 新到舊、maxContext null＝沒有上限；代換把 {{user}}→Alice、{{char}}→Bob 並記下每次輸入；計數＝code point 數；random 依序取用。預期值由網頁版跑出（web/scripts/gen-world-info-fixtures.mjs）。",
    cases.scanCases,
    runner.runScanCase,
  );
  write(
    "regex-cases.json",
    "parseRegexFromString＋test 對拍：expected 是 JS 的結果；knownDifference 是桌面版刻意不同的地方（見方案七）。",
    cases.regexCases,
    runner.runRegexCase,
  );
  write(
    "sort-cases.json",
    "sortByOrder（ST sortFn，order 已正規化成有限數字）的穩定排序結果：expected 是排序後的原索引。",
    cases.sortCases,
    runner.runSortCase,
  );
  write("entry-cases.json", "單一條目 → WiEntry 欄位＋裝飾：worldFile＝fromWorldFile，characterBook＝fromCharacterBook。", cases.entryCases, runner.runEntryCase);
  const nextTurn = await server.ssrLoadModule("/scripts/web-save-next-turn.ts");
  const expected = nextTurn.webSaveNextTurn();
  writeFileSync(
    `${OUT}web-save-next-turn.json`,
    `${JSON.stringify({ _about: "網頁存檔端對端：網頁版讀 ../web-save/" + expected.save + "、玩家再送 nextUser，下一輪內文進了提示的條目（穩定 ID）與掃完的計時表。桌面版匯入同一份存檔、補同一句，角色視角掃一次要相同（方案四之 4）。產生：web/scripts/gen-world-info-fixtures.mjs。", ...expected }, null, 1)}\n`,
  );
  console.log("web-save-next-turn.json");
} finally {
  await server.close();
}
