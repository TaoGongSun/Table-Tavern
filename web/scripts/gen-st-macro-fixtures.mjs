// 產生巨集對拍案例：用網頁版實作跑 st-macro-fixture-cases.ts 的輸入，把預期值寫進
// src/shared/contracts/st-macros/web-macro-cases.json。測試只比對、不改寫。用法（在 web/ 底下）：node scripts/gen-st-macro-fixtures.mjs
// 時區固定 Asia/Taipei（runner 的 FIXTURE_TIMEZONE），要在載入任何模組前設好。
process.env.TZ = "Asia/Taipei";
import { writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { createServer } from "vite";

const WEB = fileURLToPath(new URL("..", import.meta.url));
const OUT = fileURLToPath(new URL("../../src/shared/contracts/st-macros/web-macro-cases.json", import.meta.url));

const server = await createServer({ root: WEB, configFile: `${WEB}vite.config.ts`, server: { middlewareMode: true }, appType: "custom", logLevel: "error", optimizeDeps: { noDiscovery: true, entries: [] } });
try {
  const { macroCases } = await server.ssrLoadModule("/scripts/st-macro-fixture-cases.ts");
  const runner = await server.ssrLoadModule("/src/features/sillytavern/macro-parity-runner.ts");
  if (runner.FIXTURE_TIMEZONE !== process.env.TZ) throw new Error("時區與 runner 不一致");
  const body = {
    _about:
      "網頁版 substituteParams 對拍：預設卡、對話與 chatId 見 macro-parity-runner.ts；時鐘固定、時區 Asia/Taipei（+480 分）；random 依序取用、用完是 0。expected 是輸出、代換後兩個變數表的 JSON 原文與亂數用量。預期值由網頁版跑出（web/scripts/gen-st-macro-fixtures.mjs）。",
    timezone: runner.FIXTURE_TIMEZONE,
    utcOffsetMinutes: runner.FIXTURE_UTC_OFFSET_MINUTES,
    nowMs: runner.FIXTURE_NOW_MS,
    defaultCard: runner.DEFAULT_CARD,
    defaultChat: runner.DEFAULT_CHAT,
    cases: macroCases.map((item) => ({ ...item, expected: runner.runMacroCase(item) })),
  };
  writeFileSync(OUT, `${JSON.stringify(body, null, 1)}\n`);
  console.log(`web-macro-cases.json: ${macroCases.length} cases`);
} finally {
  await server.close();
}
