# Task
Task-ID: econ-professor-blackboard
Title: 經濟學教授 HTML 黑板角色卡（SillyTavern／Table Tavern 共用）
Status: in_progress
Created: 2026-09-09T14:28:37+08:00
Updated: 2026-09-09T15:17:03+08:00
Branch: `feature/econ-professor-blackboard`
Base: `main@891d47f734ecca0bd7a75752f51adb0cfcc1254f`
Plan: [../plans/econ-professor-blackboard.md](../plans/econ-professor-blackboard.md)

## Summary
做一張小而精緻的「經濟學教授」SillyTavern 相容角色卡。模型每回合只負責正常講解與一塊短 `<blackboard>...</blackboard>`；角色卡自己的 `data.extensions.regex_scripts` 把它排成固定教學介面。

2026-09-09 使用者看過第一版實機畫面後拍板：黑板要**一直固定在最上方**，教授最新一則講解放中間獨立捲動，下方在 Table Tavern 顯示輸入框，讓使用者能一直待在卡片介面裡上課。這仍然是**做卡，不擴建 Table Tavern**；輸入框只使用 TT 現有 sandbox `__ttHost`／`triggerSlash` bridge。

## Goal
- 同一張標準 `chara_card_v2` 可匯入 SillyTavern 與 Table Tavern。
- Table Tavern：第一則開場就能畫出固定黑板教學介面；黑板置頂、講解區可捲、底部輸入框可直接送出並留在介面內等回覆。
- SillyTavern：仍是標準角色卡；Scoped Regex／HTML 可用，TT 專用輸入框不出現，繼續用 ST 原生輸入框。
- 每回合固定只帶一塊短板書，不讓模型重吐 HTML/CSS。
- v1 維持一個下午可收尾的範圍，不追加圖表／LaTeX renderer／通用教具框架。

## v1 範圍
### 角色行為
- 經濟學教授 persona／教學提示詞。
- 每則回覆都必須含且只含一塊完整 `<blackboard>`，黑板固定放回覆最後；即使沒新板書也保留／重寫目前仍有用的短摘要，避免介面消失。
- 黑板內只用純文字與 Unicode 經濟學符號；完整推理放黑板外。

### 卡片介面
- 一支 `extensions.regex_scripts` display script。
- 完整 HTML 畫面：上方固定木框深綠黑板／中間教授最新回答捲動區／下方 composer。
- composer 預設隱藏；只有偵測到 `window.__ttHost` 且有 `window.triggerSlash` 時顯示。
- 卡內只准最小 JS：TT 環境偵測、送出、Enter／Shift+Enter；不載遠端腳本、不碰外掛 API。
- CSS selector 以 `.econ-classroom` scope 為主，降低污染 SillyTavern 宿主樣式的風險。

### Table Tavern
- 沿用現有 `src-tauri/src/import/interface.rs`、`findShell`、sandbox iframe、`__ttHost`／`triggerSlash` bridge。
- 不修改任何 TT production code。
- 不新增 inline HTML renderer 或 `Blackboard.tsx`。

## 明確不做
- 自由手繪／canvas／SVG 經濟圖。
- LaTeX／KaTeX／MathJax 黑板 renderer。
- 卡片介面內完整聊天歷史；中間區只顯示教授最新一則回答。
- 通用角色教具框架。
- ST 酒館助手／MVU／EJS runtime 相容。
- 修改 Table Tavern sandbox 安全模型。

## 驗收
1. TT：匯入後第一則開場就能畫出介面。
2. TT：黑板固定頂端；長講解只捲中間區；底部輸入框固定。
3. TT：輸入框 Enter 可送出、Shift+Enter 換行；送出後介面留著，下一則回覆到達後更新畫面。
4. TT：每一則教授回覆都有黑板，因此不因某回合省略板書而掉回一般聊天介面。
5. ST：角色卡可正常匯入／聊天；Regex HTML 不破壞宿主；TT composer 不出現。
6. 離線完整顯示；無 CDN、遠端字型、圖床或外部 JS。
7. 若要通過上述驗收必須改 TT 核心 renderer／sandbox／card-interface 大架構，停止本案並另立案。

## 施工邊界
### 可以修改
- `docs/examples/cards/econ-professor-blackboard.character.json`
- 本 task／plan 與必要的最小測試 fixture。

### 不要修改
- `src/shared/ui/story-markdown.ts`
- 一般對話訊息 renderer。
- 卡片介面 sandbox 安全模型。
- ST 外掛 runtime 相容層。
- 與本案無關的世界書、重構模式或 UI。

## Progress
- 2026-09-09：從最新 `main` 建立 `feature/econ-professor-blackboard`，base `891d47f734ecca0bd7a75752f51adb0cfcc1254f`。
- 2026-09-09：立案完成；v1 鎖定為跨 ST／TT 的小型 HTML 黑板教授卡，不擴建核心 renderer。
- 2026-09-09：確認 v1 黑板不做 LaTeX renderer，維持純文字／Unicode。
- 2026-09-09：第一版角色卡 source 完成於 `docs/examples/cards/econ-professor-blackboard.character.json`（commit `009f477`），使用標準 `chara_card_v2`＋常駐 `character_book` 規約＋單一 display regex。
- 2026-09-09：使用者在 Table Tavern 實機看到第一版黑板，回報視覺效果良好，並要求改成固定教學桌面：黑板置頂、中間講解捲動、下方輸入框。
- 2026-09-09：角色卡升到 `0.2.0`（commit `0012245`）：開場即帶預設黑板；規約改為每回合固定一塊黑板；HTML 改三段式 grid；中間講解獨立 overflow；新增只在 TT `__ttHost`／`triggerSlash` 存在時啟用的最小 composer JS。未修改任何 TT production code。
- 2026-09-09：計畫文件同步更新（commit `e8c2db6`）。下一步是 TT 實機驗證 composer／捲動／回覆更新，再驗 SillyTavern。
