// test-harness 端對端驗收：全程只用 scripts/harness.mjs，不靠使用者、不打 AI。
// 用法（repo 根目錄）：node scripts/harness-e2e.mjs <測試 root> <TestCards/WestFantsy.png 路徑> <截圖目錄>
// 先 npm run harness:build。fixture 在主 checkout 的 TestCards/（worktree 沒有），sha256 不符即中止。
import { spawnSync } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs";

const [root, fixture, shots] = process.argv.slice(2);
if (!root || !fixture || !shots) {
  console.log("用法：node scripts/harness-e2e.mjs <測試 root> <WestFantsy.png 路徑> <截圖目錄>");
  process.exit(1);
}
const FIXTURE_SHA = "89fcec4c1588d487b590adf097390699b3135fbf551e6346d6d7b03071734421";
const log = [];

function h(...args) {
  const r = spawnSync("node", ["scripts/harness.mjs", ...args, "--root", root], { encoding: "utf8" });
  let json;
  try { json = JSON.parse(r.stdout); } catch { json = { ok: false, error: r.stdout + r.stderr }; }
  return json;
}
function step(name, ok, detail) {
  log.push({ step: name, ok, detail });
  console.log(`${ok ? "PASS" : "FAIL"} ${name}${detail === undefined ? "" : " — " + JSON.stringify(detail).slice(0, 300)}`);
  if (!ok) { console.log(JSON.stringify(h("quit"))); process.exit(1); }
}
const must = (name, res) => step(name, res.ok === true, res.ok ? res.value : res.error);
const mustFail = (name, res, pattern) =>
  step(name, res.ok === false && pattern.test(res.error ?? ""), (res.error ?? "").split("\n")[0]);
const js = (code) => h("js", code);

// 頁面端 H 的自測：在頁面上臨時放一組固定 DOM，跑完就移除。
const SELF_TEST = `
const box = document.createElement("div");
box.style.cssText = "position:fixed;left:0;top:0;z-index:2147483647;background:#fff;padding:4px";
box.innerHTML = '<button disabled><span>自測停用鈕</span></button>'
  + '<fieldset disabled><input aria-label="自測停用欄"></fieldset>'
  + '<select aria-label="自測選單"><option value="a">甲</option><optgroup label="g" disabled><option value="b">乙</option></optgroup></select>'
  + '<span id="tt-st-hidden" style="visibility:hidden">自測隱藏</span><span id="tt-st-shown">自測可見</span>';
document.body.appendChild(box);
const error = (f) => { try { f(); return "沒有錯誤"; } catch (e) { return e.message.split("：")[0]; } };
const saved = Element.prototype.checkVisibility;
try {
  const r = {
    textDisabled: error(() => H.click("text=自測停用鈕")),
    roleDisabled: error(() => H.click('role=button[name="自測停用鈕"]')),
    fieldsetDisabled: error(() => H.fill('role=textbox[name="自測停用欄"]', "x")),
    optgroupDisabled: error(() => H.select('role=combobox[name="自測選單"]', "b")),
    enabledOption: H.select('role=combobox[name="自測選單"]', "a").value,
  };
  delete Element.prototype.checkVisibility;
  r.fallbackHidden = H.describe(document.getElementById("tt-st-hidden")).visible;
  r.fallbackShown = H.describe(document.getElementById("tt-st-shown")).visible;
  return r;
} finally {
  if (saved) Element.prototype.checkVisibility = saved;
  box.remove();
}`;

const sha = crypto.createHash("sha256").update(fs.readFileSync(fixture)).digest("hex");
step("fixture sha256", sha === FIXTURE_SHA, sha);

must("launch --fresh", h("launch", "--fresh"));
const self = js(SELF_TEST);
const sv = self.value ?? {};
step(
  "H 自測：text 與 role 對同一 disabled 鈕結果一致、fieldset／optgroup 繼承、visible fallback",
  self.ok &&
    sv.textDisabled === "target 是 disabled" &&
    sv.roleDisabled === sv.textDisabled &&
    sv.fieldsetDisabled === "target 是 disabled" &&
    sv.optgroupDisabled === "選項是 disabled" &&
    sv.enabledOption === "a" &&
    sv.fallbackHidden === false &&
    sv.fallbackShown === true,
  self.ok ? sv : self.error,
);
must("開新的一桌", h("click", "text*=開新的一桌"));
must("進桌", h("wait", 'role=button[name="GM 推進"]'));
must("截圖：新桌", h("shot", `${shots}/e2e-1-table.png`));

must("塞檔匯入", h("file", 'input[type=file][accept*=".png"]', fixture));
must("等匯入選擇", h("wait", "text=匯入成世界書"));
must("匯入成世界書", h("click", "text=匯入成世界書"));
const done = h("dialog-wait");
step("原生訊息：38 條", done.ok && /38 條/.test(done.value?.[0]?.message ?? ""), done.value?.[0]?.message);
must("回答匯入訊息", h("answer", done.value[0].id, "ok"));
for (;;) {
  const more = h("dialog-wait", "--timeout", "3000");
  if (!more.ok) break;
  must(`回答後續訊息：${more.value[0].message.slice(0, 20)}`, h("answer", more.value[0].id, "ok"));
}

