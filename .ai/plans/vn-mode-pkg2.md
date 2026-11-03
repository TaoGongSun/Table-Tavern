# vn-mode 包 2 方案：VN 播放器

總綱 [vn-mode.md](vn-mode.md) §3／§6／§9／§11；消費包 1 的命令與串流契約（[vn-mode-pkg1.md](vn-mode-pkg1.md) §2）。大案：方案 Sol 審（第 1 輪必改 3 全採納）、驗收 Sol 審，測試通道實測。核心估 **600**（立案凍結；寫到 1200 停下檢討）。本檔除標〔作者裁決〕者皆〔模型判斷·未裁決〕。

## 0 查證現況（2026-11-01）

- 後端已有 `read_vn_cells(world_id, scene)`（逐則事件→正式格；玩家句＝一格旁白、系統事件略過）、`parse_vn_text(world_id, source, text)`（串流暫定格，`source` 為 `{kind:"gm"}` 或 `{kind:"character", self_id}`）、`vn_reroll_choices(world_id)`（錯誤碼 `VN_REROLL_BUSY`／`STALE`／`FAILED`／`UNAVAILABLE`）。`VnCell` 以 `type` 標籤序列化；前端還沒有 TS 型別。
- `useChatController` 已給播放器要的一切：`events`（落檔即加入）、`generating {id, kind}`（id 空字串＝GM）、`streamText`、`submitText`（VN 桌走推進迴圈）、`gmAdvance`、`canStop`／`stopResponse`、`busy`、`reload`。App 層的 `lockedChat` 已包好匯入等待鎖。**落檔順序**：事件先進 `events`，`streamText` 要到下一輪開始或整輪結束才清空；停止時半截有字就落成 `truncated` 事件。
- 角色卡圖 `characters.images[id]`（data URL）可當立繪佔位（包 7 換成定稿＋臉層）；`height_cm` 欄位不存在。
- 開場白面板與回合失敗框都在 `views/AppDialogs.tsx`（App 層），VN 視圖上照樣看得到；開場白面板無條件渲染，與精靈同時 modal（包 0 限制 a）。
- `App.tsx` 1138 行、`useChatController.ts` 922 行，皆熱點。

## 1 範圍

做：舞台（站位、底部對齊下緣裁切、說話者亮微前傾、其他壓暗、同屏 3、呼吸微動、closeup／shake）、對話框（名牌、thought 字色、打字機、點擊推進提示）、斷格推進、auto／skip／log、選項面板＋「換一組」、「繼續」、停止鈕、`[bg]` 地點字卡＋清場、串流暫定格與正式化對位、1 套主題；取代 `VnTableView`、解包 0 限制 (a)(b)(d)；換一組四個錯誤碼進 i18n（十語系）。

不做：背景圖（包 3，`[bg]` 只出字卡）、表情臉層與變體（包 7，表情照解析但不換圖）、逐格回溯／已讀標記／存檔槽／標題畫面（包 6）、章與結局語意（包 5）、`height_cm` 比例縮放（欄位不存在，包 7 再議）。

`Chapter`／`Ending` 格在包 2 一律當「停點」：播到就停在那格、顯示「繼續」鈕（走 `gmAdvance`），不另出字卡；包 5 接上語意。

## 2 資料流

**正式格**：`useVnCells(worldId, scene, events)` 在 `events` 陣列換了（落檔、收回、重讀、換幕，含同 id 內容改寫）就重抓整幕 `read_vn_cells`，攤平成 `FlatCell[]`，鍵＝`<事件 id>#<格序>`；另回傳 `refresh(): Promise<void>`，等新正式格裝好才 resolve。每次重抓帶世代號，晚到的舊結果丟掉。整幕重抓簡單且解析器在 Rust，一幕百則內不構成負擔。

