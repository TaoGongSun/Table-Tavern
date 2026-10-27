# shell-update-flash：量測方案＋雙緩衝方案（已結案）

作者裁決〔作者裁決 2026-10-27〕：先校準量測、量出現況；量完不看門檻直接做雙緩衝；卡片介面接管段排第③。

## 現況（程式事實）

- 覆蓋層 `CardInterfaceOverlay.tsx` 單 iframe：`key=cardShellKey`、`srcDoc` 掛載時就在。key 涵蓋 worldId、varsGeneration、varsScene、activeCard、殼、本樓 id／名／文（`useCardInterfaceController.ts` 355 行起）。每回合 GM 回覆＝最新 GM 樓換了＝殼 HTML 本身換了＝key 換＝舊元素卸、新元素掛，中間沒有東西蓋著。
- 事故版（e02e0083 拆掉的那版）真正的錯：兩格 `key` 固定成格號，靠**改既有元素的 srcDoc 屬性**重載＋等宿主側 `onLoad` 翻面、沒有逾時；首開時殼塞進背面格、正面格是空的，load 不來就永遠空白。現行 `onLoad={push}` 在 WKWebView 上同樣可能不觸發，實際靠 `useEffect` 推送。
- 宿主訊息監聽：`mvu-write`／`mvu-eval` 核 token；`input`、`storage`、`close` 不核 token；`storage` 每次送整份快照。
- 網頁版（`web/`）是每則訊息一支 iframe、不翻面，不在本案範圍；墊片多送的 ready 訊息網頁版宿主不認、直接忽略。

## 一、量測方案

### 量什麼
玩家看得到的空白：舊殼畫面消失、新殼內容還沒畫出來的那段（覆蓋層底色或 iframe 預設白）。只有像素證據（M2）能直接回答；M1 給事件時間與 ready 延遲，用來定抽樣窗口與逾時，不單獨當「沒閃白」的證據。

### 第 0 步：先驗被蓋住的 srcdoc 會不會節流 rAF
在真 WKWebView（測試通道）掛三種配置各量 ≥20 次「殼 DOMContentLoaded→第二次 rAF」延遲：①單獨一支（現況）、②墊在不透明 iframe 下面（主案）、③疊在上面 `opacity:0`（備案）。②明顯比①慢（中位數多一個 frame 以上，或出現 rAF 不跑）就不准當預設，改用③；③也節流就回報主線另議。

### 儀器

**M1 時間線（事件與延遲）**
- 殼內（墊片，排在卡片 script 前、用讀訊息墊片收好的真 parent）送 `{source:"table-tavern-card", kind:"ready", token, reason, t:{dcl, raf, sent}}`；reason 見下方協議。`raf` 只有 reason=raf 才填，timer 兜底不填成繪製時刻。時間一律 `performance.timeOrigin + performance.now()`。另試記 `PerformancePaintTiming` 的 first-contentful-paint，拿得到就多一條、拿不到不影響。
- 宿主記到 `window.__ttShellTiming`（**只有該陣列事先存在才寫**，harness 用 `js` 建；正式使用零成本、前端不讀 harness 旗標）：新 key commit、pending 掛載、ready 收到（含 reason）、翻面、強制翻面、舊殼卸載、鎖點擊開始／結束。正對照的人為藏／恢復也由 harness 記進同一陣列。
- 時鐘校準：宿主送 ping（t0）、殼回 t1、宿主收 t2，偏移＝t1−(t0+t2)/2；|偏移|≤2 ms 才直接比較兩邊時間戳，否則用偏移修正並在報告註明。

