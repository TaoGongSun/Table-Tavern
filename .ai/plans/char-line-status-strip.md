# char-line-status-strip — 角色台詞剝狀態區塊

網頁版公開前門檻之一〔作者裁決 2026-10-10〕。

## 根因
GM 回覆在後端統一剝狀態區塊；角色回覆完全沒經過這道，原文直接回前端、原樣落檔、原樣進下一輪歷史。

| 環節 | GM 線 | 角色線（現況） |
|---|---|---|
| 剝除本體 | `transport::extract_state_block`（`src-tauri/src/transport/response.rs:336`；標籤判定 `find_state_tag` :284：`<status*>` 收欄位、`<UpdateVariable>` 只剝；另剝 `<details><summary>狀態`、`<maintext>` 外殼、```state 圍欄與結尾裸圍欄） | 沒呼叫 |
| 串流顯示 | `PlayView.tsx:23-26` `narrationStreamText` 遇到 ``` `<details` `<status` `<UpdateVariable` 就截斷（:253） | `PlayView.tsx:240-241` 原樣顯示 |
| 回傳／落檔 | `commands/chat.rs:526-527` 剝殼＋點名行；:546-547 `text`＝剝後、`raw`＝原文（不同才存）；中止半截同剝（:346-347） | `commands/chat.rs:210-213`（CLI）、:268-271（api／codex）只剝本輪「名字：」前綴；`ChatReply`（:16）沒有 raw；前端 `useChatController.ts:591-614` 把它當 `text` 落檔 |
| 送回模型的歷史 | 組裝讀 `event.text`（`transport/assemble.rs:117-128`），所以是剝後版 | 同處讀 `event.text`＝含標籤原文，GM 與其他角色都會看到並模仿 |
| CLI 續聊對帳 | `lanes/mod.rs:967-971` 預期落檔文字＝剝殼後 | `lanes/mod.rs:961-965` 只剝前綴 |
| 畫面渲染 | — | `story-markdown.ts:57-58` 原始 HTML 一律跳脫，所以標籤以字面出現 |

卡片介面殼讀 `event.raw ?? 顯示文字`（`card-interface/card-shell-route.ts:19-21`），GM 事件已經給卡原文；角色事件現在沒 raw，給的是同一份含標籤文字，結果碰巧一樣。

## 網頁版
沒有同樣的問題：網頁版只有一張卡、一條回覆路，照 ST 處理每一則 AI 回覆——存原文、顯示靠卡的 markdownOnly regex、送模靠 promptOnly regex（`web/src/features/chat/st-text.ts:1-8`、`sillytavern/regex-scripts.ts:1-6`），MVU 照 MagVarUpdate 處理最後一則 char 回覆（`mvu/runtime.ts:72-93`，補 `<StatusPlaceHolderImpl/>`、拿掉 `<status_current_variable>`：`mvu/tables.ts:51-60`）。網頁版自己不認 `<UpdateVariable>`，卡沒帶隱藏 regex 就照 ST 原樣顯示。

但網頁存檔匯進桌面版時會把這個問題帶過來：`import/web_save/mod.rs:298-324` 把網頁訊息的 `text`（ST 的 mes，含 `<UpdateVariable>` 與 `<StatusPlaceHolderImpl/>`）原樣當桌面事件 `text`，角色路變成 Dialogue、GM 路與開場變成 Narration，兩種都沒剝（fixture `src/shared/contracts/web-save/web-export-mvu.json` messages[2].text 就是這形）。

## ST（06bde939）
ST 本體不認 `<UpdateVariable>`／狀態標籤，回覆只套 AI_OUTPUT 一般 regex 後原樣存；顯示與送模的隱藏全靠卡或全域 regex（markdownOnly／promptOnly）。MVU 擴充（MagVarUpdate）對最後一則 AI 訊息解析 `<UpdateVariable>` 並寫變數——群組聊天裡哪個角色回的都一樣處理。也就是說 ST 沒有「GM 線剝、角色線不剝」的分別，每則回覆走同一條路。

## 做法：角色線共用 GM 的剝除
桌面版 GM 線本來就用 app 自己的剝除取代 ST 的「靠卡 regex」（比 ST 好、簡單），角色線照「每則回覆同一條路」的 ST 精神共用同一處理，不另寫一套。

