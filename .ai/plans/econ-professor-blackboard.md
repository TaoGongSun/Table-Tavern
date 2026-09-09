# 經濟學教授 HTML 黑板角色卡：施工計畫

對應任務：[econ-professor-blackboard](../tasks/econ-professor-blackboard.md)

## 1. 問題與方向
目標不是替 Table Tavern 新增一套黑板系統，而是做出一張**本身就攜帶黑板顯示能力**的 SillyTavern 角色卡。

現有 Table Tavern 已具備關鍵地基：
- `src-tauri/src/import/interface.rs` 會從匯入卡原始資料讀取 `data.extensions.regex_scripts`。
- 只收「模型輸出後套用」的 display script。
- 前端既有 card-interface controller 會把 regex replacement 產出的 HTML 交給 sandbox iframe 顯示。

因此 v1 最省且最可攜的方案是：

```text
模型輸出短標籤
<blackboard>板書純文字</blackboard>
        ↓
角色卡自帶 regex_scripts
        ↓
固定 HTML/CSS 黑板
        ↓
SillyTavern 原生顯示 / Table Tavern 既有卡片介面顯示
```

這條路不需要把一般聊天訊息改成可執行任意 HTML，也不需要新增 React 黑板元件。

## 2. 核心設計

### 2.1 模型只負責內容
教授提示詞負責兩件事：
1. 決定何時值得板書。
2. 以固定 `<blackboard>` 包住短板書。

模型不得每回合輸出 `<style>`、複雜 `<div>`、JavaScript 或整份 UI；這些固定成本全部留在角色卡 display script。

### 2.2 黑板內容契約
第一版只要一種格式，不做小語言：

```text
<blackboard>
標題
板書第一行
板書第二行
板書第三行
</blackboard>
```

規則：
- 第一行＝標題。
- 後續＝正文，保留換行。
- 允許 Unicode：`↑ ↓ → ← ≤ ≥ ≈ Δ π Σ`、上下標字元等。
- 為避免 capture 內容被當 HTML，提示詞明定黑板內不用 `<`、`>`；不等式改用 `≤`、`≥`。
- 每則最多一塊黑板。

若實測發現「標題獨立 capture」反而讓 ST／Table Tavern regex 相容性變脆，退一步改成整塊單 capture；**穩定優先於多一層樣式**。

### 2.3 固定視覺
黑板殼只做 CSS 可完成的精緻感：
- 深綠或近黑板面。
- 低對比木框／內陰影。
- 暖白粉筆色文字。
- 標題較大、正文有足夠行距。
- `white-space: pre-wrap`、`overflow-wrap: anywhere`，避免窄視窗炸版。
- 小幅粉筆質感可用 CSS text-shadow／背景漸層；不載圖片。
- 不依賴外部字型，使用系統 serif／handwriting fallback。

第一版不追求逼真的粉筆噪點；「乾淨、像黑板、長文可讀」優先。

## 3. 角色卡內容

### 3.1 Persona
教授應是可真正教學的角色，而不是只會扮演教授語氣：
- 先確認學生目前卡住的概念，再解釋。
- 公式與直覺並列，不用板書取代口頭說明。
- 板書只放「值得留下來看的骨架」，例如定義、推導關鍵步驟、比較結果。
- 不因為有黑板就每回合使用。
- 若使用者要求推導，先逐步講，再用黑板收束。

本案不把教授限定成單一經濟學分支；角色提示詞保持通用，足以處理微觀、總體與基礎計量的文字／簡式公式教學。

### 3.2 First message
開場只需要讓使用者知道「這是一位教授，可以直接丟題目或概念給他」。第一則訊息可用一次小黑板示範，但不應把整個開場做成大型 UI。

### 3.3 Regex display script
優先使用一支 display script：
- find：只匹配 `<blackboard>...</blackboard>`。
- replace：固定黑板 HTML/CSS + capture 內容。
- placement：沿 ST 模型輸出顯示腳本現行格式。
- 不使用 promptOnly。
- 不使用 JavaScript。

