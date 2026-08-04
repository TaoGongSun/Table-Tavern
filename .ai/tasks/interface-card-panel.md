# Task
Task-ID: interface-card-panel
Title: 介面卡渲染面板：ST 介面卡原樣顯示（殼匯入＋沙盒面板）
Status: todo
Created: 2026-08-04T00:00:00+08:00
Updated: 2026-08-04T00:00:00+08:00

## Summary
2026-08-04 拆「西幻魔法世界模拟器」卡後拍板立案。目標：ST 介面卡（卡內自帶 HTML 殼）匯入後，聊天旁開一個可收放的沙盒網頁面板原樣渲染，對話與狀態照舊由本 app 驅動。定位＝相容選配：原生狀態欄仍是省 token 正解，想要原汁原味、願付 token 的玩家才開面板。搶 ST 用戶的拼圖因此湊齊三塊：吃得下他們的卡、跑得起（原生省 token）、看得到原味（本面板）。

## 拆卡實據（樣本在 TestCards/，gitignored）
- 西幻卡（WestFantsy.png）：殼在 `extensions.regex_scripts`：『西幻』44KB（HTML 模板＋分頁腳本，顯示時套在模型輸出上）、『开场白』24KB（開場畫面）。模型每回合只吐帶標籤純文字，殼零 token；真實成本＝38 條世界書（含 6.5KB『回复规则』輸出格式規定）＋每回合重吐整份狀態文字。
- 同卡另有 SCrypt 加密版（社群 DRM，密文＋專用外掛登入驗證）：不解、不繞，偵測到就明講不支援渲染，其餘欄位照常匯入。
- 尋道太虛卡（RPGImmortal.png，另一作者）：殼 327K 字（顯示時從 CDN 載 Tailwind、jsdelivr、imgur.la 圖床——離線會破相）＋BGM 音樂腳本。三個關鍵事證：
  1. 按鈕代送靠單一函式：`triggerSlash('/send 文字|/trigger')`，且自帶「偵測不到酒館就退回 console」容錯 → 沙盒內塞一個 `triggerSlash` 墊片轉接我們的橋即可相容，免做整套 ST API。
  2. 模型輸出契約＝`<Status_block>`＋YAML（含小地圖座標、儲物袋、4 個行動選項），跟 state-values-mvu 已解析的同一家族 → 原生狀態欄與面板吃同一份輸出，兩路並存。
  3. 卡自帶『不发送前文状态栏』promptOnly regex（送模型前剝掉歷史裡的舊狀態塊省 token）→ 此類腳本直接忽略，本地權威天然等效。
- 訓帝卡（TrainEmperor.png，第三作者，視覺小說型）：**雲端載入器流派**——regex 把整則輸出換成一行 `$('body').load('作者的 GitHub Pages URL')`，整個 VN app 從雲端載回，再回頭抓酒館內部資料畫面（依賴酒馆助手 galgame 外掛，`ext.tavern_helper` 是另一個藏腳本的欄位）。沙盒天生不給碰宿主 → 此型 v1 不支援，偵測到載入器型（replaceString 極短＋整段吞掉原文＋remote load）就退純文字。
  - 當參考的價值在輸出協議：正文仍是 `<maintext>`＋固定 6 選項，舞台指令一行一句 `角色名|背景|CG|服飾|表情|台詞`，世界書列「合法資產清單」（13 立繪、30 背景、CG 圖集）逼模型只點名存在的圖——將來自做原生 VN 皮就照這配方（資產詞彙表＋管線指令），文字部分我們現有剝殼路線已吃得動。圖庫 DB 樣本 TestCards/TrainEmperor-pic.json（名字→網址對照：立繪服飾×表情＋定位、30 背景、CG 觸發句＋連播序列；101 圖全掛 postimg.cc 免費圖床＝雲端流派死穴，原生皮走本地圖包即無此弱點）。

三張卡三流派：內嵌殼（西幻）／內嵌殼＋triggerSlash 代送（尋道）／雲端載入器（訓帝）。前兩型 v1 蓋得住，第三型明講不支援。

## 範圍（v1）
- 匯入：收存角色卡 `extensions.regex_scripts`（findRegex／replaceString／placement／disabled）。
- 顯示：可收放面板＋sandbox iframe（`allow-scripts`、無 same-origin，碰不到 app 內部），殼的 regex 套在該回合模型原文的顯示層。
- 橋：面板內按鈕 → postMessage → 文字填入輸入框，單向、不代送；沙盒內提供 `triggerSlash` 墊片（認 `/send`、`/trigger`），一律映射成「填入輸入框」。代送要不要開（每桌開關）待拍板。
- 外部資源（圖床等）只在 iframe 內放行。
- best-effort：這張能動不代表每張能動；殼壞或 regex 不合＝正文照常顯示，絕不擋對話。

不做：ST 外掛 API 相容（酒馆助手＝開放讀寫訊息／變數／世界書／事件／觸發生成的完整 runtime，仿它＝無底洞）、DRM 解密、卡內腳本觸及 app 內部。

生態現況（2026-08-04 網查）：galgame 介面層無統一框架——酒馆助手只給 runtime，各作者自建前端自架雲端＋免費圖床。原生 VN 皮＋本地圖包＝補生態缺的標準件，且無圖床倒站死穴。

## 交界（state-values-mvu 包 4 順手帶最省）
✅ 包 4 已帶到（2026-08-04）：`TranscriptEvent` 加 `raw` 欄位存剝殼前的模型原文，`gm_narrate` 一併回傳、前端存進事件；只在真的剝到東西時才存，舊檔沒這欄照樣讀。角色台詞本來就沒剝殼，`text` 即原文。

## 驗收（v1）
- 匯入西幻卡開桌：面板出現同款介面（分頁、物品技能、推薦行動），開闔自如；點推薦行動→文字入輸入框。
- 關面板照常玩，原生狀態欄不受影響。
- 殼壞／regex 不合：正文照常顯示，無報錯中斷。
- SCrypt 卡匯入：提示不支援渲染，其餘照常。
- cargo test 蓋 regex_scripts 匯入收存與套用。

## Next action
- 排程待拍板：建議 state-values-mvu 包 5 之後開工（原文欄位地基包 4 已備好）。

## Constraints
- 卡內腳本一律沙盒，無任何 app API；橋只有「文字入輸入框」一條單向道。
- 不碰 DRM（不解密、不繞驗證）。
- 面板為選配，任何失敗回退純文字對話。
