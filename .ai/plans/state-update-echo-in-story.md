# state-update-echo-in-story 規格

[交接檔](../handoffs/archive/state-update-echo-in-story.md)。三方共識（主線、Sol、opus-review）2026-10-28〔模型判斷·未裁決〕。

## 問題
歷史裡的「（系統）狀態更新」事件，在卡片或世界書要求每回合附狀態回報時被 GM 當成格式模仿，自寫在正文尾端並顯示。目前重現集中於 hroot2；一般 MVU 桌 21 回合未見。

## 做法
- **C 主修：只改送模型的標頭。** 事件照留歷史（state-values-mvu 包 5 的快取設計不動）。`transport::arrivals::prompt_text` 對 `StateUpdate` 改用送模型專用標頭，共用字典 `transcript-marker.json`（UI、匯出）不動；只分 zh-TW／en 兩種（同 `prompt_lang`）。標頭同時講「供理解目前狀態」與「不是回覆格式、不要重印」，避開「狀態更新」「狀態欄」「欄位」等與協定撞名的字。GM 與角色線同一出口。
- **E 保險：後端回覆端剝除。** `gm_narrate` 在 `extract_next_speaker` 之後、空正文檢查之前剝 display；raw 與機制解析輸入保留原文。全部成立才剝：獨立標頭行完全相符（行首有縮排不算）（新標頭、舊標頭各語系與 `（系統）`／`（系统）`／`(System) ` 前綴寫法）；段落位於顯示尾端；其後每一行都是「本桌狀態路徑：值」（比對全部現有路徑）；不在 CommonMark 程式碼區塊內（pulldown-cmark 判斷；沒閉合的圍欄延伸到文末）。任一行對不上就整段保留。

## 已知限制
- 續聊線指紋按原事件算，不因此重開；已在 session 裡的舊標頭不追改，下次重開才換新。
- E 只在落檔時剝，串流途中會先看到再消失；剝過的回合續聊線預期回覆對不上，下一輪 ReplyDiverged 重開、把帶照抄段的 session 換掉，這是有意的（重開不代表舊 session 檔已刪）。
- 英文標頭沒有拿模型實測；中文初稿 1/20 證明措辭會影響結果。
- E 只剝 GM 正文，角色線不剝。
- 模型縮寫或改寫標頭後照抄，E 剝不到，主修靠 C。
- 卡片自訂格式剛好以「（系統）＋狀態變動標頭」開頭、後面全是本桌路徑行時會被一併剝掉（罕見）。

## 驗收
- hroot2 同批原樣 vs C，Sonnet 各 ≥20 次，C 須 0。
- agy（Gemini 3.8 Flash Medium）續聊線原樣 vs C 各 ≥8 次（原樣組有抄才算比得出）。
- 抽查 C 組現值照常推進（如驻守天数）。
- E 反例（卡自訂狀態段、跨行值、對話引用「（系統）」、混入不認得路徑）全保留；正例剝除後 raw 保留。
- 既有續聊 session 指紋不變。verify 12 步綠。

## 驗收結果（2026-10-28）
- hroot2 攤平重播：Sonnet 原樣 12/20、定稿 0/20（另一版不帶「（系統）」前綴的寫法也 0/20）；agy Gemini 3.8 Flash Medium 原樣 10/10、定稿 0/10（開線形狀＝system＋全量歷史，續聊只差在不重送歷史）。
- 現值照常推進：定稿組 20 次中 18 次 replace 剧情阶段、16 次 delta 驻守天数（原樣 19／17）。
- E 正反例單元測試（`reply_cleanup_tests.rs`，含縮排程式碼、未閉合 ~~~、四反引號圍欄遇三反引號、關閉記號後有文字）；程式碼區塊判斷用 pulldown-cmark（CommonMark）；提交層 `commands::chat::gm_reply_parts` 測試同時核對正文、raw 與機制輸入（`block`）。
- 續聊指紋只吃原事件（`lanes::plan::events_fingerprint`），渲染改字不重開。verify 12 步綠。
- 「下一位發言」：hero roster 桌沒有角色卡、導演指示是不點名版，屬卡片自身格式，不立案。

## 範圍外
- GM 正文寫「下一位發言」：先查該桌是否不點名桌，是就不立案。
- 「（旁白）」前綴照抄、重構世界書「每回合在狀態欄回報」措辭：另案（narrator-prefix-echo、refactor-worldbook-statusbar-wording）。