如標題／正文需要兩個 capture 才能達到視覺層次，可做，但不要增加第二種標籤或多支互相依賴腳本。

## 4. 施工工作段

### 工作段 A：卡片最小成品
- 先依 repo 現有角色卡／測試資料慣例決定 source／fixture 落點；不要為單一卡片新增不必要的頂層目錄。
- 寫教授 persona、first message、blackboard 輸出規約。
- 寫一支最小 regex display script。
- 做固定 HTML/CSS 黑板殼。
- 用靜態字串驗證 regex 能穩定吃到板書。

完成條件：單一角色卡資料已能把 `<blackboard>` 轉成預期 HTML。

### 工作段 B：SillyTavern 實測與視覺收斂
- 匯入 SillyTavern。
- 測：一般回覆、單塊黑板、多行公式、長中文、窄視窗。
- 調整 CSS，只修視覺與 regex 穩定性，不擴功能。
- 確認不需任何外掛、CDN 或額外安裝。

完成條件：ST 端可當正常角色卡直接使用，黑板穩定且外觀完成。

### 工作段 C：Table Tavern 相容驗證
- 同一卡匯入 Table Tavern。
- 確認 `card_interfaces` 能讀到 display script。
- 確認既有 `findShell`／sandbox iframe 路徑能畫出黑板。
- 若只是小型格式差異，調整卡片 regex／replacement 解決。
- 若必須修改一般訊息 renderer、sandbox 邊界、card-interface 大架構：**停止本案**，記錄阻塞原因，另案評估。

完成條件：不擴核心的前提下，同一卡在兩邊都能用；否則清楚產出「ST 成功、TT 被既有相容層哪個限制擋住」的結論。

### 工作段 D：收尾
- 補最小必要測試／fixture。
- 跑本案涉及的既有驗證；若動到 app code，再跑 repo `verify` 規範要求的完整項目。
- 更新 task Progress、記實測結果與最終格式。
- 不在收尾時追加圖表、LaTeX、互動功能。

## 5. 驗收案例

### 一般文字
輸入：「機會成本跟沉沒成本差在哪？」

預期：教授可直接回答，不必黑板。

### 適合板書
輸入：「幫我整理需求增加對均衡的影響。」

預期模型輸出概念上包含：

```text
<blackboard>
需求增加
D₀ → D₁
供給不變
均衡價格 ↑
均衡數量 ↑
</blackboard>
```

顯示層只看到漂亮黑板，不看到原始標籤。

### 公式
輸入：「用簡單線性供需算一次均衡。」

預期板書可安全顯示：

```text
Qd = 100 − 2P
Qs = 20 + 2P
Qd = Qs
P* = 20
Q* = 60
```

### 邊界
- 模型沒輸出 `<blackboard>`：普通聊天不受影響。
- 少了 closing tag：不要吞整則訊息；原文保留比畫壞 UI 好。
- 板書很長：可換行／捲動，不突破容器。
- 離線：外觀完整。

## 6. 風險與停損

### 風險 A：ST 與 Table Tavern 對 regex replacement 的行為差異
先調角色卡格式；不先改 app。

### 風險 B：模型偶爾破壞標籤
靠簡單格式＋提示詞解決。第一版不建 parser 修復器。

### 風險 C：想順手加入供需曲線
不做。圖表另案；本案只證明「角色自帶精緻教具」這個最小模式成立。

### 風險 D：為了 Table Tavern 想做 inline HTML
明確超出本案。現有 Table Tavern 一般訊息 HTML 目前刻意經過安全白名單；本案不得拆掉這道邊界。

## 7. 成功定義
本案成功不是「做出通用黑板 framework」，而是：

> 一張不依賴外掛、低 token、自帶精緻 HTML 黑板的經濟學教授角色卡，可以在 SillyTavern 使用，並盡可能零核心修改地沿用 Table Tavern 現有 ST 介面相容層。

若做到這一點就收工；任何更通用的教具、圖表或 inline 介面都另外立案。