標示：四項作者拍板暫取 A〔模型判斷·未裁決〕（見「待作者拍板」，等作者確認）。下面標「主線裁決」的是審查中由主線定的，〔模型判斷·未裁決〕。

### 1. 共用函式（後端）
分兩層：
- **純函式（transport）**：`transport` 新增 `finish_character_reply(reply, prefix, aborted) -> CharacterReply { text, raw: Option<String>, aborted }`，小結構定義在 transport，不回 `commands::ChatReply`（transport 被 commands 依賴，反過來會循環）。`expected_reply_for` 的 Dialogue 分支（`lanes/mod.rs:959-965`，:1332 傳入含前綴原文）只呼叫這層取 `text`，保證與回前端的 `text` 字面相等。
- **接線層（commands/chat.rs）**：包一層把 `CharacterReply` 轉 `ChatReply`（`chat.rs:16` 加 `raw: Option<String>`）並做空正文判斷，CLI 路（`chat.rs:210`）與 api／codex 路（:268）各一行呼叫。

純函式步驟：
`OwnPrefixStream` 不動（串流畫面剝前綴，`chat.rs:232`、`lanes/mod.rs:1175` 用它）。`strip_own_prefix` 不再有正式呼叫端，改成 `#[cfg(test)]`，只當串流版逐字相等不變式的測試基準（`own_prefix.rs` 既有測試照舊）〔模型判斷·未裁決〕。下面要的「吃前導空白」「剝完是空也剝」做成 `finish_character_reply` 內部的私有變體（主線裁決〔模型判斷·未裁決〕）。

1. 剝本輪前綴（私有變體：先略過開頭空白再比對前綴；剝完是空也剝）。記下有沒有剝到。
2. 先移除自閉合控制標籤（`<status…/>`、`<StatusPlaceHolderImpl/>`，不分大小寫），再 `extract_state_block` 取 `display`。理由：`find_state_tag`（`response.rs:306` 一帶）不認自閉合，`<status/>正文<status>狀態</status>尾文` 會把第一個標籤配到後面的閉標籤、中間正文一起刪。raw 不受這步影響、照舊保留原樣。角色、GM 中止、匯入三處都先做這一步。
3. `trim_start`。
4. 只在第 1 步沒剝到時：extract 之後才露出的開頭前綴再剝一次（處理「標籤在名字前面」），剝完是空也剝。第 1 步已剝過就不再剝，`狐狸：狐狸：正文` 只剝一層。
5. 切未閉合尾巴（見下「最終文字規則」），最後 `trim_end`。

兩種形狀定義：
- `標籤＋換行＋名字：正文` → `text`＝`正文`。
- `標籤＋名字：`（名字後面沒東西）→ `text` 為空，完成回合判成沒正文。

`raw`：取「剝完前綴、還沒剝狀態區塊」的文字——第 1 步後的文字；若第 4 步剝到前綴，逐一試拿掉第 1 步文字裡前綴的每個出現處，重跑第 2–3 步結果等於「剝掉前綴後的顯示文字」的那一處就是正文開頭那一處，raw 只拿掉它（自閉合標籤與控制區塊照樣留在 raw；`<UpdateVariable>` 裡面也可能含「名字：」，所以不取第一個符合的位置）；都對不上（例如前綴是剝殼後才拼出來的）raw 保留原樣。與 `text` 字面相同才是 None，只差頭尾空白也保留 raw。不能帶前綴，否則卡片 `floorText`（`card-shell-route.ts:19-21`）拿到的樓文字會多一個「名字：」。

**未閉合尾巴**（主線裁決〔模型判斷·未裁決〕）：不看 truncated 旗標，後端拿不到（`transport/dispatch.rs:285`、:403、:440 只回 text）；改以「中止／完成」分。標記比對一律不分大小寫，與 `find_state_tag` 一致。Rust 實作最終文字規則，TS 只實作串流規則（不另寫收尾模式）；兩邊對「完整標記開頭」「標記前半片段」「拆殼（`<maintext>`）」的定義共用一份案例 JSON 鎖住，JSON 分欄寫 Rust 最終結果與 TS 串流結果，不要求兩邊全文相等。自閉合標籤移除是一個共用函式，角色回覆、GM 中止、匯入三處都呼叫它。

