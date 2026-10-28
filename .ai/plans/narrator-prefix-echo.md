# narrator-prefix-echo 規格

[交接檔](../handoffs/archive/narrator-prefix-echo.md)。三方共識（主線、Sol、opus-review）2026-10-28〔模型判斷·未裁決〕。

## 問題
續聊線（claude／agy／grok）把 GM 自己先前的旁白攤成 user 文字時標「（旁白）」，GM 把它當回覆格式照寫在正文開頭，落檔後又以 assistant 訊息留在 session，越抄越穩。單發攤平標「GM：」、API 用 assistant role，都沒有這個記號。

## 做法
- **G 主修**：`transport::lane_event_line` 的 GM 側（`Side::Gm`、`Side::GmOpeningRaw`）旁白改標 `speaker_prefix("GM", lang)`（「GM：」／「GM: 」），與單發攤平一致，收尾句「不要加名字前綴」直接涵蓋；修正前落檔、開頭帶照抄標籤的旁白，GM 側送出前一併剝掉（只動送出字，事件與指紋不變）。角色側照舊「（旁白）」。
- **E 保險**：`transport::strip_narrator_label` 在 `gm_reply_parts` 最後一步（狀態區塊、點名行、尾端狀態段之後）與中止半截旁白套用：顯示正文 trim_start 後，開頭連續的「（旁白）」「(Narration)」「GM：」「GM:」（後接空白一併）全剝，中段不碰；raw、機制輸入與點名照原文。剝完沒正文照既有規則回 AI_EMPTY_RESPONSE。
- 續聊預期回覆（`lanes::plan::expected_reply_for`）刻意不做顯示層剝除：剝過的回合下一輪 ReplyDiverged 重開、清掉帶標籤的 session，這是有意的（重開不代表舊 session 檔已刪）；重開後沒再抄就照常續聊。

## 已知限制
- 舊 session 裡已送出的「（旁白）」行不追改，重開才換。
- 送出前剝除只認開頭；歷史旁白中段的照抄標籤照送。
- E 只在落檔時剝，串流途中會先看到再消失。
- 卡片角色或玩家剛好叫「GM」時，GM 線分不清那行是誰說的。
- 角色側沿用「（旁白）」；掃描角色台詞照抄 0，未處理。
- 卡片自訂格式剛好要求旁白以「GM:」開頭時，畫面上那個標籤會被剝掉，raw 不受影響〔模型判斷·未裁決〕。
- API 路徑與單發攤平的 GM 旁白（`transport/assemble.rs` 的 GM 組裝）沒套歷史剝除：修正前的舊桌換走單發的供應商，可能看到「GM：（旁白）…」雙標籤〔模型判斷·未裁決〕。

## 驗收
- 發生率：測試通道既有桌全掃，claude 線 GM 旁白開頭照抄 20/91，api 0/883、agy 0/6、codex 0/4、角色台詞 0。
- 重播（echo/root1 開線形狀，長歷史）：原樣 Haiku 7/8、Gemini 3.8 Flash Medium 3/4 → G 各 0/8、0/4，`<UpdateVariable>` 與點名照常。
- 已抄過的歷史（8 段旁白內文帶標籤）Sonnet：只換標籤仍 6/6 照抄（原樣 5/6），所以 GM 側送出前要剝掉歷史裡的舊標籤。
- 長歷史乾淨版 Sonnet 0/6→0/6、第一輪 1/6→0/6；測試通道 z23 那份桌（原本 2/2 照抄、歷史帶標籤）Sonnet 真玩 5 輪：raw 開頭 0/5，續聊線一條 session 到底沒反覆重開。
- 單元：G 中英與兩個 GM 視角、角色側不變；E 正反例（四標籤、帶不帶空白、雙標籤、標籤後無正文、中段、近似字串）；`gm_reply_parts` 正文／raw／機制輸入；剝過的回合下一輪 ReplyDiverged、重開後續聊。
