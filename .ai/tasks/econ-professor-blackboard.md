# Task
Task-ID: econ-professor-blackboard
Title: 經濟學教授 HTML 黑板角色卡（SillyTavern／Table Tavern 共用）
Status: in_progress
Created: 2026-09-09T14:28:37+08:00
Updated: 2026-09-09T14:47:49+08:00
Branch: `feature/econ-professor-blackboard`
Base: `main@891d47f734ecca0bd7a75752f51adb0cfcc1254f`
Plan: [../plans/econ-professor-blackboard.md](../plans/econ-professor-blackboard.md)

## Summary
做一張小而精緻的「經濟學教授」SillyTavern 相容角色卡，讓教授需要板書時只輸出極短的 `<blackboard>...</blackboard>` 內容；角色卡自帶 `data.extensions.regex_scripts`，把該區塊轉成固定 HTML/CSS 黑板。黑板外觀與版型由卡片持有，不讓模型每回合重吐 HTML/CSS。

本案的重點是**做卡，不擴建 Table Tavern**。Table Tavern 已能讀 ST `extensions.regex_scripts` 並以既有卡片介面 sandbox iframe 渲染，因此第一版應直接吃現有相容層。若實測發現現有相容層無法承接這張小卡，先停下回報；不得為了本案順手重做訊息 renderer、inline HTML 或通用 ST runtime。

## Goal
- 同一張角色卡可匯入 SillyTavern 與 Table Tavern。
- 教授正常對話時維持一般文字；只有需要板書、公式整理或重點摘要時才叫出黑板。
- 模型輸出契約保持短、穩、低 token；固定 HTML/CSS 全放在卡片 regex replacement。
- 黑板視覺精緻但功能克制：深綠／近黑板面、木框感、粉筆字、清楚留白與換行。
- 第一版以「一個下午可收尾」為範圍上限，不做可延後的泛用能力。

## v1 範圍
### 角色行為
- 經濟學教授 persona／教學提示詞。
- 明確規定何時使用黑板、何時不要使用。
- 黑板內容只輸出純文字與 Unicode 經濟學符號；不要求模型產 HTML、CSS 或 JavaScript。
- 第一版每則回覆最多一塊黑板。

### 黑板輸出契約
建議最小格式：

```text
<blackboard>
需求增加
D₀ → D₁
供給不變
均衡價格 ↑
均衡數量 ↑
</blackboard>
```

第一行視為板書標題，其餘維持換行排版。實作時若 ST regex 的最小穩定寫法需要微調包裹格式，可以調整，但不得擴張成另一套標記語言。

### ST 卡片介面
- 使用角色卡 `data.extensions.regex_scripts`。
- display script 只處理 `<blackboard>...</blackboard>`。
- replacement 內含固定、自足的 HTML/CSS。
- 不依賴 CDN、遠端字型、圖床或外部 JavaScript。
- 不使用 ST 外掛 API、酒館助手 API、MVU 或 EJS。

### Table Tavern
- 沿用現有 `src-tauri/src/import/interface.rs` + 卡片介面 sandbox 路徑。
- 第一版不修改一般 `StoryText` Markdown renderer。
- 第一版不新增訊息內嵌 HTML 模式。
- 第一版不新增專用 `Blackboard.tsx`。

## 明確不做
- 自由手繪黑板／canvas。
- SVG 經濟圖、供需曲線繪圖引擎。
- LaTeX／MathJax／KaTeX 黑板 renderer（教授日後可在支援公式的宿主環境使用公式語法，但 v1 黑板本身只吃純文字／Unicode）。
- 互動按鈕、小遊戲、可編輯板書。
- 通用「任意角色都能叫教具」框架。
- Table Tavern 的 inline ST HTML renderer。
- 為相容單一卡片而擴充完整 SillyTavern runtime。

## 驗收
1. SillyTavern：匯入後角色可正常聊天，黑板標籤會被角色卡自己的 regex 轉成穩定黑板 HTML。
2. SillyTavern：一般回覆不出現黑板；需要整理公式／概念時才使用。
3. Table Tavern：同一卡匯入後，既有卡片介面能辨認並顯示該黑板，不需要改核心 renderer。
4. 黑板在窄視窗不橫向炸版；長文字可自然換行或在既有沙盒內安全捲動。
5. 不載任何遠端資源；離線仍完整顯示。
6. 模型每次只輸出板書內容，不重吐固定 HTML/CSS。
7. 若第 3 項需要新增 inline renderer、改 sandbox 安全邊界或大幅改 card-interface controller，視為超出 v1：停止施工，另案評估，不在本案硬做。

## 施工邊界
### 可以修改
- 本案角色卡／角色卡來源檔與必要的最小測試 fixture。
- 為證明既有 ST regex 相容性所需的小型測試資料。
- 若發現現有相容層有明確、局部、可測的 bug，先記錄並回報；只有不擴張架構時才可在本案修。

### 不要修改
- `src/shared/ui/story-markdown.ts` 的 HTML 安全政策。
- 一般對話訊息渲染架構。
- 卡片介面 sandbox 的安全模型。
- ST 外掛／JavaScript runtime 相容層。
- 與本案無關的角色卡匯入、重構模式、世界書或 UI 整理。

## Progress
- 2026-09-09：從最新 `main` 建立 `feature/econ-professor-blackboard`，base `891d47f734ecca0bd7a75752f51adb0cfcc1254f`。
- 2026-09-09：立案完成；v1 鎖定為「一張跨 ST／Table Tavern 的 HTML 黑板教授卡」，不擴建核心 renderer。
- 2026-09-09：確認 v1 不做 LaTeX renderer；黑板維持純文字／Unicode，避免把單一卡片擴成 KaTeX／MathJax 工程。
- 2026-09-09：完成第一版角色卡 source：`docs/examples/cards/econ-professor-blackboard.character.json`（commit `009f477`）。採標準 `chara_card_v2`，黑板規約放一條常駐 `character_book` entry，讓 ST 與 TT 都能收到；`extensions.regex_scripts` 只有一支 display script。
- 2026-09-09：為配合 TT 現有 `extractShell`（只抽完整 HTML 殼），有黑板的回覆會由 regex 把「黑板前文字＋板書＋黑板後文字」一起包成完整自足 HTML；沒有 `<blackboard>` 時 regex 不匹配，普通聊天完全不變。固定殼無 JS、CDN、外部字型或圖片。
- 2026-09-09：完成靜態檢查：JSON 可解析；黑板 regex 可捕捉前文／板書／後文三段；replacement 以 `<!DOCTYPE html>` 起始，符合 TT `extractShell` 現有辨識條件。尚未做 SillyTavern／Table Tavern 實機渲染驗收。
