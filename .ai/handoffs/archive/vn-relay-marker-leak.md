# vn-relay-marker-leak — VN 桌「下一位：X」接力行漏進對話框

Status: 已合 main（2026-11-05；小案、Sol 驗收 1 輪 PASS、verify 17 步綠）。

## Summary
重播桌播放時 GM 回合尾的點名行「下一位：老婆婆」顯示成一格對白（vn-ui-polish 審視截圖 32）。根因：模型把點名行寫兩次（```state 圍欄前後各一），`transport/response.rs` 的 `extract_next_speaker` 用 `replacen` 照字串內容剝第一個相同的行，另一行落檔。

改法：抽 `relay_name(line)`；`extract_next_speaker` 名字取最後一行、所有點名行一律剝掉（寫入端根因）；`chat_turn/vn_script.rs` 的 `parse_cells` 讀取時點名行不成格（舊桌與已落檔的作品包也乾淨）。核心約 27 行、測試約 25 行（真實落檔原文當夾具；重複、半形冒號、前後空白、`[choices]` 前後、中英混用）。

## 已知限制
- 正文若有獨立整行以「Next:」或「下一位：」開頭會被剝（舊版也剝最後一行，低頻碰撞）。
- 聊天視圖直接顯示落檔文字（`src/shared/ui/event-text.ts`），舊污染事件在聊天視圖仍露出；新回合乾淨。
- 角色線自己寫點名行時角色收尾仍存下（`vn_script.rs` 約 671 行），VN 播放擋住、聊天視圖會露出；要模型違反角色格式才觸發。
- 舊污染文字匯出作品包仍帶著；只保證讀取播放乾淨，不清洗既存檔案。
- `.lines()` 重組把 CRLF 轉 LF；未命中點名時原文不變。