**M2 像素取樣（主證據）**
- 時間條：harness 用 `js` 在主頁 iframe 區之外（工具列旁）掛一條 rAF 更新的色塊時間條；每張 `shot` 解碼出畫面時刻。只要求解碼值落在該次 shot 請求送出與回應收到之間（界限驗證），不假設它等於像素上屏時刻。
- 分類：每張的 iframe 區對照兩張參考圖（更新前穩定的 OLD、更新後穩定的 NEW）分成 OLD／NEW／BLANK（區內單一色佔 ≥95%）／TRANSITION／MIXED。TRANSITION＝可重做的過渡判定：該張逐像素都落在 OLD 與 NEW 同位置像素的線性混合上（單一混合係數 α∈(0,1)、殘差在門檻內），或落在 NEW 與覆蓋層底色的線性混合上（卡片自己的淡入）。只有 TRANSITION 可豁免收斂條件，BLANK 一律不靠人眼剔除。
- 每張樣本＝**重新觸發一次更新、隨機延遲 d 後只拍一張**；同一次更新不連拍，樣本才彼此獨立。
- 抽樣分兩階段：
  1. 粗掃：觸發後均勻 d∈[0,800] ms 抽 ≥40 張，加上 M1 的 commit／翻面時刻，估出可能出現空白的窗口。
  2. 加密：在窗口兩側（例：翻面前 50 ms 到後 150 ms，現況則是 commit 到 ready 後 150 ms）再抽 ≥60 張。漏抓機率＝(1−g/W)^n，前提：n 張各自獨立觸發、d 在窗寬 W 內均勻；空窗落在窗口內、每次更新都會出現、長度至少 g。W=200、g=16、n=60 約 0.7%。報告附各階段樣本數與這個漏抓機率。
- 結論用語：「快照中未見空白（n 張、漏抓 16 ms 空窗機率 x%）」，不寫「上屏零空窗」。

### 方法本身可不可信（先校準再量）
1. 正對照：harness 把 iframe 人為藏 100 ms（visibility hidden→visible），藏／恢復事件記進 `__ttShellTiming`。驗兩件事、分開判：M1 宿主時間線記到 ≈100 ms 的藏／恢復區間；M2 在該區間的 BLANK 比例接近 1、區間外接近 0。ready 延遲不受人為藏影響，所以不拿 ready 公式驗正對照。
2. 負對照：不觸發更新、同樣節奏拍 N 張，全部應為 OLD；有 BLANK 就是分類門檻或時間條有問題。
3. 時間條：連拍 20 張，解碼值單調遞增且都落在請求前後界限內；順便量出 shot 單張耗時。
4. 遮擋：確認 `occlusion.rs` 沒在 stderr 報缺 selector、主頁與 iframe `visibilityState` 都是 visible。

已知限制〔模型判斷·未裁決〕：兩次 rAF 只是「已排進繪製」的代理；卡片框架（Vue 掛載、v-cloak）可能在 rAF 之後才上色，這段只有 M2 看得到。卡片自己的淡入動畫在 M2 會被判 BLANK，報告分開列。

### 觸發方式
走真流程：`scripts/harness-fake-openrouter.mjs` 腳本化一串 GM 回覆（每則帶這張卡會畫成殼的狀態區塊，值每回合不同），面板開著連續送出，每回合＝一次殼更新。測試 root 的設定檔從零寫、只放假金鑰，不抄正式 config。不用 `append_transcript`＋回大廳重進（那是重掛，不是原地更新）。換幕另量一組。

測試桌：毛絨轉變桌＝`TestCards/Transfur-safe.png`（開工先確認它有殼可畫；沒有就換 WestFantsy 並註明）。全程零真模型。

## 二、雙緩衝方案

### ready 協議（殼→宿主）
- 墊片在 DOMContentLoaded 後：頁面 visible 時排兩次 rAF，第二次 rAF 送 `reason:"raf"`；visible 但 300 ms 內 rAF 一次都沒跑，送 `reason:"timer"`（`t.raf` 留空）；頁面 hidden 時不計時，等 `visibilitychange` 轉 visible 再排兩次 rAF。只送一次。
- 不等 window `load`（卡片的外站 script／字型離線時會把 load 拖很久）。
- 第 0 步或 M2 發現 rAF 之後仍有 BLANK（卡片框架晚上色）→ 改 ready 時機（例：再多等 N 個 rAF，或等 DOM 變動靜止），改完重量。

