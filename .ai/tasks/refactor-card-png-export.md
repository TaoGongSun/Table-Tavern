# 重構卡 PNG 匯出：單檔圖卡＋含角色圖版＋套用映射地基

Status: todo

## Summary
匯出三階全走單一 PNG：ST 卡（已有）／重構卡 PNG／重構卡＋角色圖 PNG（全身圖＋裁切頭像，排除 gen-gallery）。自家私有 chunk 存 manifest 與圖片原始 bytes，asset_id 配對不靠 chunk 順序；前置地基＝套用時把 outcome_index→character_id 映射持久化進 refactor-outcome.json（相容舊裸 JSON）。規格見 [refactor-card-png-export 規格](../plans/refactor-card-png-export.md)。2026-08-15 使用者拍板＋Sol（GPT-5.6）二輪收斂同意。

## Next action
排程待定；開工首包＝套用映射持久化（refactor-outcome.json 擴充 envelope＋舊格式相容讀取），再做 #2/#3 PNG 封裝。

## Constraints
- 匯入是信任邊界：整包原子驗證（CRC、hash、asset 對帳、張數與總大小上限、MIME 與尺寸），manifest 不含檔名欄位。
- #2/#3 不寫 chara/ccv3；匯出底圖剝舊卡片 chunk 與舊版自家 chunk。
- 檔名尾碼僅供玩家辨識，格式判定只認 chunk。