const worlds = js('return await H.invoke("list_worlds")');
const card = (worlds.value ?? []).find((w) => w.name !== "迷霧酒館（範例）");
step("找到匯入的桌", Boolean(card), card?.name);
const entries = js(`return (await H.invoke("read_worldbook", { worldId: ${JSON.stringify(card.id)} })).length`);
step("後端世界書 38 條", entries.value === 38, entries.value);

if (h("query", "text=先不要").value?.length) must("關開場白挑選", h("click", "text=先不要"));
if (!h("query", 'role=button[name="關閉介面"]').value?.length) {
  must("打開卡片介面", h("click", 'role=button[name="卡片介面"]'));
  must("等介面", h("wait", 'role=button[name="關閉介面"]'));
}
mustFail("被介面遮擋時點擊回錯", h("click", 'role=button[name*="世界設定"]'), /被遮擋/);
must("關閉介面", h("click", 'role=button[name="關閉介面"]'));
mustFail("disabled 點擊回錯（role）", h("click", 'role=button[name="換幕"]'), /disabled/);
mustFail("disabled 點擊回錯（text 選到內層文字）", h("click", "text=換幕"), /disabled/);

must("開世界設定", h("click", 'role=button[name*="世界設定"]'));
must("等世界書", h("wait", 'role=button[name="新增條目"]'));
const rows = js('return H.query("[aria-label$=\\" 的選項\\"]").length');
step("畫面世界書 38 條", rows.value === 38, rows.value);
must("截圖：世界書", h("shot", `${shots}/e2e-2-worldbook.png`));
mustFail("多重匹配回錯", h("click", 'role=button[name="編輯"]'), /匹配 38 個/);
must("開第一條條目", js('const el = H.find("role=button[name=\\"編輯\\"]")[0]; el.click(); return H.describe(el)'));
must("等條目編輯器", h("wait", 'role=button[name="儲存條目"]'));
must("截圖：條目編輯", h("shot", `${shots}/e2e-3-entry.png`));
must("取消條目編輯", h("click", 'role=button[name="取消"]'));
must("編輯器關閉", h("wait", 'role=button[name="儲存條目"]', "--gone"));
const pending = h("dialogs");
step("沒有掛起的原生對話窗", pending.ok && pending.value.length === 0, pending.value);

must("回大廳", h("click", 'role=button[name="回大廳"]'));
must("桌卡選單", h("click", `role=button[name="${card.name} 的選項"]`));
must("重新命名", h("click", "text=重新命名"));
must("填新桌名（受控欄位）", h("fill", 'role=textbox[name="桌名"]', "自動化改名測試桌"));
must("送出改名表單", h("submit", 'role=textbox[name="桌名"]'));
must("改名欄位關閉", h("wait", 'role=textbox[name="桌名"]', "--gone"));
const renamed = js(`return (await H.invoke("list_worlds")).find((w) => w.id === ${JSON.stringify(card.id)})?.name`);
step("後端桌名＝填入值", renamed.value === "自動化改名測試桌", renamed.value);
must("開設定", h("click", 'role=button[name="設定"]'));
// 視窗不可見時 WebKit 凍結過渡；測試包注入動態歸零，切頁後底線要立刻在新分頁（harness/motion.rs）
must("切到 AI 連線分頁", h("click", 'role=button[name="AI 連線"]'));
const underline = js(`return [...document.querySelectorAll(".settings-tab")].map((b) => {
  const s = getComputedStyle(b);
  return { current: b.getAttribute("aria-current") === "true", color: s.borderBottomColor, duration: s.transitionDuration };
})`);
step(
  "分頁底線只在 AI 連線、過渡時長歸零",
  underline.ok &&
    underline.value[1].current &&
    underline.value.every((t, i) => t.duration === "0s" && (t.color === "rgba(0, 0, 0, 0)") === (i !== 1)),
  underline.ok ? underline.value : underline.error,
);
must("回到外觀分頁", h("click", 'role=button[name="外觀"]'));
mustFail("select 不存在的選項回錯", h("select", 'role=combobox[name="語言 Language"]', "xx"), /選項不存在/);
must("切換語言到 English", h("select", 'role=combobox[name="語言 Language"]', "en"));
must("範例桌詢問出現", h("wait", "text=Create a new sample table in the new language?"));
must("截圖：範例桌詢問", h("shot", `${shots}/e2e-4-sample-prompt.png`));
must("取消（重選語言）", h("click", "text=Cancel, pick a language again"));
must("詢問關閉", h("wait", "text=Create a new sample table in the new language?", "--gone"));
const lang = js('return document.querySelector("select").value');
step("語言回到 zh-TW", lang.value === "zh-TW", lang.value);
const after = js('return (await H.invoke("list_worlds")).length');
step("沒有新增範例桌", after.value === 2, after.value);
must("Escape 關設定（press）", h("press", 'role=combobox[name="語言 Language"]', "Escape"));
must("設定視窗確實消失", h("wait", 'role=combobox[name="語言 Language"]', "--gone", "--timeout", "3000"));

const ai = h("ai-log");
if (!ai.ok || ai.value.writeFailures !== 0) {
  step("AI log 證據不足（讀取失敗或有寫入失敗），不能判定零派送", false, ai.ok ? ai.value : ai.error);
}
step("全程零 AI 派送", ai.value.entries.length === 0, ai.value);
const quit = h("quit");
step("quit", quit.ok && quit.exited, quit);
console.log(`\n全部 ${log.length} 步通過`);