### 宿主比對
- 只認 `event.source` 是待翻面那支 iframe 的 contentWindow **且** token 等於它的 key，比對值存在 ref，不靠 render 閉包比字串（事故版的 stale 比對就是錯在這）。
- 早到的 ready：source 對不上任何已登記 iframe、但 token 等於目前 pending 的 key 時先記下（token→{event.source, 收到時刻}）；pending 的 iframe ref 登記時重新核三件事——存下的 source 就是剛登記那支的 contentWindow、token 等於它的 key、它仍是目前的 pending——三項都符合才補翻，不讓快殼白等逾時。

### 元素與狀態機（任何時刻最多兩支 iframe）
- 每支 iframe 都是**獨立掛載的新元素**，`key=自己的 shellKey`，掛載時 srcDoc 就在；**絕不改既有元素的 srcDoc**（事故根因）。
- 兩支是**同一個父層下的 keyed siblings**，翻面只改角色（class／`data-shell-role`），元素不換父層、不換 key，React 不會重掛。驗收時 harness 在翻面前把 pending 元素存到 `window`，翻面後比對 `===`。
- 狀態：`front`（正在顯示）＋可選的 `pending`（背面在載）。
  - 沒有 front（首開、重開面板）→ 新殼直接當 front，**不等 ready**，行為＝今天的單 iframe 直繪。
  - 硬切（新殼直接當 front、舊的立刻卸）：worldId、varsGeneration、activeCard 任一與 front 不同。
  - 同桌同卡的殼更新（每回合、換幕）→ 新殼掛成 pending，front 不動，進入鎖定期。
  - 鎖定期又來新 key → 卸掉舊 pending、掛新的，front 不動。
  - 收到 pending 的 ready → 同一次 commit：pending 升 front、舊 front 卸載、解鎖。
  - 逾時：**從鎖定期開始算**（不因換 pending 重算）1500 ms（`FLIP_TIMEOUT_MS`）還沒翻 → 強制翻到最新的 pending、解鎖，並記進 `__ttShellTiming`。強制翻面只保證不再等，不保證內容已畫好——那一刻的畫面等同今天的直繪剛掛上時。
  - shellDoc 變 null（關面板、殼沒了）→ 兩支全卸、解鎖。
- 逾時值：定案＝max(p99×2, 1500 ms)〔模型判斷·未裁決，Sol／Grok 同意〕＝1500 ms：主案配置、reason 全是 raf，commit→ready n=260、p50 136 ms、p99 390 ms、max 913 ms；缺 ready 0、逾時 0。
- 疊放：兩支都 `position:absolute; inset:0`，父層 `position:relative; flex:1`。主案：pending 墊在 front 下面，front 的 iframe 元素加不透明背景＝覆蓋層底色（卡片文件多半透明底；與現在透出的底色同色，畫面不變）。備案：pending 疊在上面 `opacity:0`。第 0 步決定哪個能當預設；兩種都量 M2，收斂條件＝翻面前後的快照都不得判 BLANK，兩種都出現 BLANK 就改 ready 時機。翻面不加過渡動畫。