**暫定格**：`generating` 非空且 `streamText` 非空時，節流（150 ms，或新字含換行時立即）呼叫 `parse_vn_text`；`source` 由 `generating` 導出（id 空字串＝GM，否則 `character` 帶該 id）。每段串流一個串流世代（`generating` 換人或 `streamText` 歸零就換代），暫定格鍵＝`pending:<世代>#<格序>`，晚到的解析結果世代不符就丟；暫定格接在正式格後面，最後一格 `tail_tentative` 時打字機停在已到的字，不顯示推進提示。

**換手規則**（依 §0 的落檔順序；Sol 必改 1）：串流結束（`events` 長出新的非玩家事件，或 `generating` 換人／歸零）時，凍結這段串流的暫定格快照與游標，繼續顯示；等含該新事件的正式格重抓回來，才在同一次更新裡把快照換成該事件的正式格並對位，畫面不出現空窗也不重複。下一段串流用新世代的鍵，不會佔到舊快照的格。停止而半截有落檔＝同一流程；重抓回來確定沒有新事件（停止或失敗沒落檔）才撤掉快照，游標退到最後一格正式格。

**選項只從正式格出**：`Choices` 暫定格不顯示（播到即視為不存在）；選項面板只在游標到達某則正式事件的 `Choices` 格、且 `generating` 為空時出現。這同時滿足總綱 §3「選項永遠排在該回合所有線之後」與「半截 `[choices]` 不可按」。

**正式化對位**（包 1 §2 的要求）：游標＝`{key, shown}`（`shown`＝這格已打出的字數）。暫定格換成正式格時，算「該事件已顯示文字長度」＝前面暫定格全文長＋本格 `shown`（只算 `Narration`／`Dialogue` 的 `text`，`Bg`／`Choices` 計 0），在該事件的正式格上找累積長度跨過這個值的那格與格內位置。純函式 `realignCursor(tentative, cursor, formal)`，表驅動測試（格數相同、正式版多一格 `[bg]`、正式版剝掉多餘選項、指令上限剝除、游標停在最後暫定格中間、正文讀完後接 `[bg]`／`Choices`）。同長度落在格界時停在前一格文字格的結尾（已讀完），不跳到其後的零字格。

## 3 播放

- **推進**：點對話框或按 Enter／Space（對話框本身是 `<button>`，鍵盤走原生）。打字中＝補完本格；已完＝下一格。下一格還沒有：生成中＝顯示「…」等待格（不轉圈，§6）；閒置＝停在結尾狀態。
- **打字機**：固定 40 字／秒；`prefers-reduced-motion` 與 skip 時直接全出。
- **結尾狀態**（依序判）：生成中＝等待格；游標所在事件的最後一格是 `Choices`＝選項面板；最後一格是 `Chapter`／`Ending`、或沒有選項＝「繼續」鈕（`gmAdvance`）；回合失敗＝App 層既有失敗框與錯誤通知，播放器顯示「繼續」。
- **auto**：本格打完後等 `1200 + 40×字數` ms（上限 6000）推進；遇選項、等待格、結尾停住但不關 auto。
- **skip**：包 2 沒有已讀標記，skip＝每 80 ms 推一格、文字全出，直到選項、等待格或結尾；任何點擊或再按 skip 即停。包 6 改成只跳已讀。
- **log**：疊加層，列出游標以前的正式格（名牌＋文字、thought 斜體、`[bg]` 一行地點字、玩家選的選項以「▶ 選項文字」列出）；不用聊天視圖的事件流渲染（VN 桌原文含標記，總綱 §9「既有事件流渲染」改讀成「同一份事件、以格渲染」）。Esc 或關閉鈕收起。
- **玩家事件**：不當成播放格（選了就送出，不重播一次），只出現在 log。
- **進桌位置**：沒有已讀標記前，進桌游標放在最後一則非玩家事件的第 0 格；更早的內容看 log。包 6 換成已讀位置。
- **選項有效性**（Sol 必改 3）：`Choices` 格只在它屬於本幕最後一則非系統正式事件時有效；之後已有玩家事件（選過了）或任何新事件＝已消耗，不再出面板，游標往後走。
- **共用動作鎖**：選項、換一組、繼續三者共用 `VnPlayer` 裡一把同步 ref 鎖，按下當下立即佔用，涵蓋送出（含 `submitText` 落玩家句、尚未設 `generating` 那段）、換一組呼叫與其後的正式格重抓，全部完成才放；鎖住期間三種鈕停用。
- **選項面板**：2–4 鈕，按下＝`submitText(選項文字)`；旁邊「換一組」＝`vn_reroll_choices`，成功後 `reload()` 並 await `refresh()`，新正式格裝好才解鎖，游標停在同一則事件的 `Choices` 格（Sol 必改 2）；失敗依錯誤碼顯示 i18n 訊息、原組照留。
- **停止**：`canStop` 時工具列出停止鈕（`stopResponse`）。
- **忙碌**：觸發回合的鈕另讀 `busy` 與 `lockedChat` 的鎖；auto／skip 計時器只推游標、不觸發回合。

