// 網頁版端對端（npm run e2e）：本機假端點＋正式建置（端點換成假端點）＋WebKit。
// 走一遍：授權→選卡→串流→送出互斥→取消未完成回合→停止並保留→匯入卡（錯誤說明、選開場白、玩家名、
// ST 提示組裝、重新生成／編輯／刪除最後一則）→額度用完導流→下載連結→登出。
// 不連任何外部服務、不花額度。不進 verify（CI 沒裝瀏覽器）。
import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";
import { build, preview } from "vite";
import { webkit } from "playwright";
import { E2E_KEY, startFake } from "./fake-endpoints.mjs";

const ROOT = fileURLToPath(new URL("..", import.meta.url));
const CARDS = fileURLToPath(new URL("../../src/shared/contracts/card-view/", import.meta.url));
const RELEASES_PAGE = "https://github.com/TaoGongSun/Table-Tavern/releases";

const fake = await startFake();
process.env.VITE_TT_OPENROUTER_API = `${fake.url}/api/v1`;
process.env.VITE_TT_OPENROUTER_AUTH = `${fake.url}/auth`;
process.env.VITE_TT_GITHUB_API = `${fake.url}/github`;

// 只有 e2e 模式才接受 VITE_TT_* 覆寫（src/shared/endpoints/resolve.ts）
await build({ root: ROOT, mode: "e2e", logLevel: "warn", build: { outDir: "dist-e2e", emptyOutDir: true } });
const server = await preview({ root: ROOT, logLevel: "warn", build: { outDir: "dist-e2e" }, preview: { port: 4317, strictPort: false } });
const base = server.resolvedUrls.local[0];

const browser = await webkit.launch();
const page = await browser.newPage();
const consoleErrors = [];
page.on("console", (message) => message.type() === "error" && consoleErrors.push(message.text()));
const step = (name) => console.log(`• ${name}`);

