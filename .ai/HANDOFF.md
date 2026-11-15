# 工作線索引

一線一檔於 [handoffs/](handoffs/)，交接＝就地改寫該檔。開工先整份讀該檔。

分類規則：仍需程式施工／規格落地才放「進行中」；施工與這台機器測得到的實測都做完、只剩這台機器驗不到的條件，放「等這台機器驗不到的條件」，每一條都要在[待實測清單](reference/verification-queue.md)有對應列。兩者都沒有就結案歸檔到 [handoffs/archive/](handoffs/archive/)、從本檔刪掉。

## 進行中

- [release-9](handoffs/release-9.md) — v0.5.4 發佈：版本、更新說明、README、截圖都已進 main；部署手機端、演練打包、推正式 tag 等作者說

## 等這台機器驗不到的條件（細項見[待實測清單](reference/verification-queue.md)）

- [multiplayer-room](handoffs/multiplayer-room.md) — 多人跑團房間 v1：已進 main、伺服器已部署、不需真手機的部分已用測試通道驗完；剩真手機、Windows／Linux 防休眠、牆內網路、Caddy、無人值守一晚
- [multiplayer-friends-test](handoffs/multiplayer-friends-test.md) — 給朋友測試的多人連線版：已上線；剩朋友用自己的手機實際玩一場
- [room-status-strip](handoffs/room-status-strip.md) — 桌邊狀態列＋房主提醒：畫面已驗；剩提示音要人耳聽、iOS 提示音與熄螢幕未讀要真手機
- [room-table-chat](handoffs/room-table-chat.md) — 桌邊聊天：送出與私訊已驗；剩中文輸入法選字時按 Enter 不誤送（要真輸入法）
- [room-scene-capacity](handoffs/room-scene-capacity.md) — 幕容量提醒：提醒本身已驗；剩舊版桌面接新伺服器（要舊版安裝包）
- [vn-audio](handoffs/vn-audio.md) — VN 音訊：畫面與音軌狀態已驗；剩實際聽（要人耳）與音效各項（要 Freesound 金鑰）
- [vn-credits](handoffs/vn-credits.md) — VN 感謝名單：音樂署名已驗；剩音效署名（要 Freesound 金鑰，列在 vn-audio 那一列）
- [vn-pack-opaque](handoffs/vn-pack-opaque.md) — 作品包不透明容器：匯出匯入已驗；剩音效進包（要 Freesound 金鑰，列在 vn-audio 那一列）
- [desktop-csp](handoffs/desktop-csp.md) — 桌面版 CSP：macOS 已驗；剩 Windows 安全驗收
- [refactor-card-png-export](handoffs/refactor-card-png-export.md) — 重構卡 PNG 匯出：macOS 與 SillyTavern 拒收已驗；剩 Windows Explorer 縮圖
- [release-2-ci-windows](handoffs/release-2-ci-windows.md) — 桌面版發佈：v0.5.3 已發布（未簽章）；剩作者在 Mac 與 Windows 照繞過步驟下載安裝能開
- [ui-redesign](handoffs/ui-redesign.md) — 介面整體重新設計：已進 main；剩要新版才出現的對話窗與 Windows 外觀（譯詞重定已做，2026-11-15）
- [desktop-update-detect](handoffs/desktop-update-detect.md) — 桌面版 App 內更新與回退：剩兩個真發佈版的端對端驗收〔作者裁決 2026-10-12〕
- [image-model-picker](handoffs/image-model-picker.md) — 生圖模型下拉與統一存 PNG：三家 CLI 真生圖已驗；剩付費 OpenRouter 生圖模型（要儲值帳戶）
- [image-model-free-tier-hide](handoffs/image-model-free-tier-hide.md) — 免費層不顯示生圖模型選單：免費金鑰已驗；剩付費金鑰那一半（列在 image-model-picker 那一列）
- [api-shared-lane](handoffs/api-shared-lane.md) — API 路徑改走共線：剩 API 快取成對測試（要會回報快取的付費模型）
- [refactor-survey-spans](handoffs/refactor-survey-spans.md) — 盤點四分類＋照搬零輸出：剩 API 模式退 GM 模型（要 API 額度）；T1–T3 盤點品質補測另立 BACKLOG
- [refactor-dispatch](handoffs/refactor-dispatch.md) — AI 重構提速省費：只剩 P8（API 模式跑重構，列在 refactor-survey-spans 那一列）
- [claude-compat-endpoint](handoffs/claude-compat-endpoint.md) — Claude 相容端點：實作完成；剩真相容端點實測（要端點與金鑰）
- [i18n-more-languages](handoffs/i18n-more-languages.md) — 十國語言：版面與殘留字十語系已驗（2026-11-15）；剩母語者審校