### 鎖定期擋操作
- 擋板：鎖定期在兩支 iframe 之上蓋一層透明 `div`（同父層、z 序最高），點擊、捲動、拖曳都落在擋板上，不會穿透到任一支 iframe（主案下層新殼、備案上層 opacity:0 新殼都擋得到）。
- 焦點：進鎖定期時若 `document.activeElement` 是 front iframe，把焦點移到擋板（`tabIndex=-1`），iframe 內的 Enter／Space 不再送進舊殼；翻面後焦點留在宿主，不自動搶進新殼。
- 宿主訊息：
  - `input`：只收「目前 front、而且不在鎖定期」那支送來的（source＋token 都對）；鎖定期內誰送都拒收。
  - `storage`：沒有 pending 時只收 front 的；有 pending 時只收 pending 的（舊殼晚寫的整份快照不能蓋掉新殼）。新殼的掛載種子在 key 換時讀，自那一刻起舊殼的 storage 一律拒收，所以新殼之後 sync 回來的就是以它為準的最新值。
  - `mvu-write`／`mvu-eval`：本來就核 token、寫入佇列在 key 換時換成新殼的，舊殼的照舊拒收。
  - `close`（沙盒內 Esc）：任一支送來都收，關面板本來就要即時。
- 換幕：維持雙緩衝、照樣鎖定〔模型判斷·未裁決〕。理由：換幕是同一桌同一張卡的正常推進，雙緩衝一樣省掉閃白；換幕後舊殼顯示的是上一幕狀態，點了送出的行動對不上新幕，鎖最長 1500 ms 比送錯安全。硬切只留給「顯示對象換了」的情形（別桌、還原、別張卡）。

### 與既有機制的接法
- 推送：`push` 改成按 key 推給對的那支（ref 存 key→iframe）；pending 收到 ready 時再推一次最新快照（取代靠不住的 `onLoad`）。
- 驗收另加：焦點移到擋板後，按 Tab／Shift+Tab 都回不到兩支 iframe 內的控制項（鎖定期 iframe 設 `tabIndex=-1` 並 `inert`）。
- 可觀測性：iframe 帶 `data-shell-key`、`data-shell-role="front|pending"`，擋板帶 `data-shell-lock`，harness `js` 直接斷言「落定後只剩一支、是新 key、沒有擋板」。
- 檔案落點：新增 `src/features/card-interface/ShellFrames.tsx`（雙格元件、擋板、焦點）與一支純邏輯的狀態轉移檔（可單測，含逾時、早到 ready、連續換 pending、硬切）；覆蓋層只換掉 iframe 那段；ready 墊片放 `interface-card.ts` 的橋接墊片旁（該檔 538 行，加約 30 行）；controller 的訊息監聽加「依來源與角色收 input／storage」，需要 ShellFrames 提供目前 front／pending 的 contentWindow 與 key（經 ref 交給 controller）。資料夾現 17 檔，加 2 支（＋各自測試）。

### 怎麼保證不重演三次空白
1. 不改既有元素的 srcDoc、每份殼都是新掛元素；翻面不換父層、不重掛。
2. 首開與硬切不等任何訊號——這兩條就是今天的直繪。
3. 雙緩衝那條有總等待上限，到時強制翻面，不讓舊殼或擋板永遠停著。
4. ready 比對靠 ref＋event.source＋token，早到的另外記住補翻。
5. 驗收前先過三步實測（毛絨轉變桌開介面、開關多次、跨桌切換），每步用 `data-shell-role`／`data-shell-lock` 斷言＋`shot` 看畫面不空白；過了才替換。

## 業界做法與差距

| 做法 | 代表 | 我們 |
|---|---|---|
| 每則訊息一支 iframe、舊的不拆，零翻面 | SillyTavern／酒館助手、我們的網頁版 | 桌面覆蓋層是單一面板，做不到逐則排列 |
| 雙緩衝＋子框架主動握手＋逾時，不信任宿主側 load 事件 | Penpal、post-robot、Zoid、iframe-resizer | 本案要補的就是這個；不引函式庫——沙盒裡卡片會蓋掉 window.parent，握手得用墊片先收好的 parentRef |
| 同一支 iframe 內 `document.open/write` 換內容 | 網頁版 `sandbox.html`、部分線上編輯器預覽 | open 到首繪之間照樣空白，不解閃白 |
| 不重掛、只用 postMessage 送新資料 | VS Code webview（`retainContextWhenHidden`＋postMessage）、Sandpack HMR | 只適用自己寫的 iframe 程式。我們的殼是卡片作者的 HTML，每回合由最新 GM 樓的文字經卡片 regex 產出、HTML 本身就不同；讀訊息／MVU 快照已經是這種推送。重構骨架桌改成殼內填值是另案〔模型判斷·未裁決〕，作者裁決的雙緩衝仍是通用必要解 |
| 量測：逐 frame 錄影（ScreenCaptureKit／`screencapture -v`）、Web Inspector Rendering Frames、Paint Timing | 一般前端效能量測 | 錄影要螢幕錄製授權、harness 視窗常被蓋住；Web Inspector 要人操作。改用 M1 事件時間線＋M2 帶時間條的兩階段像素取樣，以正／負對照校準 |