最終文字規則（後端 `text`、匯入）：
- 中止與完成都切，從該標記開頭切到結尾：完整標記開頭但沒閉合的 `<UpdateVariable`、`<status…`、```state 圍欄；任何沒閉合的 `<details`（照層級配對，從最外層沒閉合的那個切；半截時分不出是不是狀態，與畫面正則一致；已閉合的非狀態 details 照舊交給 extract 判斷）。
- 只在中止時切：結尾停在標記前半的片段（`<U`…`<UpdateVariabl`、`<s`…`<statu`、`<d`…`<detail`、`<m`…`<maintext`、`</`…`</maintext` 這類至少兩字的前半），以及任何沒閉合的 ```。
- 都不切：結尾只有單獨一個 `<`，或單、雙反引號。完成時模型忘了閉合的普通 ``` 圍欄也不切。
- 自閉合的 `<StatusPlaceHolderImpl/>`、`<status…/>` 已在第 2 步拿掉標籤本身，不切到結尾。

串流規則（前端顯示，沿用現行 `narrationStreamText` 做法）：從第一個完整標記開頭（``` 、`<details`、`<status`、`<UpdateVariable`，不分大小寫，與現行 `narrationStreamText` 同四個）一路截到結尾，不管後面有沒有閉合——「拿掉已閉合區塊、後文接著顯示」只在最終 text 做；`<maintext>` 是要拆的外殼不是控制區塊（`response.rs:405-421`），串流時只拿掉 `<maintext>`、`</maintext>` 兩個標籤字面，正文照常顯示；結尾可能是標記開頭的片段（單獨 `<`、單雙反引號、標記前半）另外暫扣，等後續字到確定不是標籤再放出。串流結束時畫面換成落檔的 text：中止時，前半片段與未閉合的 ``` 本來就會從最終文字消失；不會少字的是單獨的 `<`、單雙反引號，以及完成回合留下的前半片段。

**空正文**：接線層判斷，只有完成回合 `text` 為空才回 `AI_EMPTY_RESPONSE`（走 `settle` 的 Err，世界書落地撤回）；中止且沒正文維持現行 Ok＋aborted（`chat.rs:150` 由 settle 撤回）。CLI 路的空正文必須在抹寫之前判出來，見 §2。

**GM 中止半截**（主線裁決〔模型判斷·未裁決〕）：`aborted_narration`（`chat.rs:346`）先移除自閉合控制標籤，extract 之後照最終文字規則的「中止」那套切未閉合尾巴。GM 中止維持現行 `raw: None`——半截不套狀態、也不走變數模式提交，卡片介面那樓拿顯示文字即可，不擴大本案。

### 2. CLI 續聊線的 session 抹寫（主線裁決〔模型判斷·未裁決〕）
Claude 多角色桌與 Grok 的角色線是全角色共用一條 session（`lanes/mod.rs:202-208`），回合後抹寫只補前綴、不剝標籤，下一個角色續用 session 時會看到並模仿。改成：抹寫時把本輪 assistant 回覆換成「前綴＋剝後文字」，與正典重建一致。

`run_turn`（`lanes/mod.rs:1302` 附近）呼叫抹寫時同時傳**原文 reply**（核對用）與**剝後文字**（寫入用，＝純函式的 `text`）。

**空正文先擋**：完成回合純函式 `text` 為空時，照 runaway 的模式（`lanes/mod.rs:1265` A5）在 `run_turn` 內先走 `settle_abort`——Claude 抹掉機密段、不寫空的 assistant；Grok 整條撤線——之後再回 `AI_EMPTY_RESPONSE`（`settle_abort` 自己失敗就照既有方式回那個錯，不用 `AI_EMPTY_RESPONSE` 蓋掉）。不觸發 `run_turn` 的「續聊失敗降級重開再試一次」；回錯前不清 `pending_rewrite`（`settle_abort` :895 的語意），下一輪才會重開。不能走一般抹寫成功後才回錯，否則 session 裡多一則空白 assistant。接線層的空正文判斷對 CLI 路是第二道。

Claude（`apply_rewrite` `lanes/mod.rs:782`、`prefix_last_assistant` `lanes/session_file.rs:173-201`）：
- 簽名加原文與剝後文字。
- 收尾沒剝掉任何東西（原文去前綴、去頭尾空白後等於收尾台詞）的回合照舊只補前綴，不核對、不換寫〔模型判斷·未裁決〕；Grok 同。
- 有剝掉東西時，先核對最後一則 assistant 的 text 分段串起來是否等於 `with_prefix(原文)`（或未補前綴的原文），對不上走既有抹寫失敗、丟線。
- 對得上就把所有 text 分段收成一段「前綴＋剝後文字」（放第一個 text 分段的位置，其餘 text 分段刪掉），thinking 分段保留。
- `settle_abort`（:896，:919 也呼叫 `apply_rewrite`）改簽名時保留中止與 runaway 原本的撤線行為：中止不改寫 assistant 內容的語意照舊（只做機密抹除與既有前綴處理），不因新增參數而改成寫入剝後文字或改變丟線條件。

Grok（`grok_session.rs`）：
- `rewrite` 簽名把核對用原文 reply 與寫入用剝後文字分開傳。`check_prefixed`（:319-325）照舊用原文核對。
- 核對通過後：chat_history 那則 assistant 的 `content`（:413-414 一帶）換成「前綴＋剝後文字」；updates 裡拼接成整段回覆的 chunk（:461-470）第一個 chunk 的文字換成「前綴＋剝後文字」，其餘回覆 chunk 刪掉。
- 兩個檔的寫入都要測。

GM 線不動（沒有跨角色共用）；agy 一角一線不動（見已知差異）。

### 3. 前端
- `useChatController.ts:591-614` 中止與完成兩處 `appendEvent` 都帶 `raw`（有才帶，同 GM :722 寫法）。角色中止也帶 raw：中止的半截台詞仍會落成一樓，卡片介面讀那樓時要拿原文，與完成回合同一套。（GM 中止 `raw: None` 不變，理由見 §1。）
- 角色串流（`PlayView.tsx:240-241`）與旁白串流共用截斷函式，照 §1「串流規則」：從第一個完整標記開頭截到結尾，扣住可能是標記開頭的尾巴（單獨 `<`、`<U`…、`<s`…、`<d`…、`<m`…、單雙反引號）等確定不是標籤再顯示；`<maintext>`／`</maintext>` 只拿掉標籤字面。串流結束時畫面換成落檔的 text。

### 4. 只剝不收
角色回覆裡的 `<UpdateVariable>`／狀態欄位丟掉不套用（拍板 1）。

### 5. 網頁存檔匯入（`import/web_save/mod.rs:298-324`）
- 所有 `role==Char` 都剝：開場、Dialogue、沒有角色 id 的 Narration。
- 先拿掉自閉合控制標籤（`<StatusPlaceHolderImpl/>` 有沒有前導換行都拿、`<status…/>`），再過 `extract_state_block`，再照 §1「最終文字規則」切未閉合尾巴：沒標 `interrupted` 的訊息照「完成」、標了 `interrupted` 的照「中止」，最後 `trim_end`。
- `raw`：與剝後 `text` 比；網頁有 raw 用網頁 raw，沒有就用網頁原文 text；與剝後 text 相同就不存。
- 純控制樓（剝完沒正文）留空 text，不套生成失敗規則。

## 範圍
- 改：`src-tauri/src/transport/`（純函式與私有前綴變體、截斷規則；`own_prefix.rs` 的公用函式不動）、`src-tauri/src/commands/chat.rs`、`src-tauri/src/lanes/mod.rs`、`src-tauri/src/lanes/session_file.rs`、`src-tauri/src/lanes/grok_session.rs`、`src-tauri/src/import/web_save/mod.rs`、`src/features/play/useChatController.ts`、`src/features/play/PlayView.tsx`（截斷函式視大小可抽成同目錄小檔）。
- 不改：`extract_state_block` 本體、GM 線正常完成路、網頁版、卡片介面殼選路、已落檔的舊逐字稿（拍板 4）。

## 測試
- Rust 純函式：`<UpdateVariable>`、`<status>…</status>`、```state 圍欄、`標籤＋換行＋名字：正文`、`標籤＋名字：`（空）、前導空白＋前綴、前綴＋標籤並存、`狐狸：狐狸：正文`（只剝一層）各一；`text` 乾淨、無差異時 raw 為 None；「名字在標籤後面、且 `<UpdateVariable>` 內也含「名字：」」時 raw 只拿掉正文開頭那一處前綴。
- Rust 未閉合尾巴（最終文字規則）：完整標記開頭未閉合的 `<UpdateVariable`／`<status`／```state／任意 `<details`，中止與完成都切；`正文<Upd` 中止切、完成不切；任意未閉合 ``` 中止切、完成不切（模型忘了閉合的普通圍欄）；`正文<`、單反引號、雙反引號兩種都不切；台詞中間的 `<StatusPlaceHolderImpl/>` 只拿掉標籤、後面正文保留；`<status/>正文<status>狀態</status>尾文` 得「正文尾文」（自閉合先移除）；大寫 `<UPDATEVARIABLE`、`<Status` 同樣認。
- Rust raw 位置：`<StatusPlaceHolderImpl/>\n狐狸：你好` 的 raw 不帶「狐狸：」、也不切進標籤；多個控制區塊、`<maintext>` 外殼、正文含多位元組 Unicode 時，名字在標籤後面的 raw 都只拿掉正文開頭那一處前綴。
- Rust 接線層：完成且純標籤回 `AI_EMPTY_RESPONSE`；中止且無正文回 Ok＋aborted。
- Rust GM `aborted_narration`（中止規則）：停在未閉合標籤裡、停在 `<Upd` 前半、停在未閉合的普通 ``` 都切乾淨；自閉合混合案例同上。
- 前後端對拍（共用一份案例 JSON）：兩邊對「完整標記開頭、前半片段」的判定一致（`正文<`、`正文<Upd`、單反引號、雙反引號、`<maintext` 前半、大小寫變體）；同一份完整回覆，TS 串流結束前最後顯示的內容不比 Rust 最終文字多出任何標籤。
- Rust 續聊線走真的 `plan_turn`：帶標籤的角色回覆落檔後下一輪接受（不 ReplyDiverged）、換另一角色續用同一 session。
- Rust Claude 抹寫：最後一則 assistant 多個 text 分段＋thinking 分段 → 收成一段「前綴＋剝後文字」、thinking 保留；文字與原文對不上 → 抹寫失敗丟線；`settle_abort` 中止與 runaway 撤線行為不變（既有測試綠＋補一條帶新參數的）。
- Rust Grok 抹寫：chat_history 的 assistant 與 updates 的多 chunk 回覆都換成「前綴＋剝後文字」（第一 chunk 換、其餘刪），核對仍用原文、對不上丟線。
- Rust 純標籤完成回合（走 `settle_abort` 後回 `AI_EMPTY_RESPONSE`）：CLI 只被呼叫一次（不降級重試）；Claude session 檔沒有機密段、也沒有空白 assistant；`pending_rewrite` 留著；Grok 那條線已撤；下一輪重開。
- Rust 指紋：只有 `raw` 不同、`text` 相同的事件，`events_fingerprint` 不變。
- Rust 匯入：沒有 raw、已有 raw、GM 路匯入、開場、純控制訊息、中斷訊息（停在 `<Upd` 前半：interrupted 切、未標 interrupted 不切）各一，`text` 不含 `<UpdateVariable>` 與 `<StatusPlaceHolderImpl/>`，`raw` 依規則保留或不存。
- 既有會變紅的測試改法：
  - `import/web_save/export_tests.rs:173` 原驗 text 含 placeholder → 改驗 text 乾淨、raw 保留。
  - `import/web_save/tests.rs:853-902`（D24）原靠 placeholder 驗「送模前不跑 promptOnly」→ 測試內把允許與拒絕兩份存檔輸入的卡上 promptOnly 腳本「送模前拿掉狀態欄」的 findRegex 都改成正文裡的字面（例如 `/銅板/g`，fixture 不動），斷言兩桌送出歷史都仍含「銅板」。
- vitest：串流截斷逐字送、跨 chunk 送都不閃出 `<Upd`／`<maintext` 等前半段；開標籤→內容→閉標籤→後續正文逐 chunk 送，標籤起都不顯示；GM 串流途中狀態區塊已閉合仍不顯示；角色「標籤＋換行＋正文」串流中不顯示標籤；`<maintext>正文…` 串流時正文逐步顯示、外殼不顯示；確定不是標籤的 `<` 在後續字到時顯示；串流結束畫面換成落檔 text；`replyOnce` 完成與中止兩處落檔都帶 raw。
- `npm run verify` 全綠。
- 測試通道（用新桌或新匯入的桌）：一張輸出格式條目會讓角色照抄 `<UpdateVariable>` 的卡（`TestCards/` 取），請角色發言，畫面與逐字稿 `text` 無標籤、下一輪 GM 診斷提示詞看不到角色的標籤。

## 已知差異
- 與 ST：ST 群組裡角色回的 `<UpdateVariable>` 會被 MVU 寫進變數；桌面版角色線只剝不收（拍板 1）。
- 卡片介面讀 raw：沒有隱藏 regex 的卡，在卡片介面區仍看得到標籤（ST 也一樣）。
- 結尾的普通 ``` 圍欄會被 `extract_state_block` 拿掉，與 GM 線相同。
- agy 一角一線的 session 仍留著該角色上一輪的標籤，該角色續聊時看得到自己上一輪的標籤（不外流給別的角色，正典乾淨）。
- 純控制樓匯入後是空泡泡；送模時是一行「名字：」空台詞。
- 供應商截斷（長度上限等）後端仍可能走「完成」路徑，尾端的 `<Upd` 這類前半片段依規則保留，不保證所有截斷標記都會消失。
- 串流中，已閉合的控制區塊之後的正文要等串流結束換成落檔 text 才看得到（沿用現行旁白串流行為）。
- 串流時 `<maintext>` 改成只拆外殼、正文逐步顯示（原旁白串流不認它，標籤字面會出現在畫面上）。
- 網頁版不做 app 層剝除，照 ST 依賴卡的 regex；同一張沒帶隱藏 regex 的卡，網頁版會顯示標籤、桌面版不顯示。
- 驗收要用新桌或新匯入的桌；舊桌已落檔的台詞不回溯（拍板 4）。

