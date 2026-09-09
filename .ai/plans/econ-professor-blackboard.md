# 經濟學教授 HTML 黑板角色卡：施工計畫

對應任務：[econ-professor-blackboard](../tasks/econ-professor-blackboard.md)

## 1. 問題與方向
目標不是替 Table Tavern 新增一套黑板系統，而是做出一張**本身就攜帶教學介面能力**的 SillyTavern 角色卡。

現有 Table Tavern 已具備關鍵地基：
- `src-tauri/src/import/interface.rs` 會從匯入卡原始資料讀取 `data.extensions.regex_scripts`。
- 前端 card-interface controller 會把 regex replacement 產出的完整 HTML 交給 sandbox iframe 顯示。
- sandbox 已提供 `window.__ttHost`／`triggerSlash` 橋，卡內介面可送出文字且介面留在原處等回覆。

因此 v1 走：

```text
模型每回合：講解文字 + <blackboard>短板書</blackboard>
        ↓
角色卡自帶 regex_scripts
        ↓
固定黑板 + 可捲動講解 +（TT）底部輸入框
        ↓
SillyTavern 原生顯示 / Table Tavern 既有 sandbox 顯示
```

不改一般聊天 renderer、不新增 React 黑板元件、不擴完整 ST runtime。

## 2. 核心設計

### 2.1 模型只負責內容
模型不得輸出 `<style>`、複雜 `<div>`、JavaScript 或整份 UI；固定介面全部留在角色卡 display script。

為讓 Table Tavern 的卡片介面整場維持，**每一則教授回覆都必須包含且只包含一塊 `<blackboard>`**。即使沒有新板書，也保留／重寫目前仍有用的短板書。完整解釋放在黑板外，黑板只留骨架。

### 2.2 黑板內容契約

```text
教授的正常講解……

<blackboard>
標題
短公式／推導關鍵／比較結果
</blackboard>
```

規則：
- 黑板固定放回覆最後。
- 第一行＝板書主題，其餘保留換行。
- 允許 Unicode：`↑ ↓ → ← ≤ ≥ ≈ Δ π Σ ε`、上下標字元等。
- 黑板內不用 `<`、`>`；不等式改用 `≤`、`≥`。
- 每則最多一塊，closing tag 必須完整。
- v1 黑板不解析 LaTeX／Markdown／HTML。

### 2.3 固定教學介面
完整 HTML 殼採三段式：
1. 上：固定深綠木框黑板；板書過長時黑板自身可捲。
2. 中：教授最新一則講解；高度吃剩餘空間，過長只捲這一區。
3. 下：Table Tavern 專用輸入框。

視覺維持自足 CSS：木框、暖白粉筆字、系統字型、無圖片／CDN／遠端字型。

### 2.4 最小卡內 JavaScript
原規劃「零 JavaScript」在使用者實測後調整：若要一直留在 TT 卡片介面裡對話，需要一小段內嵌 JS 連既有 bridge。

限制：
- 只做 `window.__ttHost`／`window.triggerSlash` 特徵偵測、輸入框送出、Enter 行為。
- 偵測不到 TT bridge 時輸入框保持隱藏，因此 SillyTavern 繼續使用宿主原本輸入框。
- 不呼叫酒館助手、MVU、EJS 或任何外部 API。
- 不載遠端腳本。
- 不修改 TT sandbox 安全邊界。

## 3. 角色卡內容

### Persona
教授要能真的教經濟學：先抓假設與直覺，再決定是否補模型、公式、推導；若學生已熟悉基礎，就直接提高分析層次。黑板永遠只是摘要，不取代講解。

### First message
第一則訊息即附一塊極短預設黑板，使 TT 匯入後一開始就有可開啟的教學介面。

### Regex display script
只用一支 display script：
- find：捕捉黑板前文字、板書、黑板後文字三段。
- replace：完整自足 HTML，畫面順序重新排成「黑板在上、講解在中、輸入在下」。
- `placement: [2]`、`markdownOnly: true`、`promptOnly: false`。

## 4. 施工工作段

### A：卡片最小成品
已完成 `docs/examples/cards/econ-professor-blackboard.character.json`；標準 `chara_card_v2`，固定規約放常駐 `character_book` entry。

### B：教學介面收斂
- 固定黑板在頂部。
- 最新講解區獨立捲動。
- TT 偵測 bridge 後顯示底部輸入框，Enter 送出；Shift+Enter 換行。
- CSS selector 儘量 scope 在 `.econ-classroom`，避免影響宿主頁面。

### C：實機驗證
- Table Tavern：第一則開場就能開介面；輸入框能送出；回覆後介面仍在；黑板更新；長講解只捲中間區。
- SillyTavern：角色卡可正常匯入；Regex／HTML 顯示不破壞 ST；TT 專用輸入框不出現；仍用 ST 原生 composer。
- 若 ST 對完整 HTML／style 的實際處理與 TT 差異太大，優先調卡，不先改 app。

## 5. 邊界與停損

不做：
- LaTeX／KaTeX／MathJax 黑板 renderer。
- SVG 供需曲線、canvas、自由手繪。
- 完整聊天歷史搬進卡片介面；中間區只顯示教授最新一則回答。
- 通用教具框架。
- Table Tavern inline ST HTML renderer。
- 完整 SillyTavern runtime／外掛 API 相容。

若要達成目前介面仍必須修改一般訊息 renderer、sandbox 安全模型或 card-interface 大架構，停止本案並另案評估。

## 6. 成功定義

> 同一張低 token、自帶精緻固定黑板的經濟學教授角色卡，在 Table Tavern 可以留在卡片介面裡持續問答；在 SillyTavern 仍可作為標準角色卡使用。整案不需要修改 Table Tavern production code。
