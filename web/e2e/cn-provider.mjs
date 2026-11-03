// 簡中情境（cn-web-provider）：首開預設魔搭貼令牌面板、頂欄沒有下載鈕也不打 GitHub；矽基流動貼錯金鑰當場擋；
// 回合中供應商／模型切換停用、按停止收尾後才能換，換了下一句打新的一家、帶那家自己的金鑰；手選模型失敗只報錯
// 不換別支；換家後有返回鈕；其他語系只有 OpenRouter；整段沒有任何請求帶付費模型。TT_E2E_SHOTS＝資料夾：手機尺寸截圖存到那裡。
import assert from "node:assert/strict";
import { join } from "node:path";
import { devices } from "playwright";
import { MS_KEY, SF_KEY } from "./fake-endpoints.mjs";

export async function runCnScenario({ browser, base, fake, step, consoleErrors }) {
  const shots = process.env.TT_E2E_SHOTS;
  const context = await browser.newContext({ ...devices["iPhone 13"], locale: "zh-CN" });
  const page = await context.newPage();
  // Playwright 截圖會往頁面塞一段 <style>，被宿主 CSP 擋下的那則不算 app 的錯
  let shooting = false;
  page.on("console", (message) => message.type() === "error" && !shooting && consoleErrors.push(message.text()));
  const shot = async (name) => {
    if (!shots) return;
    shooting = true;
    await page.screenshot({ path: join(shots, `${name}.png`) });
    await page.waitForTimeout(200);
    shooting = false;
  };
  const githubBefore = fake.state.githubHits;
  const requestsBefore = fake.state.cnRequests.length;
  const input = page.getByPlaceholder("输入你的行动或对话…");

  step("簡中首開：預設魔搭的貼令牌面板（四步、各開官方頁），沒有下載鈕、不打 GitHub");
  await page.goto(base);
  await page.getByTestId("key-panel").getByText("连接魔搭 ModelScope就能开始玩").waitFor();
  assert.equal(await page.getByTestId("key-panel").locator("ol li").count(), 4);
  await page.getByTestId("download-link").click();
  const cta = page.getByTestId("download-cta").locator("a");
  assert.deepEqual(await cta.allTextContents(), ["从 pCloud 下载（国内可用）"], "中國供應商模式下載頁只給 pCloud");
  assert.equal(await page.getByTestId("download-version").count(), 0);
  await page.getByRole("button", { name: "回到游戏" }).click();
  await shot("cn-key-panel");

  step("貼到一半的草稿不跟著換家（兩個方向）：換到矽基流動輸入框是空的、送不出");
  const draftBox = page.getByPlaceholder("粘贴令牌或密钥");
  await draftBox.fill(MS_KEY);
  await page.getByText("更多设置").click();
  await page.getByLabel("硅基流动 SiliconFlow").click();
  await page.getByTestId("key-panel").getByText("连接硅基流动 SiliconFlow就能开始玩").waitFor();
  assert.equal(await draftBox.inputValue(), "", "魔搭的草稿不帶到矽基流動");
  assert.ok(await page.getByRole("button", { name: "使用这把密钥" }).isDisabled());
  await draftBox.fill(SF_KEY);
  await page.getByLabel("魔搭 ModelScope").click();
  await page.getByTestId("key-panel").getByText("连接魔搭 ModelScope就能开始玩").waitFor();
  assert.equal(await draftBox.inputValue(), "", "矽基流動的草稿不帶到魔搭");
  await page.getByLabel("硅基流动 SiliconFlow").click();
  assert.equal(await draftBox.inputValue(), "");
  assert.ok(!fake.state.cnRequests.slice(requestsBefore).some((request) => request.auth), "沒按送出就沒有帶金鑰的請求");

  step("矽基流動：貼錯金鑰當場擋、不存；貼對的登入");
  await page.getByPlaceholder("粘贴令牌或密钥").fill("sk-wrong-key-000000");
  await page.getByRole("button", { name: "使用这把密钥" }).click();
  await page.getByText("这把密钥无效").waitFor();
  assert.equal(await page.evaluate(() => localStorage.getItem("tt-web:key:siliconflow")), null);
  await page.getByPlaceholder("粘贴令牌或密钥").fill(SF_KEY);
  await page.getByRole("button", { name: "使用这把密钥" }).click();
  await page.getByText("选一张角色卡开始").waitFor();

  step("頂欄換回魔搭：沒存令牌就回到面板，貼上後開桌聊天，打寫死名單第 1 支、不帶 X-Title");
  await page.getByTestId("provider-menu").click();
  await page.getByLabel("魔搭 ModelScope").click();
  await page.getByTestId("key-panel").waitFor();
  await page.getByPlaceholder("粘贴令牌或密钥").fill(MS_KEY);
  await page.getByRole("button", { name: "使用这把密钥" }).click();
  await page.getByRole("button", { name: "开始", exact: true }).click();
  await input.fill("你好");
  await page.getByRole("button", { name: "发送" }).click();
  await page.getByText("雪還在下，先喝口湯暖暖身子吧。").waitFor();
  await page.getByRole("button", { name: "发送" }).waitFor();
  const first = fake.state.cnRequests.filter((request) => request.route === "chat/completions").at(-1);
  assert.deepEqual([first.provider, first.model, first.auth, first.xTitle], ["ms", "deepseek-ai/DeepSeek-V4.1-Flash", `Bearer ${MS_KEY}`, null]);
  assert.equal(await page.getByTestId("download-link").count(), 1);
  await shot("cn-chat");

  step("回合中切換選項停用、回合收尾後才解鎖；換到已存金鑰的矽基流動，桌照舊、下一句打那家");
  fake.state.chat = "hang";
  await input.fill("第二句");
  await page.getByRole("button", { name: "发送" }).click();
  await page.getByText("正在回复", { exact: false }).waitFor();
  await page.getByTestId("provider-menu").click();
  assert.ok(await page.getByLabel("硅基流动 SiliconFlow").isDisabled(), "回合中不能換供應商");
  assert.ok(await page.getByTestId("model-picker").isDisabled(), "回合中不能換模型");
  const langOption = (lang) => page.getByTestId("lang-picker").locator(`option[value="${lang}"]`);
  assert.ok(await langOption("zh-TW").isDisabled(), "回合中不能換到會連帶換供應商的語系");
  await page.getByText("回复完成或按停止后再切换").waitFor();
  await shot("cn-switch-disabled");
  await page.getByRole("button", { name: "停止" }).click();
  await page.getByRole("button", { name: "发送" }).waitFor();
  fake.state.chat = "normal";
  assert.ok(!(await page.getByLabel("硅基流动 SiliconFlow").isDisabled()), "回合收尾後才解鎖");
  assert.ok(!(await langOption("zh-TW").isDisabled()));
  const beforeSwitch = fake.state.cnRequests.length;
  await page.getByLabel("硅基流动 SiliconFlow").click();
  await page.getByTestId("provider-menu").click();
  assert.equal(await input.inputValue(), "第二句", "沒吐字就停：原句回到輸入框（既有規則）");
  await page.getByRole("button", { name: "发送" }).click();
  await page.getByText("雪還在下，先喝口湯暖暖身子吧。").nth(1).waitFor();
  const afterSwitch = fake.state.cnRequests.slice(beforeSwitch).filter((request) => request.route === "chat/completions");
  assert.deepEqual(
    afterSwitch.map((request) => [request.provider, request.model, request.auth]),
    [["sf", "XingChenAGI/Xing4.0-29B", `Bearer ${SF_KEY}`]],
  );

  step("換回魔搭手選名單外的模型：失敗只報錯（403 說明不改用付費、可能魔粒不足），不換別支");
  await page.getByTestId("provider-menu").click();
  await page.getByLabel("魔搭 ModelScope").click();
  await page.getByTestId("model-picker").selectOption("other/big-model");
  assert.equal(await page.getByTestId("model-picker").locator("option").filter({ hasText: ":" }).count(), 0, "帶外部計費後綴的不列");
  await page.getByTestId("provider-menu").click();
  fake.state.cnChat = "403";
  const beforeManual = fake.state.cnRequests.length;
  for (const line of ["第三句", "第四句"]) {
    await input.fill(line);
    await page.getByRole("button", { name: "发送" }).click();
    await page.getByText("本站不会改用付费模型", { exact: false }).waitFor();
    await page.getByRole("button", { name: "发送" }).waitFor();
  }
  await page.getByText("也可能是今天的魔粒不足", { exact: false }).waitFor();
  fake.state.cnChat = "normal";
  const manual = fake.state.cnRequests.slice(beforeManual).filter((request) => request.route === "chat/completions");
  assert.deepEqual([...new Set(manual.map((request) => request.model))], ["other/big-model"]);

  assert.equal(fake.state.githubHits, githubBefore, "中國供應商模式不打 GitHub");

  step("桌上換到沒存金鑰的 OpenRouter＝離開桌：換回魔搭後落在選卡畫面，不從舊快照重掛桌");
  await page.getByTestId("provider-menu").click();
  await page.getByLabel("OpenRouter").click();
  await page.getByRole("button", { name: "用 OpenRouter 登录" }).waitFor();
  await page.getByTestId("left-table").getByText("接着玩", { exact: false }).waitFor();
  await page.getByTestId("connect-back").getByText("返回魔搭 ModelScope").click();
  await page.getByText("选一张角色卡开始").waitFor();
  assert.equal(await page.getByTestId("message-user").count(), 0, "沒有重掛舊桌");

  step("換語系分流：繁中只有 OpenRouter、沒有供應商設定；換回簡中回到魔搭");
  await page.getByTestId("lang-picker").selectOption("zh-TW");
  await page.getByRole("button", { name: "用 OpenRouter 登入" }).waitFor();
  assert.equal(await page.getByText("更多設定").count(), 0, "其他語系不列魔搭、矽基流動");
  assert.equal(await page.getByTestId("provider-menu").count(), 0);
  await page.getByTestId("lang-picker").selectOption("zh-CN");
  await page.getByText("选一张角色卡开始").waitFor();
  assert.equal(await page.getByTestId("provider-menu").textContent(), "魔搭 ModelScope");

  step("整段：沒有請求帶付費模型；每家只收到自己的金鑰");
  const all = fake.state.cnRequests.slice(requestsBefore);
  assert.ok(!all.some((request) => request.model && (/^(Pro|LoRA)\//.test(request.model) || request.model.includes(":"))), "不得送付費模型");
  assert.ok(!all.some((request) => request.provider === "ms" && request.auth === `Bearer ${SF_KEY}`), "矽基流動金鑰不得送去魔搭");
  assert.ok(!all.some((request) => request.provider === "sf" && request.auth === `Bearer ${MS_KEY}`), "魔搭令牌不得送去矽基流動");
  await context.close();
}