try {
  step("未登入：常駐下載連結退回 releases 頁（目前沒有正式版）");
  await page.goto(base);
  await page.getByText("連接 OpenRouter 就能開始玩").waitFor();
  assert.equal(await page.getByTestId("download-link").getAttribute("href"), RELEASES_PAGE);

  step("PKCE 授權：回呼後網址清乾淨、金鑰只進 localStorage");
  await page.getByRole("button", { name: "用 OpenRouter 登入" }).click();
  await page.getByText("選一張角色卡開始").waitFor();
  assert.ok(!/code=|tt_oauth=/.test(page.url()), `回呼參數沒清掉：${page.url()}`);
  assert.equal(fake.state.exchanges.length, 1, "授權碼應只交換一次");
  assert.equal(await page.evaluate(() => localStorage.getItem("tt-web:openrouter-key")), E2E_KEY);
  await page.getByText("今日免費 40/50").waitFor();

  step("選範例卡：開場白出現");
  await page.getByRole("button", { name: "開始" }).click();
  await page.getByText("鞋上的雪先跺乾淨").waitFor();

  step("串流聊天：回覆中只有停止鈕（送出互斥），完成後落進逐字稿");
  await page.getByPlaceholder("輸入你的行動或對話…").fill("你好");
  await page.getByRole("button", { name: "送出" }).click();
  await page.getByRole("button", { name: "停止" }).waitFor();
  assert.equal(await page.getByRole("button", { name: "送出" }).count(), 0);
  await page.getByText("雪還在下，先喝口湯暖暖身子吧。").waitFor();
  await page.getByRole("button", { name: "送出" }).waitFor();
  const request = fake.state.chatRequests.at(-1);
  assert.equal(request.model, "alpha/one:free", "應選 RP 排行第一的穩定免費模型");
  assert.equal(request.messages.at(-1).content, "你好");

  step("取消未完成回合：一個字都沒出來就停，玩家句回到輸入框");
  fake.state.chat = "hang";
  await page.getByPlaceholder("輸入你的行動或對話…").fill("第二句");
  await page.getByRole("button", { name: "送出" }).click();
  await page.getByText("瑟拉 正在回覆…").waitFor();
  await page.getByRole("button", { name: "停止" }).click();
  await page.getByRole("button", { name: "送出" }).waitFor();
  assert.equal(await page.getByPlaceholder("輸入你的行動或對話…").inputValue(), "第二句");
  assert.equal(await page.getByTestId("message-user").count(), 1);

  step("停止並保留：已有正文就留下並標回應中斷，玩家句不刪");
  fake.state.chat = "partial-hang";
  await page.getByRole("button", { name: "送出" }).click();
  await page.getByTestId("message-streaming").getByText("說到一半").waitFor();
  await page.getByRole("button", { name: "停止" }).click();
  await page.getByText("回應中斷").waitFor();
  assert.equal(await page.getByTestId("message-user").count(), 2);
  assert.equal(await page.getByPlaceholder("輸入你的行動或對話…").inputValue(), "");

  step("匯入卡：只有 iTXt 的 PNG 說明讀不到卡資料");
  await page.getByRole("button", { name: "← 換一張卡" }).click();
  await page.getByLabel("你在故事裡的名字").fill("旅人");
  await page.getByTestId("card-file").setInputFiles(`${CARDS}itxt-only.png`);
  await page.getByTestId("import-error").getByText("這張 PNG 圖裡沒有角色卡資料").waitFor();

  step("匯入複合卡 PNG：列出三個開場白，選第二個、帶入玩家名");
  await page.getByTestId("card-file").setInputFiles(`${CARDS}composite.png`);
  await page.getByRole("heading", { name: "灰燼旅店的莫拉" }).waitFor();
  assert.equal(await page.locator('input[name="opening"]').count(), 3, "三個開場白");
  await page.getByText("深夜，灰燼旅店的莫拉 擦著最後一個杯子。").click();
  // D21：卡內 regex 腳本照 ST 預設不允許，玩家勾了才套
  const allowRegex = page.getByTestId("import-regex-allow");
  assert.equal(await allowRegex.isChecked(), false, "regex 腳本預設不允許");
  await allowRegex.check();
  await page.getByRole("button", { name: "開始這張卡" }).click();
  await page.getByTestId("message-char").getByText("深夜，灰燼旅店的莫拉 擦著最後一個杯子。").waitFor();

  step("ST 提示組裝：main→description→personality→scenario→範例→歷史（depth_prompt）→post_history_instructions");
  fake.state.chat = "numbered";
  const sendFrom = fake.state.chatRequests.length;
  await page.getByPlaceholder("輸入你的行動或對話…").fill("*揮手* 晚安");
  await page.getByRole("button", { name: "送出" }).click();
  await page.getByText(`第 ${sendFrom + 1} 次回覆`).waitFor();
  const sent = fake.state.chatRequests.at(-1).messages;
  assert.equal(sent[0].content, "Write 灰燼旅店的莫拉's next reply in a fictional chat between 灰燼旅店的莫拉 and 旅人. 一律用繁體中文。");
  assert.deepEqual(sent[5], { role: "system", name: "example_user", content: "還有房間嗎？" });
  assert.deepEqual(sent.slice(-5), [
    { role: "system", content: "[Start a new Chat]" },
    { role: "system", content: "記得 灰燼旅店的莫拉 說話總帶一句「親愛的」。" },
    { role: "assistant", content: "深夜，灰燼旅店的莫拉 擦著最後一個杯子。" },
    { role: "user", content: "（揮手） 晚安" },
    { role: "system", content: "回覆最後一行固定寫 <StatusPlaceHolderImpl/>。" },
  ]);
  await page.getByTestId("message-user").getByText("（揮手） 晚安").waitFor();

  step("重新生成：換掉最後一則回覆");
  const regenerateFrom = fake.state.chatRequests.length;
  await page.getByRole("button", { name: "重新生成" }).click();
  await page.getByText(`第 ${regenerateFrom + 1} 次回覆`).waitFor();
  assert.equal(await page.getByText(`第 ${regenerateFrom} 次回覆`).count(), 0, "舊回覆應被換掉");
  assert.equal(fake.state.chatRequests.at(-1).messages.at(-2).content, "（揮手） 晚安", "重新生成不帶舊回覆");

  step("編輯最後一則（巨集代換）、刪除最後一則");
  await page.getByTestId("last-actions").getByRole("button", { name: "編輯" }).click();
  await page.locator(".message-edit textarea").fill("改過的回覆，{{user}}。");
  await page.getByRole("button", { name: "儲存" }).click();
  await page.getByText("改過的回覆，旅人。").waitFor();
  await page.getByTestId("last-actions").getByRole("button", { name: "刪除" }).click();
  assert.equal(await page.getByText("改過的回覆，旅人。").count(), 0);
  assert.equal(await page.getByTestId("message-char").count(), 1, "只剩開場白");
  fake.state.chat = "normal";

  step("今日免費用完：送出前查 /key，跳導流面板、原文留在輸入框");
  fake.state.remaining = 0;
  fake.state.chat = "normal";
  await page.getByPlaceholder("輸入你的行動或對話…").fill("第四句");
  await page.getByRole("button", { name: "送出" }).click();
  await page.getByTestId("quota-panel").waitFor();
  assert.equal(await page.getByRole("link", { name: "前往下載頁" }).getAttribute("href"), RELEASES_PAGE);
  assert.equal(await page.getByPlaceholder("輸入你的行動或對話…").inputValue(), "第四句");
  await page.getByRole("button", { name: "知道了" }).click();
  assert.ok(await page.getByRole("button", { name: "送出" }).isDisabled(), "用完時送出鈕要停用");

  step("有正式版：下載連結帶版本號、導流面板給各平台安裝檔");
  fake.state.release = "available";
  await page.reload();
  await page.getByText("下載桌面版 v0.3.0").waitFor();
  await page.getByTestId("quota-panel").waitFor();
  assert.match(await page.getByRole("link", { name: "下載 Windows 版" }).getAttribute("href"), /_x64-setup\.exe$/);
  assert.match(await page.getByRole("link", { name: "下載 macOS 版（Apple Silicon）" }).getAttribute("href"), /_aarch64\.dmg$/);
  await page.getByRole("button", { name: "知道了" }).click();

  step("登出：清掉金鑰、回到登入畫面");
  await page.getByRole("button", { name: "登出" }).click();
  await page.getByText("連接 OpenRouter 就能開始玩").waitFor();
  assert.equal(await page.evaluate(() => localStorage.getItem("tt-web:openrouter-key")), null);

  const csp = consoleErrors.filter((text) => /Content Security Policy|Refused to/i.test(text));
  assert.deepEqual(csp, [], "不該有 CSP 擋下的資源");
  console.log("✓ e2e 全部通過");
} finally {
  await browser.close();
  await server.close();
  await fake.close();
}
