# Task
Task-ID: multiplayer-room
Title: 多人跑團房間：AI 當 GM、全員手機玩、出額度者的桌面版當 GM 執行端
Status: 等實機驗收（v1 包 0–7 全部通過驗收、2026-11-03 已進 main；剩作者本人實機項目）
Created: 2026-11-02

## Summary
v1＝出額度者家裡的桌面版當 GM 執行端，所有人（含出額度者）用手機瀏覽器玩；伺服器只做中繼與回合裁判、不碰金鑰。規格與裁決見 [plans/multiplayer-room.md](../plans/multiplayer-room.md)（§11 拍板、§12 觀察、§13 Node 殼），協定見 [plans/multiplayer-room-protocol.md](../plans/multiplayer-room-protocol.md)（§14 Workers 實作差異、§15 Node 殼差異，以這兩節與程式為準）。v2（伺服器代打、純手機群）另行立案；APK 屬 [cn-apk-shell](../tasks/cn-apk-shell.md)，不在本案。

## 落點
- 伺服器：`party/server/core/`（純 TS）、`party/server/workers/`（DO 殼）、`party/server/node/`（自架殼，用法見 `main.ts` 檔頭）。
- 手機客戶端：`party/src/`（繁簡英齊、七語先英文；QQ／微信內建瀏覽器引導固定簡中，QQ 優先）。
- 桌面版執行端：Rust `src-tauri/src/room.rs`＋`room/`、`commands/room.rs`；前端 `src/features/room/RoomPanel.tsx`（進入前顯示出額度者風險說明）。多玩家 GM 提示在 `chat_turn/party_batch.rs`。
- 中國供應商：桌面版「連接 AI」的矽基流動（`transport/siliconflow.rs`），房間 capabilities 多一條 `siliconflow` 路線。

## 下一步：作者實機驗收（排在[實測佇列](../reference/verification-queue.md)）
1. 真手機：iOS Safari、Android Chrome 加入選角、送出／收回／跳過、串流、鍵盤彈出；手機 QQ、微信內建瀏覽器看到簡中「用瀏覽器開啟」引導，照文案操作能跳到外部瀏覽器（QQ 選項名稱「浏览器打开」未在真機核對），QQ 瀏覽器 App 不跳引導。
2. iOS 鍵盤開著時點「送出」要點兩次：確認是否只是模擬器注入點擊的現象。
3. 防休眠：Windows 先 `cargo check`（`SetThreadExecutionState` 路徑未編譯驗證）再實跑；Linux `systemd-inhibit`。
4. 牆內：系統代理模式下 WebSocket 走 CONNECT 連得上 Cloudflare。
5. 命令列工具通道（Claude／Codex／Gemini）在房間模式無人值守跑一晚；撞額度時手機端看到 quota 類錯誤並能切路線。
6. 真 Workers 部署：已完成（2026-11-03，自訂網域 `tt-room.pirahus.com`，桌面版預設連它；見 [multiplayer-friends-test](multiplayer-friends-test.md)），端到端實跑過。
7. Node 殼放在 Caddy 反向代理後實跑（`PUBLIC_ORIGIN`、`TRUST_PROXY=1`），手機經 https 加入跑兩回合。

## 已知限制（v1，不另立案）
- 伺服器：切模型後 `round.dispatch.settings` 不更新（手機標頭顯示舊模型，執行端靠本機 `room.json` 覆寫撐）；`seat.create` 連點多建卡；成員上限 12 含已離開者；快照含第 1 幕整份種子歷史無上限；`scene_full` 的 held 仍可 `round.continue`；`webSocketClose` 未回 `ws.close()`。Node 殼：狀態與鬧鐘分兩次落地（崩潰時鬧鐘早醒或晚到一次）、靜態檔 realpath 檢查與開檔之間有 TOCTOU 窗（自架者控制目錄）、建房限流只在單程序內。
- 執行端：socks 代理與代理帳密不支援；桌面版不報額度，撞額度只靠 `gen.failed`；覆寫槽整個程序共用（房間期間其他桌含重構也走房間路線與模型）；held 後晚到結果丟掉；NPC 不在名冊（>64 張卡）時 `gen.part.begin` 被 4400 關連線會無限重送；QR 碼未做；境內供應商以 HTTP 400 帶審查文字拒答時被歸成「連不上供應商」（動作仍可切路線）〔作者裁決 2026-11-03〕；矽基流動路線的生圖仍走 OpenRouter。
- 引擎：`{{user}}` 與世界書玩家名只代換主玩家；分段摘要無多人句；低階模型可能替真人玩家卡說話（多人段指示在、遵守度看模型）。
- 手機端：七語先英文；純文字不做 Markdown；不顯示卡片介面〔作者裁決 2026-11-02〕；自建角色、改暱稱、人數上限設定未做；故事串不顯示「某某觀望」；`round.opened` 不帶暫停剩餘時間。
- 協定 §13 另列 9 條（執行端永久失聯只給匯出、去重以派工為單位等）。

## Constraints
- AI 額度一律由出額度者自備，伺服器不持有任何平台帳號或金鑰。
- 專案 AGPL-3.0：誰拿改過的伺服器給別人用，要向使用者提供原始碼；交給朋友營運前要說清楚。