## 量測結果（2026-10-27，測試通道真 WKWebView，毛絨轉變桌 Transfur-safe，本機假 GM 端點，零真模型）

- 第 0 步（被蓋住會不會節流 rAF）：單獨／墊在不透明 iframe 下／上層 opacity:0 各 20 次，DCL→第二次 rAF 中位數 50／50／51.5 ms、最大 60／62／63 ms，全是 raf、無逾時 → 沒有節流，主案（墊在下面）當預設。
- 校準：時鐘偏移 |t1−(t0+t2)/2| ≤3.5 ms（基線 ≤1 ms；最後一組超過 2 ms 門檻，但結論不跨 frame 對時——宿主事件都在宿主時鐘、殼內時間戳只算殼內相對值、「未見空白」靠像素，不需修正）；時間條 20 張單調、落在「請求前 42 ms～回應」之間（快照拍的是最後一個已合成的 frame）；iframe 內自帶時間條與主頁時間條差 −20～+1 ms（iframe 最多落後一個 frame）；負對照 40 張全 OLD；正對照（人為藏 100 ms）宿主時間線記到 100–103 ms，快照在 −4～64 ms 判 BLANK、區間外 0 張 BLANK。Paint Timing（FCP）在沙盒 srcdoc 拿不到。
- 現況（單 iframe 直繪）：commit 附近加密抽樣 80 次中 BLANK 21、MIXED（半畫好）10，分佈在舊殼卸載後 −22～82 ms，98 ms 起全是新殼——估計每回合約 80–100 ms 看得到覆蓋層底色（以快照解析度與樣本分佈推估）。粗掃 40 次（多落在 90 ms 後）1 張 MIXED。目前判斷空窗主因是新殼 srcdoc（約 264 KB）同步解析卡住主執行緒、畫面停在舊殼已拆的那一格；沒有 trace 佐證。
- 雙緩衝後：每回合（新殼掛上附近 80 次、翻面附近 80＋40 次、粗掃 40 次）與換幕 20 次，共 260 張快照中未見 BLANK／MIXED；強制翻面 0。估計漏抓機率（前提：每次更新都會出現、長度至少 16 ms、落在 200 ms 窗內）：掛上附近 52 張約 1.3%、翻面附近 42 張約 3%（另 40 張複驗 19 張在窗內）。
- 三步實測（`npm run harness:build` 測試通道）：毛絨轉變桌開介面＋一回合翻面（等待期兩支 inert、tabIndex −1、有擋板；翻面後同一元素升 front、擋板消失）、開關 5 次、西幻桌↔毛絨轉變桌切 3 輪，連跑 3 次各 20/20 通過，每步只有一支 front、截圖不空白。
- 網頁版宿主對不認得的 kind（ready、pong、paint）走 default 直接忽略。
- 量測工具（不入庫）：scratchpad `flash/`（fake-gm.mjs、trials.mjs、analyze.py、summary.py、gui3.mjs 等）。

## 已知邊界
- 多卡桌跟著最新 GM 樓自動換成另一張卡（不是玩家點分頁）：顯示對象含殼所屬的卡，會走硬切，那一下仍會閃。主線另案處理。