## 待作者拍板
1. **角色回覆裡的 `<UpdateVariable>`／狀態欄位要不要套用**
   - A 只剝不收：狀態只有 GM 能改，角色照抄的格式條目不會動到數值；角色真的想改狀態（例如付錢）這輪不生效，要等 GM 下一輪寫。
   - B 照 GM 套用：與 ST＋MVU 一致；但角色只看得到自己那支分支，寫出的路徑可能錯或蓋掉 GM 剛寫的值，還要給角色回合加狀態提交、骰值、面板刷新與撤回，等於再做一條 GM 的寫狀態流程。
   - 建議 A：角色線本來就沒被要求輸出狀態，標籤是照抄格式條目的副產品；B 費工且容易寫壞狀態。
2. **剝完沒有正文**
   - A 照 GM 當失敗（`AI_EMPTY_RESPONSE`，跳回合失敗框可重試）：不會落一則空白台詞。
   - B 落一則空台詞／不落但不報錯：玩家看到角色「沒講話」卻不知道原因。
   - 建議 A。
3. **網頁存檔匯入的訊息一起剝**
   - A 一起剝：網頁存檔匯進桌面版後，角色台詞與 GM 旁白不再出現 `<UpdateVariable>`、`<StatusPlaceHolderImpl/>` 字面，續玩時也不會被送回模型；符合公開前門檻「網頁存檔匯入後能續玩」。
   - B 不剝：匯入的舊訊息照現在顯示標籤字面，也會一直送回模型讓它模仿；另立案處理。
   - 建議 A：同一類問題、同一個剝除函式，多改一個函式。
4. **已落檔的舊逐字稿**
   - A 不處理：新回合起乾淨；舊桌已落檔的角色台詞仍顯示標籤，也仍送回模型。
   - B 對舊 Dialogue 事件補剝：只在組裝與顯示時另外剝，舊桌就乾淨、不碰續聊指紋，但每次顯示／組裝都多跑一次；若改寫落檔的 `event.text`，已送段指紋改變，每條續聊線會整線重開一次。
   - 建議 A：發佈前舊桌不必相容（作者既有原則），B 多一層每次都跑的處理不划算。
