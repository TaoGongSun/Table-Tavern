# 複合卡網頁存檔匯入桌面版續玩（預演）

公開前門檻第三條（[web-version](web-version.md)）的預演：origin/main（ed896144）上跑，正式驗收要等桌面 release。只驗不修〔作者裁決 2026-10-12〕；例外是①類那一項，主線交辦在本分支修掉後重跑。

## 步驟
1. 卡：`TestCards/DongeonMaster-safe.png`（世界書 25 條、MVU `[initvar]`＋`[mvu_update]`、三支介面 regex）。
2. 網頁段：`web/` e2e 模式建置＋假 OpenRouter（`web/e2e/fake-endpoints.mjs` 排好回覆），WebKit 匯入卡、勾允許 regex、玩家名「林恩」，玩 3 回合（每則回覆帶 `<UpdateVariable>` JSONPatch：改代價、命名、insert 房間／魔物／小隊、陣列 append），匯出網頁存檔。沒有真金鑰所以用腳本回覆。
3. 桌面段：測試通道（harness:build、獨立 root），新桌塞檔匯入網頁存檔，claude CLI `sonnet`（balanced）實聊 2 回合。
4. 對照：同一 root 另開一桌直接匯入原卡（世界書路）、貼開場白，只看初始變數樹。
5. 修掉①後網頁段、桌面段各重跑一次（同樣的腳本回覆與玩家句），下表是重跑結果。

## 結果
| 項目 | 結果 |
|---|---|
| 存檔匯入 | 成功，開成新桌，跳兩則說明（世界書規則提醒、卡帶介面） |
| 逐字稿 | 7 則全數接上；GM 則顯示文字剝掉 `<UpdateVariable>`，原文留在 `raw` |
| MVU 變數 | 7 則的 `message_vars` 與網頁存檔逐則相同；control 的 seed、巨集（林恩／卡名）帶過來；頂層是乾淨的 `基础信息`，網頁段的迷宫名称、时间、公會公告都改到了 |
| 世界書 | 25 條匯入；計時狀態空（卡內本來就沒有 sticky／cooldown／delay）。桌面兩回合的提示裡關鍵字條目照觸發（`职业`、`存储区衍生`、`迷宫房间类型`、`魔法与复活` 等），`[mvu_update]` 三條都在 |
| 卡片介面 | 迷宫之书畫得出來；續玩後「世界」頁顯示 `第2天 凌晨 03:45`、迷宫名称「幽绿深渊」、規模「一层迷宫」、公會公告；「迷宫」頁出現第 5 回合新開的房間、苔藓回廊 `[魔:1] [队:1]`——數值跟著續玩變動 |
| 續玩 patch | 第 4 回合 9/9 套用（含 `/基础信息/具体时间`）；第 5 回合 11 條套 10，拒 1 是 GM 自己寫錯鍵名 `/基础信息/迷宫整体威胁度`（正確是 `迷宫威胁度_整体`），屬正常拒收 |
| 花費 | 桌面重跑 2 回合 $0.29（輸入 21.9K→42.0K、輸出 2.3K／3.0K），修前那輪 $0.34；網頁段假端點 $0 |

## 判定
過（預演）：逐字稿、變數、世界書、介面都接得上，續玩 patch 照套，介面數值跟著續玩變動。唯一的①類已在本分支修掉。

## ① 擋住續玩（已修）
- **網頁版 initvar 把 `<Initial_Variables>` 包裝折進鍵名**：修前網頁版開局的樹頂層是 `<Initial_Variables> 基础信息` 與 `</Initial_Variables>: null`（YAML 把首行標籤併進第一個鍵），存檔帶進桌面後 GM 寫 `/基础信息/*` 全被拒（修前第 4 回合 6/13），介面「世界」頁永遠「未记录」；桌面版直接匯入同卡得到乾淨的 `基础信息`。
  - 修法：`web/src/features/mvu/initvar.ts` 的 `initvarBody` 剝完 `<initvar>` 與 code fence 後，首尾非空行若整行是成對的 `<標籤>`／`</標籤>` 就剝掉這一對（只剝最外層，不做通用 XML 剝除），得出與桌面版同樣的樹。這張是 zod 卡，ST 裡靠 schema 補齊結構，照非 zod 的 MagVarUpdate 解析會讓卡壞掉，所以不照上游〔模型判斷·未裁決，主線依「照做會變爛時不照」裁定〕。測試：`web/src/features/mvu/mvu-parts.test.ts` 一則。
  - 修前證據：`scratchpad/cc/web/web-save.json`、`scratchpad/cc/dt/.../mechanism-log.jsonl`（rejected「路徑不存在：/基础信息」）、對照桌 `scratchpad/cc/dt/data/worlds/01M4PPQ743B3FFT5P1CJNR60A4/card-vars/control.json`。

## ② 其他瑕疵（只列）
- 網頁版第 2 回合 `insert /迷宫房间/Room_002/房间内容/魔物/-` 被拒，陣列維持 `[]`（修前修後都一樣，沒查是否跟上游一致）；桌面續玩時 GM 改用 replace 整列補上。
- 網頁版聊天裡 GM 訊息照原文顯示整段 `<UpdateVariable>`（桌面版剝掉）。
- 匯入網頁存檔的提醒仍說桌面版「還沒有 sticky、cooldown 這類計時」，WI 補齊已完成，文案過時（`src/i18n/zh-TW.ts:390`，用處 `src/features/import/useImportController.ts:503`）。
- 桌面聊天的開場白尾端留著字面 `<开局>`（網頁版被 regex 換成介面）。
- 介面「魔物」頁把狀態「交戰中」算進「死亡消散」：GM 寫繁體、自創狀態字，卡的介面只認簡體列舉（同 [interface-takeover-card-types](interface-takeover-card-types.md) 已記的繁簡混排）。
- 網頁段 console 有 3 個 404 與 4 則宿主 CSP 擋下樣式表（沒查來源，介面截圖照畫）。
- 桌面每回合輸入 21.9K→42.0K，增長快（同上案已記）。

證據都在本機暫存 `/private/tmp/claude-501/-Users-pachelo-GitHub-Table-Tavern/1b77609f-1ee3-454f-b35a-437005d7c286/scratchpad/cc/`：網頁腳本 `composite-continue.mjs`；重跑：網頁 `web2/`（存檔、截圖、`web-requests.json`）、桌面 root `dt2/`（`harness-ai.log`、`data/worlds/01M4PQ25BACNN0V6823G6BX0N5/` 的 transcript、mechanism-log、card-vars）、截圖 `dt2-ui.png`、`dt2-chat.png`、介面 srcdoc `ui2/index.html` 與 `ui2-世界.png`、`ui2-迷宫.png`；修前：`web/`、`dt/`、`ui/`、`dt-*.png`、`ui-*.png`。
