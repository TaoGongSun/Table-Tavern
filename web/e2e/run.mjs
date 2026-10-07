// 網頁版端對端（npm run e2e）：本機假端點＋正式建置（端點換成假端點）＋WebKit。
// 走一遍：授權→選卡→串流→送出互斥→取消未完成回合→停止並保留→自動存檔（重新整理後繼續）→匯出網頁存檔
// （契約檢查、用桌面版繼續導流）→匯出 ST 聊天檔→存檔區刪除與匯入→匯入卡（錯誤說明、選開場白、玩家名、
// ST 提示組裝、重新生成／編輯／刪除最後一則）→額度用完導流→下載頁與下載連結→找卡清單→登出→英文瀏覽器走一遍與換語系。
// 不連任何外部服務、不花額度。不進 verify（CI 沒裝瀏覽器）。
// TT_WEB_EXPORT_OUT＝路徑：把這次真匯出的網頁存檔另存一份（桌面版來回測試的 fixture 由它產生）；
// TT_WEB_EXPORT_WI_OUT 同理，另存帶世界書觸發狀態的那一份；TT_WEB_EXPORT_MVU_OUT 另存帶 MVU 與卡片設定的那一份。
import assert from "node:assert/strict";
import { copyFileSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { parseWebSave } from "../../src/shared/contracts/web-save/web-save.ts";
import { join } from "node:path";
import { build } from "vite";
import { webkit } from "playwright";
import { E2E_KEY, startFake } from "./fake-endpoints.mjs";
import { startStaticServer } from "./static-server.mjs";

const ROOT = fileURLToPath(new URL("..", import.meta.url));
const CARDS = fileURLToPath(new URL("../../src/shared/contracts/card-view/", import.meta.url));
const SAVES = fileURLToPath(new URL("../../src/shared/contracts/web-save/", import.meta.url));
const E2E_DIR = fileURLToPath(new URL("./", import.meta.url));
const RELEASES_PAGE = "https://github.com/TaoGongSun/Table-Tavern/releases";

const fake = await startFake();
process.env.VITE_TT_OPENROUTER_API = `${fake.url}/api/v1`;
process.env.VITE_TT_OPENROUTER_AUTH = `${fake.url}/auth`;
process.env.VITE_TT_GITHUB_API = `${fake.url}/github`;

// 只有 e2e 模式才接受 VITE_TT_* 覆寫（src/shared/endpoints/resolve.ts）
await build({ root: ROOT, mode: "e2e", logLevel: "warn", build: { outDir: "dist-e2e", emptyOutDir: true } });
// 照 Cloudflare Pages 的行為託管產物（套 `_headers`、.html 轉無副檔名），標頭與部署時同一份
const server = await startStaticServer(join(ROOT, "dist-e2e"));
const base = server.url;

const browser = await webkit.launch();
// 主流程照繁中走（語系照瀏覽器偵測，固定成 zh-TW）；最後另開一個英文瀏覽器走一遍主要畫面
const page = await browser.newPage({ locale: "zh-TW" });
const consoleErrors = [];
page.on("console", (message) => message.type() === "error" && consoleErrors.push(message.text()));
const step = (name) => console.log(`• ${name}`);

try {
  step("站點標頭：宿主頁帶 CSP（只連假端點）、沙盒那條路徑只限本站嵌入、帶 hash 的資產長快取、缺檔回 404");
  const hostHeaders = (await fetch(base)).headers;
  assert.match(hostHeaders.get("content-security-policy"), new RegExp(`connect-src ${new URL(fake.url).origin} ${new URL(fake.url).origin};`));
  assert.match(hostHeaders.get("content-security-policy"), /frame-ancestors 'none'/);
  assert.equal(hostHeaders.get("cache-control"), "public, max-age=0, must-revalidate", "首頁用 Pages 預設、每次重新驗證");
  const sandboxRedirect = await fetch(new URL("sandbox.html?x=1", base), { redirect: "manual" });
  assert.equal(sandboxRedirect.status, 308);
  assert.equal(sandboxRedirect.headers.get("location"), "/sandbox?x=1");
  const sandboxResponse = await fetch(new URL("sandbox", base));
  assert.equal(sandboxResponse.headers.get("content-security-policy"), "frame-ancestors 'self'", "沙盒只限本站嵌入、不被宿主 CSP 罩住");
  assert.match(await sandboxResponse.text(), /default-src \* data: blob: 'unsafe-inline' 'unsafe-eval'/);
  const asset = (await (await fetch(base)).text()).match(/src="(\/assets\/[^"]+\.js)"/)[1];
  assert.equal((await fetch(new URL(asset, base))).headers.get("cache-control"), "public, max-age=31536000, immutable");
  const missing = await fetch(new URL("assets/missing.js", base));
  assert.equal(missing.status, 404, "缺檔的資產回 404，不退回首頁");
  assert.notEqual(missing.headers.get("cache-control"), "public, max-age=31536000, immutable");
  assert.match(await missing.text(), /找不到這一頁/);
  // 首頁別名都 308 回 /（query 保留），宿主 app 只從 / 出、一定帶宿主 CSP；空路段回 404
  for (const alias of ["index", "index/", "index.html"]) {
    const response = await fetch(new URL(`${alias}?state=x`, base), { redirect: "manual" });
    assert.equal(response.status, 308, `/${alias} 應轉回 /`);
    assert.equal(response.headers.get("location"), "/?state=x");
  }
  assert.equal((await fetch(`${base}/`)).status, 404, "// 應回 404");

  step("未登入：常駐下載連結打開下載頁——沒有正式版就說明並只給發佈頁，附安裝繞過說明與功能對照，關掉回原畫面");
  await page.goto(base);
  await page.getByText("連接 OpenRouter 就能開始玩").waitFor();
  await page.getByTestId("download-link").click();
  await page.getByTestId("download-page").waitFor();
  assert.ok(page.url().endsWith("#download"));
  await page.getByText("桌面版還沒有正式版").waitFor();
  assert.deepEqual(
    await page.getByTestId("download-page").getByRole("link").evaluateAll((links) => links.map((link) => link.getAttribute("href"))),
    [RELEASES_PAGE],
  );
  await page.getByText("仍要執行", { exact: false }).waitFor();
  await page.getByTestId("feature-compare").getByText("多角色同桌", { exact: false }).waitFor();
  await page.getByRole("button", { name: "回到遊戲" }).click();
  assert.equal(await page.getByTestId("download-page").count(), 0);
  assert.ok(!page.url().includes("#"));
  await page.getByText("連接 OpenRouter 就能開始玩").waitFor();

  step("PKCE 授權：回呼後網址清乾淨、金鑰只進 localStorage");
  await page.getByRole("button", { name: "用 OpenRouter 登入" }).click();
  await page.getByText("選一張角色卡開始").waitFor();
  assert.ok(!/code=|state=/.test(page.url()), `回呼參數沒清掉：${page.url()}`);
  assert.equal(fake.state.exchanges.length, 1, "授權碼應只交換一次");
  assert.equal(await page.evaluate(() => localStorage.getItem("tt-web:openrouter-key")), E2E_KEY);
  await page.getByText("今日免費 40/50").waitFor();

  step("開始畫面：找卡清單五站連結與一行 18 禁標示");
  assert.equal(await page.getByTestId("find-cards").getByRole("link").count(), 5);
  await page.getByTestId("find-cards").getByText("18 禁", { exact: false }).waitFor();

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

  step("自動存檔：玩家句的 setvar 進變數；回開始畫面看得到這桌，重新整理後繼續就接得上");
  fake.state.chat = "normal";
  await page.getByPlaceholder("輸入你的行動或對話…").fill("我付了錢{{setvar::錢包::15}}{{setglobalvar::名聲::1}}");
  await page.getByRole("button", { name: "送出" }).click();
  await page.getByTestId("message-user").getByText("我付了錢").waitFor();
  await page.getByRole("button", { name: "送出" }).waitFor();
  assert.equal(await page.getByTestId("autosave-note").textContent(), "對話自動存在這個瀏覽器");
  await page.getByRole("button", { name: "← 換一張卡" }).click();
  await page.getByTestId("save-item").getByText("瑟拉").waitFor();
  // Safari 的 ITP 提示放在存檔區（e2e 跑的是 WebKit）
  await page.getByText("Safari 會在七天沒打開本站後清掉網站資料").waitFor();
  await page.reload();
  await page.getByTestId("save-item").getByText("瑟拉").waitFor();
  await page.getByTestId("save-item").getByRole("button", { name: "繼續" }).click();
  await page.getByText("鞋上的雪先跺乾淨").waitFor();
  assert.equal(await page.getByTestId("message-user").count(), 3, "三句玩家句都接得上");
  await page.getByText("回應中斷").waitFor();

  step("回合中開關下載頁不中斷串流；在 #download 上重新整理，關掉下載頁後繼續這桌，回合中那句放回輸入框（D28）");
  fake.state.chat = "partial-hang";
  await page.getByPlaceholder("輸入你的行動或對話…").fill("下載頁草稿");
  await page.getByRole("button", { name: "送出" }).click();
  await page.getByTestId("message-streaming").getByText("說到一半").waitFor();
  await page.getByTestId("download-link").click();
  await page.getByTestId("download-page").waitFor();
  await page.getByRole("button", { name: "回到遊戲" }).click();
  await page.getByTestId("message-streaming").getByText("說到一半").waitFor();
  await page.getByRole("button", { name: "停止" }).waitFor();
  // 串流中換語系：介面換成英文、串流照舊，換回來也不中斷
  await page.getByTestId("lang-picker").selectOption("en");
  await page.getByRole("button", { name: "Stop" }).waitFor();
  await page.getByTestId("message-streaming").getByText("說到一半").waitFor();
  await page.getByTestId("lang-picker").selectOption("zh-TW");
  await page.getByRole("button", { name: "停止" }).waitFor();
  await page.getByTestId("download-link").click();
  await page.getByTestId("download-page").waitFor();
  await page.reload();
  await page.getByTestId("download-page").waitFor();
  await page.getByRole("button", { name: "回到遊戲" }).click();
  await page.getByTestId("save-item").getByRole("button", { name: "繼續" }).click();
  await page.getByText("鞋上的雪先跺乾淨").waitFor();
  assert.equal(await page.getByPlaceholder("輸入你的行動或對話…").inputValue(), "下載頁草稿");
  assert.equal(await page.getByTestId("message-user").count(), 3, "回合中那句退回輸入框，不落進逐字稿");
  await page.getByPlaceholder("輸入你的行動或對話…").fill("");
  fake.state.chat = "normal";

  step("匯出網頁存檔：過得了契約檢查、帶逐字稿與變數，附「用桌面版繼續」導流");
  const [saveDownload] = await Promise.all([page.waitForEvent("download"), page.getByRole("button", { name: "匯出存檔" }).click()]);
  assert.match(saveDownload.suggestedFilename(), /^瑟拉 \d{4}-\d{2}-\d{2} \d{4}\.json$/);
  const savePath = await saveDownload.path();
  const saveText = readFileSync(savePath, "utf8");
  // 這個 session 帶著測試金鑰、選過免費模型：存檔裡不能有金鑰與模型設定
  assert.ok(!saveText.includes("sk-or-"), "存檔帶到金鑰");
  assert.ok(!saveText.includes(E2E_KEY));
  const keyNames = [];
  const walk = (value) => {
    if (Array.isArray(value)) value.forEach(walk);
    else if (value && typeof value === "object") {
      for (const [key, child] of Object.entries(value)) {
        keyNames.push(key);
        walk(child);
      }
    }
  };
  walk(JSON.parse(saveText));
  assert.deepEqual(
    keyNames.filter((key) => /model|api_?key|token|openrouter/i.test(key)),
    [],
    "存檔帶到模型或金鑰類欄位",
  );
  const exported = parseWebSave(saveText);
  assert.ok(exported.ok, `匯出的存檔沒過契約檢查：${JSON.stringify(exported.error)}`);
  const save = exported.save;
  assert.equal(save.card.data.name, "瑟拉");
  assert.equal(save.import_route, "character");
  assert.equal(save.user_name, "玩家");
  assert.equal(save.opening_index, 0);
  assert.deepEqual(
    save.messages.map((message) => message.role),
    ["char", "user", "char", "user", "char", "user", "char"],
  );
  assert.equal(save.messages[0].opening, true);
  assert.equal(save.messages[4].interrupted, true);
  assert.equal(save.messages[5].text, "我付了錢");
  assert.deepEqual(save.mvu.layers.chat, { 錢包: "15" });
  // 跨對話 global：這桌寫過的鍵，重新整理後接著玩再匯出照樣帶著
  assert.deepEqual(save.mvu.layers.global, { 名聲: "1" });
  assert.deepEqual(save.card_storage, {});
  await page.getByTestId("export-funnel").getByText("用桌面版繼續").waitFor();
  assert.equal(await page.getByTestId("export-funnel").getByRole("link").getAttribute("href"), "#download");
  if (process.env.TT_WEB_EXPORT_OUT) copyFileSync(savePath, process.env.TT_WEB_EXPORT_OUT);

  step("匯出 ST 聊天檔：一行一則 JSON、檔頭照 ST，畫面說明只帶對話");
  const [stDownload] = await Promise.all([
    page.waitForEvent("download"),
    page.getByRole("button", { name: "匯出成 SillyTavern 聊天檔" }).click(),
  ]);
  assert.match(stDownload.suggestedFilename(), /^瑟拉 - \d{4}-\d{2}-\d{2}@\d{2}h\d{2}m\d{2}s\d{3}ms\.jsonl$/);
  const stLines = readFileSync(await stDownload.path(), "utf8").split("\n").map((line) => JSON.parse(line));
  assert.equal(stLines.length, 8);
  assert.deepEqual(Object.keys(stLines[0]), ["user_name", "character_name", "create_date", "chat_metadata"]);
  assert.equal(stLines[0].character_name, "瑟拉");
  assert.deepEqual(stLines[6], { ...stLines[6], name: "玩家", is_user: true, is_system: false, mes: "我付了錢", extra: {} });
  await page.getByTestId("export-st-hint").getByText("SillyTavern 聊天檔只帶對話").waitFor();
  // D3「對話與卡」：同一處給原卡檔（內建卡沒有 PNG，給原卡 JSON 外殼）
  const [cardDownload] = await Promise.all([
    page.waitForEvent("download"),
    page.getByTestId("export-st-hint").getByRole("button", { name: "下載這張角色卡" }).click(),
  ]);
  assert.equal(cardDownload.suggestedFilename(), "瑟拉.json");
  const cardJson = JSON.parse(readFileSync(await cardDownload.path(), "utf8"));
  assert.deepEqual([cardJson.spec, cardJson.data.name], ["chara_card_v3", "瑟拉"]);
  assert.deepEqual(cardJson, save.card, "給的就是存檔裡那份原卡外殼");

  step("存檔區：刪除要確認一次；匯入剛匯出的存檔回網頁版，繼續就接得上；版號不認得的存檔拒收");
  await page.getByRole("button", { name: "← 換一張卡" }).click();
  await page.getByTestId("save-item").getByRole("button", { name: "刪除" }).click();
  await page.getByTestId("save-item").getByRole("button", { name: "確定刪除" }).click();
  await page.getByText("還沒有存檔。").waitFor();
  await page.getByTestId("save-file").setInputFiles(`${SAVES}future-version.json`);
  await page.getByTestId("saves-error").getByText("這份存檔的版本（2）這個網頁版看不懂").waitFor();
  await page.getByTestId("save-file").setInputFiles(savePath);
  await page.getByTestId("save-item").getByText("瑟拉").waitFor();
  await page.getByTestId("save-item").getByRole("button", { name: "繼續" }).click();
  await page.getByTestId("message-user").getByText("我付了錢").waitFor();
  assert.equal(await page.getByTestId("message-user").count(), 3);

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

  step("ST 提示組裝：main→世界書前→description→personality→scenario→世界書後→範例→歷史（依深度插入）→post_history_instructions");
  fake.state.chat = "numbered";
  const sendFrom = fake.state.chatRequests.length;
  await page.getByPlaceholder("輸入你的行動或對話…").fill("*揮手* 晚安");
  await page.getByRole("button", { name: "送出" }).click();
  await page.getByText(`第 ${sendFrom + 1} 次回覆`).waitFor();
  const sent = fake.state.chatRequests.at(-1).messages;
  assert.equal(sent[0].content, "Write 灰燼旅店的莫拉's next reply in a fictional chat between 灰燼旅店的莫拉 and 旅人. 一律用繁體中文。");
  // 卡內世界書：「灰燼」命中（世界書前）、常駐條目（世界書後）、「莫拉」命中的依深度插入（深度 3）
  assert.deepEqual(sent[1], { role: "system", content: "灰燼旅店在王都南門外，地窖藏著走私酒。" });
  assert.deepEqual(sent[5], { role: "system", content: "這是一個魔法逐漸消失的世界。" });
  assert.deepEqual(sent[7], { role: "system", name: "example_user", content: "還有房間嗎？" });
  assert.deepEqual(sent.slice(-6), [
    { role: "system", content: "[Start a new Chat]" },
    { role: "system", content: "莫拉年輕時當過傭兵。" },
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

  step("從 PNG 匯入的卡：ST 匯出旁給的是原 PNG（一個位元組都不改）");
  await Promise.all([page.waitForEvent("download"), page.getByRole("button", { name: "匯出成 SillyTavern 聊天檔" }).click()]);
  const [pngDownload] = await Promise.all([
    page.waitForEvent("download"),
    page.getByTestId("export-st-hint").getByRole("button", { name: "下載這張角色卡" }).click(),
  ]);
  assert.equal(pngDownload.suggestedFilename(), "灰燼旅店的莫拉.png");
  assert.ok(readFileSync(await pngDownload.path()).equals(readFileSync(`${CARDS}composite.png`)), "原 PNG 被改了");

  step("卡內世界書：關鍵字觸發、sticky 跨回合、觸發狀態進網頁存檔");
  await page.getByRole("button", { name: "← 換一張卡" }).click();
  await page.getByTestId("card-file").setInputFiles(`${E2E_DIR}world-info-card.json`);
  await page.getByRole("heading", { name: "雪嶺嚮導" }).waitFor();
  await page.getByRole("button", { name: "開始這張卡" }).click();
  await page.getByTestId("message-char").getByText("要翻山就早點睡。").waitFor();
  fake.state.chat = "numbered";
  const wiTurn = async (line) => {
    const from = fake.state.chatRequests.length;
    await page.getByPlaceholder("輸入你的行動或對話…").fill(line);
    await page.getByRole("button", { name: "送出" }).click();
    await page.getByText(`第 ${from + 1} 次回覆`).waitFor();
    return fake.state.chatRequests.at(-1).messages.map((message) => message.content);
  };
  const firstWi = await wiTurn("聽說昨天雪崩了");
  assert.ok(firstWi.includes("雪崩過後三天內山路封閉。"), "關鍵字命中的條目要進提示");
  assert.ok(firstWi.includes("嚮導從不在夜裡出發。"), "常駐條目要進提示");
  assert.ok(!firstWi.includes("山腰有狼群。"), "沒命中的不進");
  const secondWi = await wiTurn("那我們等兩天");
  assert.ok(secondWi.includes("雪崩過後三天內山路封閉。"), "sticky：沒再提到也照樣帶著");
  const [wiDownload] = await Promise.all([page.waitForEvent("download"), page.getByRole("button", { name: "匯出存檔" }).click()]);
  const wiPath = await wiDownload.path();
  const wiSave = parseWebSave(readFileSync(wiPath, "utf8"));
  assert.ok(wiSave.ok, `世界書存檔沒過契約檢查：${JSON.stringify(wiSave.error)}`);
  assert.deepEqual(wiSave.save.world_info.entries, [
    { id: "wi-0", key: "0" },
    { id: "wi-1", key: "1" },
    { id: "wi-2", key: "2" },
  ]);
  // 第 2 則（玩家第一句）觸發：sticky 3、cooldown 2；第 4 則時冷卻到期、sticky 照樣觸發又記一筆冷卻
  assert.deepEqual(wiSave.save.world_info.timed, {
    sticky: { "wi-0": { start: 2, end: 5, protected: false } },
    cooldown: { "wi-0": { start: 4, end: 6, protected: false } },
  });
  assert.equal(wiSave.save.world_info.last_message_id, wiSave.save.messages[3].id);
  if (process.env.TT_WEB_EXPORT_WI_OUT) copyFileSync(wiPath, process.env.TT_WEB_EXPORT_WI_OUT);
  await page.getByTestId("export-funnel").getByRole("button", { name: "知道了" }).click();
  fake.state.chat = "normal";

  step("卡片介面：顯示 regex 產出的整頁畫成沙盒 iframe，讀得到訊息、存得了設定、按鈕送得出句子，偽造訊息不收");
  await page.getByRole("button", { name: "← 換一張卡" }).click();
  await page.getByTestId("card-file").setInputFiles(`${E2E_DIR}interface-card.json`);
  await page.getByRole("heading", { name: "星港管理員" }).waitFor();
  await page.getByTestId("import-regex-allow").check();
  await page.getByRole("button", { name: "開始這張卡" }).click();
  await page.getByTestId("card-frontend-notice").waitFor();
  const frontend = page.frameLocator('[data-testid="card-frontend"]').first();
  await frontend.locator("#info").getByText("樓 0／最後 0：歡迎來到星").waitFor();
  await frontend.locator("#visits").getByText("來過 1 次").waitFor();
  assert.equal(await page.getByTestId("card-frontend").first().getAttribute("sandbox"), "allow-scripts", "沙盒不給 allow-same-origin");

  step("MVU：開局照 initvar 初始化、介面讀寫第 0 樓的表；回覆照指令更新自己那樓、補狀態欄，下一輪提示讀得到");
  await frontend.locator("#ships").getByText("船隻 1").waitFor();
  await frontend.locator("#add").click();
  await frontend.locator("#saved").getByText("已寫入").waitFor();
  await frontend.locator("#ships").getByText("船隻 2").waitFor();
  // 宿主頁自己偽造的卡片訊息：來源不是掛著的 iframe，不收
  await page.evaluate(() => window.postMessage({ source: "table-tavern-card", kind: "input", text: "偽造的句子" }, "*"));
  const uiFrom = fake.state.chatRequests.length;
  fake.state.replies = ["貨船進港了，碼頭一下子熱鬧起來。<UpdateVariable>\n_.set('港口.船隻', 2, 5);//進港\n</UpdateVariable>", "夜裡的港口很安靜。"];
  await frontend.locator("#go").click();
  await page.getByTestId("message-user").getByText("從介面送出").waitFor();
  await page.getByText("貨船進港了，碼頭一下子熱鬧起來。").waitFor();
  assert.equal(fake.state.chatRequests.length, uiFrom + 1, "偽造的訊息不能觸發送出");
  assert.equal(await page.getByText("偽造的句子").count(), 0);
  const uiPrompt = fake.state.chatRequests.at(-1).messages.map((message) => message.content);
  assert.ok(uiPrompt.includes("<now>\n港口:\n  船隻: 2\n  主人: 旅人\n</now>"), `類巨集要換成卡片寫入後的表：${JSON.stringify(uiPrompt)}`);
  const status = page.frameLocator('[data-testid="card-frontend"]').nth(1);
  await status.locator("#status").getByText("本樓船隻 5").waitFor();
  // 回覆落地後開場那支介面不重掛（設定照舊），讀的還是自己那樓
  await frontend.locator("#visits").getByText("來過 1 次").waitFor();
  await frontend.locator("#ships").getByText("船隻 2").waitFor();
  // 換語系不重掛卡片介面：同一個 iframe 元素、同一個文件
  await page.getByTestId("card-frontend").first().evaluate((frame) => (frame.dataset.e2eKeep = "1"));
  await frontend.locator("body").evaluate((body) => (body.dataset.e2eKeep = "1"));
  await page.getByTestId("lang-picker").selectOption("en");
  await page.getByRole("button", { name: "Send" }).waitFor();
  await page.getByTestId("lang-picker").selectOption("zh-TW");
  await page.getByRole("button", { name: "送出" }).waitFor();
  assert.equal(await page.getByTestId("card-frontend").first().getAttribute("data-e2e-keep"), "1", "換語系不換 iframe");
  assert.equal(await frontend.locator("body").getAttribute("data-e2e-keep"), "1", "換語系不重載卡片文件");
  await page.getByPlaceholder("輸入你的行動或對話…").fill("晚安");
  await page.getByRole("button", { name: "送出" }).click();
  await page.getByText("夜裡的港口很安靜。").waitFor();
  const nightPrompt = fake.state.chatRequests.at(-1).messages.map((message) => message.content);
  assert.ok(nightPrompt.includes("<now>\n港口:\n  船隻: 5\n  主人: 旅人\n</now>"), "下一輪讀到回覆更新後的表");
  assert.ok(!nightPrompt.some((content) => content.includes("<StatusPlaceHolderImpl/>")), "占位不送模型");
  const [uiDownload] = await Promise.all([page.waitForEvent("download"), page.getByRole("button", { name: "匯出存檔" }).click()]);
  const uiPath = await uiDownload.path();
  const uiSave = parseWebSave(readFileSync(uiPath, "utf8"));
  assert.ok(uiSave.ok, `介面存檔沒過契約檢查：${JSON.stringify(uiSave.error)}`);
  assert.deepEqual(uiSave.save.card_storage, { visits: "1" }, "卡片設定要進存檔");
  const ships = (index) => uiSave.save.messages[index].message_vars?.stat_data?.港口?.船隻;
  assert.deepEqual([ships(0), ships(1), ships(2), ships(3), ships(4)], [2, undefined, 5, undefined, 5]);
  assert.equal(uiSave.save.mvu.seed.stat_data.港口.船隻, 1, "種子是開局那張");
  assert.deepEqual(uiSave.save.mvu.macros, { user: "旅人", char: "星港管理員" });
  assert.deepEqual(uiSave.save.mvu.layers.character, { 主題: "海藍" });
  assert.ok(uiSave.save.messages[2].text.endsWith("<StatusPlaceHolderImpl/>"));
  if (process.env.TT_WEB_EXPORT_MVU_OUT) copyFileSync(uiPath, process.env.TT_WEB_EXPORT_MVU_OUT);
  await page.getByTestId("export-funnel").getByRole("button", { name: "知道了" }).click();

  step("卡片介面把自己導走：新頁送來的句子不收、那支介面換一支新的重掛（新 token）");
  // Pages 會把 /sandbox.html 308 轉到 /sandbox
  const sandboxFrame = page.frames().find((frame) => /\/sandbox(\.html)?$/.test(frame.url()));
  assert.ok(sandboxFrame, "找得到開場那支介面的 frame");
  const navFrom = fake.state.chatRequests.length;
  const framesBefore = await page.getByTestId("card-frontend").count();
  // 導向同站的另一頁（沙盒旗標仍在、來源仍是不透明），那頁再假冒卡片送句子
  await sandboxFrame.evaluate(() => {
    location.href = "/sandbox.html?navigated";
  });
  // 新的 iframe 是延後載入的：捲回開場那則才會載
  await page.waitForTimeout(300);
  await page.getByTestId("card-frontend").first().scrollIntoViewIfNeeded();
  await frontend.locator("#visits").getByText("來過 2 次").waitFor();
  assert.equal(await page.getByTestId("card-frontend").count(), framesBefore, "換一支新的，不多也不少");
  const navigated = page.frames().find((frame) => frame.url().includes("?navigated"));
  if (navigated) {
    await navigated.evaluate(() => parent.postMessage({ source: "table-tavern-card", kind: "input", text: "導走後的句子" }, "*"));
  }
  await page.waitForTimeout(500);
  assert.equal(await page.getByText("導走後的句子").count(), 0, "導走後的頁面送的句子不能收");
  assert.equal(fake.state.chatRequests.length, navFrom);
  await page.getByTestId("card-frontend-notice").getByRole("button", { name: "知道了" }).click();

  step("今日免費用完：送出前查 /key，跳導流面板、原文留在輸入框");
  fake.state.remaining = 0;
  fake.state.chat = "normal";
  await page.getByPlaceholder("輸入你的行動或對話…").fill("第四句");
  await page.getByRole("button", { name: "送出" }).click();
  await page.getByTestId("quota-panel").waitFor();
  // 前往下載頁：蓋在導流面板上，關掉回到面板
  await page.getByRole("link", { name: "前往下載頁" }).click();
  await page.getByTestId("download-page").waitFor();
  await page.getByRole("button", { name: "回到遊戲" }).click();
  await page.getByTestId("quota-panel").waitFor();
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
  // 下載頁也給版本與兩個平台檔
  await page.getByTestId("download-link").click();
  await page.getByText("目前版本 v0.3.0").waitFor();
  const files = page.getByTestId("download-page").getByRole("link");
  assert.match(await files.nth(0).getAttribute("href"), /_x64-setup\.exe$/);
  assert.match(await files.nth(1).getAttribute("href"), /_aarch64\.dmg$/);
  await page.getByRole("button", { name: "回到遊戲" }).click();

  step("登出：清掉金鑰、回到登入畫面");
  await page.getByRole("button", { name: "登出" }).click();
  await page.getByText("連接 OpenRouter 就能開始玩").waitFor();
  assert.equal(await page.evaluate(() => localStorage.getItem("tt-web:openrouter-key")), null);

  step("十語系：英文瀏覽器首開就是英文（含 <html lang> 與標題）；登入、額度面板、開始畫面、範例卡、下載頁都是英文；桌上換成日文不丟桌、不丟草稿");
  // 獨立的瀏覽器情境：不共用主流程的 localStorage（含剛才記下的語系）與 IndexedDB
  const englishContext = await browser.newContext({ locale: "en-US" });
  const english = await englishContext.newPage();
  english.on("console", (message) => message.type() === "error" && consoleErrors.push(message.text()));
  await english.goto(base);
  await english.getByText("Connect OpenRouter to start playing").waitFor();
  assert.equal(await english.evaluate(() => document.documentElement.lang), "en");
  assert.equal(await english.title(), "Table Tavern | Web version");
  await english.getByTestId("download-link").click();
  await english.getByText("Current version v0.3.0").waitFor();
  await english.getByText("How the web and desktop versions differ").waitFor();
  await english.getByRole("button", { name: "Back to the game" }).click();
  await english.getByRole("button", { name: "Sign in with OpenRouter" }).click();
  await english.getByText("Pick a character card to start").waitFor();
  // 這時 /key 回報今日已用完（上一段的設定）：導流面板也是英文
  await english.getByText("Today's free uses are gone", { exact: true }).first().waitFor();
  await english.getByRole("button", { name: "Got it" }).click();
  await english.getByTestId("find-cards").getByText("Where to find character cards").waitFor();
  await english.getByRole("button", { name: "Start", exact: true }).click();
  await english.getByText("Stamp the snow off your boots first.").waitFor();
  await english.getByPlaceholder("Type your action or words…").fill("草稿留著");
  await english.getByTestId("lang-picker").selectOption("ja");
  await english.getByPlaceholder("行動やセリフを入力…").waitFor();
  assert.equal(await english.getByPlaceholder("行動やセリフを入力…").inputValue(), "草稿留著");
  // 桌還是那張英文範例卡（開了桌，卡不跟著換語系）
  await english.getByText("Stamp the snow off your boots first.").waitFor();
  assert.equal(await english.evaluate(() => document.documentElement.lang), "ja");
  assert.equal(await english.evaluate(() => localStorage.getItem("tt-web:lang")), "ja");
  await english.reload();
  await english.getByText("キャラクターカードを選んで始める").waitFor();
  // 重新整理後額度面板（今日已用完）又會跳出來：先關掉
  await english.getByTestId("quota-panel").locator("button.ghost").click();
  // 窄螢幕（手機寬）十個語系的開始畫面與下載頁都不橫向溢出
  await english.setViewportSize({ width: 375, height: 760 });
  const overflow = () => english.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
  for (const lang of ["zh-TW", "zh-CN", "en", "ja", "ko", "es", "pt-BR", "de", "fr", "ru"]) {
    await english.getByTestId("lang-picker").selectOption(lang);
    assert.ok((await overflow()) <= 0, `${lang} 開始畫面在 375px 寬橫向溢出`);
    await english.getByTestId("download-link").click();
    await english.getByTestId("download-page").waitFor();
    assert.ok((await overflow()) <= 0, `${lang} 下載頁在 375px 寬橫向溢出`);
    await english.getByTestId("download-page").locator("button.ghost").click();
  }
  await englishContext.close();

  const csp = consoleErrors.filter((text) => /Content Security Policy|Refused to/i.test(text));
  assert.deepEqual(csp, [], "不該有 CSP 擋下的資源");
  console.log("✓ e2e 全部通過");
} finally {
  await browser.close();
  await server.close();
  await fake.close();
}