## 4 舞台（純函式 `stageAt(cells, index)`）

從幕首摺疊到游標，所以重進桌、log、之後的回溯都得到同一個舞台：
- 有 `speaker_id` 的 `Dialogue` 格＝該角色上場（已在場不動）；`speaker_id: None` 的配角只出名牌、不擺立繪。
- 站位照進場順序：首位左、次位右、第三位中；第四位進場時，最久沒說話的人下場、新人補他的位子。
- 在場角色再次發言要更新「最近發言」次序（下場判定用）。
- `Bg` 格的地點與上一個 `Bg` 不同＝清場（只換時間不清場）；地點字卡顯示在舞台角落，換地點時淡入一次。
- 說話者＝目前這格的 `speaker_id`（旁白格沒有說話者，全員半亮）；說話者亮＋微前傾，其他人壓暗。
- `directive`：`closeup`＝說話者放大置中、其他人淡出，只改當格呈現、不動槽位與清場狀態；`shake`＝舞台震一次。
- 立繪＝`characters.images[id]`，沒有圖＝剪影＋名牌。CSS 底部對齊、下緣裁切、待機呼吸（1–2% 縮放循環，`prefers-reduced-motion` 關閉）。

## 5 包 0 限制與視圖切換

- (a) `AppDialogs` 在精靈開著時不渲染開場白面板，精靈關掉後才出；面板貼出的開場白照常落檔，播放器當第一則事件播。
- (b) 聊天視圖的頂列在 `vnTable` 時多一顆「VN 視圖」鈕切回；播放器工具列保留「聊天視圖」。兩鈕共用 App 的 `vnView` 狀態。
- (d) `VnTableView` 刪除，由 `VnPlayer` 取代並套主題。

## 6 落點與預算

新檔都在 `features/vn/`：`VnPlayer.tsx`（版面、工具列、接線）、`Stage.tsx`、`DialogueBox.tsx`、`ChoicePanel.tsx`、`Backlog.tsx`、`useVnCells.ts`（正式＋暫定＋換手）、`useVnPlayback.ts`（游標、打字機、auto／skip）、`vn-cells.ts`（`FlatCell`、攤平、`realignCursor`、`stageAt`、auto 延遲等純函式）、`vn-player.css`（主題 token 與版面）。`VnCell` 的 TS 型別放 `shared/contracts/backend-contracts.ts`。資料夾升到約 11 支 production 檔，未過 20，不分子資料夾。`useChatController` 不改；`App.tsx` 只換掉 `VnTableView` 那段 props 與傳 `characters`、`scene`、`lockedChat`。

核心分項（含 CSS；包 1 的盲點是提示詞字串與「同一規則接幾處」，包 2 對應的是 CSS 與忙碌鎖要接的鈕數）：

