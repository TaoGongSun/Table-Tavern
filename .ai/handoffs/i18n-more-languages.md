# Handoff: i18n-more-languages（介面擴充多語系，十國語言）

## Current state：十語系三處全部上齊、機械驗證全綠，等實機驗收

十國語言＝繁體中文、简体中文、English、日本語、한국어、Español、Português (Brasil)、Deutsch、Français、Русский。
每個語系的「介面字典＋首開範例桌內容＋AI 輸出語言規範」三處都齊（任務檔要求缺一不上）。
真 app 畫面的實機驗收尚未做——目前只驗到編譯、測試與機械量測。

## Completed（2026-07-30）

**架構改造（新增語系不必動程式邏輯）**
- 字典拆成每語言一檔：`src/i18n/<code>.ts`，入口 `src/i18n/index.ts` 從 MESSAGES 推導 `Lang` 型別，加語系只需 import ＋ MESSAGES ＋ LANGUAGE_OPTIONS 三行；缺鍵由 TypeScript 擋。
- 範例桌文本抽出程式碼：`src-tauri/samples/<code>.json`，`data.rs` 只留語系對應表與角色外觀設定（顏色／emoji／檔位）。
- `transport.rs` 的 `language_rule()` 擴成十語系 match，規範一律用該語言本身書寫；兩岸中文互加反向禁令。
- 切語言時同步 `document.documentElement.lang`（App.tsx），中日韓字形與斷行才正確；`index.html` 預設 lang 改 zh-TW。

**翻譯與品質關卡**
- 流程：Gemini 產 250 鍵初稿 → Opus 逐語系審校 → 機械量測按鈕寬度 → 超寬的退回原審校員縮短 → 同一審校員接著翻範例桌（術語才一致）。
- 審校抓到的實質錯誤舉例：韓文把「角色卡」誤作傳統跑團「角色紙」9 處；俄文整組「幕」誤作「場景」8 處、`{name}` 直接代入導致人名變格錯誤；西文 GM 與 director de juego 混用；葡巴「不會自動扣款」譯反成「不會自動加值」；法文把「AI 連線」誤作「API key」。
- 按鈕寬度判準：中日韓字算 2 格，上限為中英兩版較寬者的 1.3 倍＋2 格。全語系收斂到 0 超寬，另有 4 顆經逐顆判斷後保留（`de:editBtn`、`de:hideActs`、`ru:removeImageBtn`、`ru:worldbookSaveEntry`——該語言沒有更短的地道說法，硬換會犧牲已統一的術語）。
- 體檢腳本留在 repo：`npm run check:i18n`（佔位符一致性＋按鈕寬度，保留清單寫在腳本的 ACCEPTED_LONG）。

## Verification

- `npm run build` rc=0（tsc ＋ vite，十語系字典全數型別檢查通過）。
- `cargo test` 116 passed / 0 failed，含兩個新測試：
  - `transport.rs:806` `language_rule_follows_ui_language` — 八個新語系各自注入自己語言寫的規範、不殘留繁中規範（角色與 GM 兩條路徑都驗）。
  - `data.rs:2197` `sample_world_ready_in_every_language` — 九個非繁中語系都建得出範例桌：3 角色各有名字／公開設定／GM 秘密、世界設定非空、開場旁白恰一則、桌名確實翻過。
- `npm run check:i18n` rc=0：九語系佔位符全對、55 顆按鈕全在寬度上限內。
- 範例桌重構是逐字搬移：舊 `data.rs` 的 18 段長文本比對 JSON，只有時間戳沒進 JSON（本來就該留在程式裡）。
- 四處一致性交叉檢查（下拉選單／字典檔／範例桌 JSON／語言規範）：十語系全部到齊。

## Remaining / Next action

1. **實機驗收**（唯一擋結案的項目）：真 app 逐語系切過去看畫面，重點看德文與俄文的按鈕（最長）、日韓字形是否正確、首開語言選單十個選項的排版。
2. 使用者拍板後才動的四件（見下）。

## 待拍板（審校員提出，主線未擅自決定）

- 日文「世界書」20 個鍵維持中日同形的「世界書」，還是改用日本 AI 角色扮演圈慣用的「ワールド情報／ロアブック」。
- 日文範例桌角色名「狐」單字是否夠清楚（要不要加註讀音）。
- 範例桌地名處理不統一：德文創譯 Nebelmund、法文創譯 Bouche-de-Brume，日韓俄音譯，西葡保留 Mistmouth。要統一還是各語系照母語習慣各自處理。
- 三個角色在中英原文性別留白，法德西葡俄語法強制選性別，審校員一律用陽性（葡文狐狸用陰性，因 raposa 是陰性名詞）。若要保留性別留白需另想寫法。

## 已知限制（記錄不擋）

- 後端錯誤訊息仍是繁中（`ui-i18n-switch` 就記過的缺口，不在本任務範圍）。
- 範例桌內容各語系獨立翻譯，情節相同但文字風格會有差異（刻意，讀起來才像母語作品）。

## 派工紀錄

- 主線 Opus 5：架構決策與改造、字典裝機、按鈕寬度量測器（掃 App.tsx 找出真正在按鈕裡的 55 個鍵）、體檢腳本、全部驗證與逐字比對。
- codex:codex-rescue（gpt-5.6-terra）：範例桌抽資料檔重構一包（使用者其後指示本任務不再派 Codex）。
- agy（Gemini 3.5 Flash Low）：八語系 250 鍵初稿，16 次呼叫。
- general-purpose subagent（opus）×8：逐語系審校 → 按鈕縮短 → 範例桌翻譯，每語系同一個 agent 負責到底。

## Constraints（承前）

- 字典品質要有驗證關卡，不能 AI 產完直接上（本輪＝Opus 逐鍵審校＋機械量測；母語者複核仍未做）。
- 每加一語系，介面字典、範例桌內容、AI 輸出語言規範三處要同步，缺一不上該語系。
- 資料層識別字維持中文常數（`玩家` 哨兵、保留名檢查），語系只影響顯示與 system prompt 語言規範。
