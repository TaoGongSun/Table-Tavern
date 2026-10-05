# 重構卡 PNG 匯出：單檔圖卡＋含角色圖版＋套用映射地基

Status: in-progress

## Summary
匯出三階全走單一 PNG：ST 卡（已有）／重構卡 PNG／重構卡＋角色圖 PNG（全身圖＋裁切頭像，排除 gen-gallery）。自家私有 chunk 存 manifest 與圖片原始 bytes，asset_id 配對不靠 chunk 順序；前置地基＝套用時把 outcome_index→character_id 映射持久化進 refactor-outcome.json（相容舊裸 JSON）。規格見 [refactor-card-png-export 規格](../plans/refactor-card-png-export.md)。2026-08-15 使用者拍板＋Sol（GPT-5.6）二輪收斂同意。

## Next action
包 A（套用映射持久化）已完成、verify 10 步綠：apply 落檔改封套（refactor/card_file.rs，含 applied 完整性驗證、舊裸檔相容、對外去 player_card_id）；前端 refactor-card.ts 解析封套並以 applied 重現預設勾選。下一步包 B（共用 codec＋PNG 匯出），規格見計畫「施工計畫」段。包 A 的 GUI 實測併入包 D。測試通道整機一把鎖，啟動前 ps 確認沒有別人的實例，用完立刻 quit。

## 待使用者決定
（無）

## Constraints
- 匯入是信任邊界：整包原子驗證（CRC、hash、asset 對帳、張數與總大小上限、MIME 與尺寸），manifest 不含檔名欄位。
- #2/#3 不寫 chara/ccv3；匯出底圖剝舊卡片 chunk 與舊版自家 chunk。
- 檔名尾碼僅供玩家辨識，格式判定只認 chunk。
