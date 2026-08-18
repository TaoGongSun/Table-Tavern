# refactor-card-png-export — 規格

2026-08-15 使用者拍板＋Sol（GPT-5.6）二輪查證收斂同意；逐字稿在 Codex app 同名 Companion 串。

## 目標與格式選擇

匯出分三階，全部單一 PNG。為何選 PNG：主要情境是玩家互傳，metadata 剝除是全生態共同前提（ST 卡過重壓平台一樣壞，圈內靠 Discord 附件／catbox 等保留原檔通道），而 zip 要寫各 OS 縮圖外掛才看得到封面，PNG 圖素天生是封面。

1. **ST 相容卡**：已有（card-export 結案，tEXt `chara` chunk），不動。
2. **重構卡 PNG**：RefactorOutcome 封進自家私有 chunk；沿用「副檔名決定格式」慣例，.json 照舊可存。
3. **重構卡＋角色圖 PNG**：#2 再加角色圖 chunk。

三階靠檔名尾碼區分、封面不蓋徽章；尾碼僅供玩家辨識，格式判定只認 chunk。

## chunk 與 manifest 規格

- 兩種 chunk：manifest 一個＋圖片 chunk 可重複。四字名取 ancillary＋private＋safe-to-copy（例 `ttRd`／`ttId`，實作定案）；payload 開頭放 magic＋版本，防私有名稱碰撞。
- manifest JSON：`{ format, version, outcome, applied?, assets }`；assets 每筆 `{ asset_id, outcome_index, kind: portrait|avatar, mime, length, hash }`，**不含檔名欄位**（路徑注入面直接消滅）。
- 圖片 chunk 直存原始 bytes（免 base64）；配對只認 asset_id，不依賴 chunk 順序（PNG 編輯器可重排 ancillary chunk）。
- 匯出底圖剝掉既有 `chara`／`ccv3`＋舊版自家 chunk，避免新舊疊包。#2/#3 不寫 `chara`，ST 誤吃會乾淨報「找不到角色卡」。
- 匯入單一拖放入口，按 chunk 嗅探分流 `chara`／`ccv3`／自家。

## 地基：套用映射持久化（前置包）

現況 `worlds/<id>/refactor-outcome.json` 存原始 outcome（refactor.rs `write_refactor_outcome`），outcome_index → 建卡 character_id 的映射只活在 `RefactorApplyResult` 記憶體，角色改名後 #3 匯出配不回圖。改法：

- 落檔擴充為 `{ format, version, outcome, applied }`；applied 含 outcome_index → character_id 映射（走 person 條目沒建卡的 index 標記無卡）＋玩家卡資訊；二次套用整份覆寫。
- 讀取端同時接受舊版裸 RefactorOutcome，既有 JSON 重構卡不失效。
- 接收端套用要拿明確 index → id 映射，不靠 character_ids 陣列順序（勾選可為不連續）。
- `applied.player_card_id` 屬來源桌本地資訊，跨桌重現玩家選擇用邏輯 player_index。

## #3 範圍

- 角色圖＝各角色目前的全身圖＋avatar.png 裁切頭像；排除 gen-gallery（個人生成資產、體積大、桌子運行不需要）。
- 卡內容凍結為 outcome 的角色；套用後手動新增的角色屬世界編輯，不進卡。

## 匯入驗證（信任邊界）

整包原子驗證：CRC、hash、重複／缺漏 asset 對帳、張數與總大小上限、圖片 MIME 與尺寸；上限數值實作時定。

## 驗收

- 刻意做一張數十 MB 大卡實測：macOS Finder／QuickLook、Windows Explorer、ST 匯入拒收訊息、本應用匯出→匯入往返。
- 匯出完成訊息顯示檔案大小（Discord 免費附件上限目前約 10MB）。