| 項 | 行 |
|---|---|
| `useVnCells`（重抓、節流暫定、換手） | 110 |
| `useVnPlayback`（游標、打字機、auto、skip、結尾狀態） | 130 |
| `vn-cells.ts` 純函式（攤平、對位、舞台摺疊、延遲） | 110 |
| 元件四支＋`VnPlayer`（工具列、鎖、錯誤碼對照） | 170 |
| `vn-player.css` | 120 |
| App／AppDialogs／聊天頂列接線、刪佔位 | 25 |
| 效能標記、錯誤碼與字串接線（非譯文） | 20 |
| 合計 | 約 685 |

重抓世代、換手快照與共用鎖已算在兩支 hook 與 `VnPlayer` 內。估計高於凍結預算 600 約 14%，在 2 倍線內；以 600 為準，寫到 1200 停。測試 ≤ 核心 2 倍：純函式表驅動（攤平、對位、舞台摺疊、auto 延遲）、元件測試（選項→`submitText`、換一組成功後同 id 新選項顯示且重抓完才解鎖、四個錯誤碼、選過的選項不再出面板、連按只送一次、繼續→`gmAdvance`、暫定格→正式格換手不重複、生成中出等待格、精靈開著時不出開場白面板）。

## 7 實測（測試通道，真模型低階檔）

總綱 §11 指定量三個時間，播放器在關鍵點打 `performance.mark`（`vn:turn-start`、`vn:first-cell`、`vn:relay-first-cell`、`vn:choices-ready`），通道用 `js` 讀 `performance.getEntriesByType("mark")`：
1. 單角色桌：按選項到第一格出現。
2. 角色接力：GM 最後一格打完到角色第一格可播的空檔（理想是 0：玩家讀 GM 格時角色已開始生）。
3. 選項可按：回合開始到選項面板出現。
另截圖驗：站位與說話者亮、選項面板、log、等待格、聊天視圖↔VN 視圖來回、精靈→開場白面板順序。

**實測結果（2026-11-01，Haiku 5.5 經 claude CLI，單角色卡）**：按選項到第一格約 6–9 秒、空桌按「繼續」到第一格約 15–18 秒；角色接力第一格在選項後約 15 秒與 31 秒出現，模擬每 3 秒讀一格的玩家從沒追上（等待格未出現，`gapMs` 0），延遲被緩衝吸收；選項面板在讀完 GM 那則時出現（讀者節奏，約 32 秒）。實測抓到並已修：空桌與選項後游標跳到暫定格後段、卡名括號註記對不上（GM 寫「雷恩」、卡名「雷恩（花豹）」）、角色線行首「旁白：」、播放器沒撐滿寬、對話框文字垂直置中。

**觀察**（範圍外，不立案）：單角色桌 GM 常自己寫名單角色的台詞並直接出選項，角色線那回合沒開口（包 1 導演指示已要求不替名單角色說話；Haiku 遵守度低）。

## 8 業界做法與差距

業界工具：Ren'Py（say screen、`preferences.text_cps`、`afm_time` 依字數的 auto-forward、skip、history screen、rollback）、TyranoBuilder、Naninovel（Unity）。慣例：對話框底部、名牌在框左上、點擊推進提示（CTC 指示）、打字機可調速、auto 依字數、skip 預設只跳已讀、history 疊加層、選單鈕列在對話框上緣。
我們的差距：打字速度與 auto 時間 v1 固定不可調（設定頁留待之後）；skip 包 2 不分已讀（包 6）；沒有 rollback（包 6 逐格回溯）；log 不能點回某格跳讀。

## 9 已知限制

- 進桌從最後一則非玩家事件開頭播，不記玩家讀到哪（包 6）。
- 立繪是角色卡原圖，不是透明七分身，可能帶背景（包 7）；表情不換圖。
- 背景是主題底色＋地點字卡（包 3）。
- 打字速度、auto 時間固定。
