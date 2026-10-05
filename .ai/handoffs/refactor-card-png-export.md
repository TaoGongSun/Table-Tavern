# 重構卡 PNG 匯出：單檔圖卡＋含角色圖版＋套用映射地基

Status: awaiting-verification

## Summary
匯出三階全走單一 PNG：ST 卡（已有）／重構卡 PNG／重構卡＋角色圖 PNG（全身圖＋裁切頭像，排除 gen-gallery）。自家私有 chunk 存 manifest 與圖片原始 bytes，asset_id 配對不靠 chunk 順序；前置地基＝套用時把 outcome_index→character_id 映射持久化進 refactor-outcome.json（相容舊裸 JSON）。規格見 [refactor-card-png-export 規格](../plans/refactor-card-png-export.md)。2026-08-15 使用者拍板＋Sol（GPT-5.6）二輪收斂同意。

## 現況
包 A–D 已進 main，Sol 驗收全數通過（計畫三輪、A 兩輪、B 三輪、C 五輪、D 一輪），verify 10 步綠；驗收中定案的取捨寫在規格檔末段。程式落點：
- 後端 `src-tauri/src/refactor/`：card_file.rs（封套與 applied 驗證）、card_png.rs（ttRd／ttAs codec、CardLimits）、card_export.rs（三階匯出）、card_import.rs（嗅探、開卡、暫存單槽）、apply.rs `apply_with_assets`、apply_record.rs（套用＋收據、失敗嚴格回滾）；`import/png_image.rs`（PNG 嚴格串流驗證）；receipts.rs `rollback_last_import`。
- 前端：`src/features/refactor/refactor-card.ts`、useRefactorWorkflow（開檔世代、token 收尾、匯出選單與大小）、陣容欄單一入口（useImportController → App → WorldEditor）。

包 D 測試通道實測（2026-10-06，零 AI 派送）全過：三階匯出（#3 chunk 一個 ttRd＋三個 ttAs、無 chara／ccv3；#2 無 ttAs；JSON 是封套且無 player_card_id；含圖指 .json 被拒）、新桌陣容欄丟 #3 PNG 自動開世界設定出結果卡（附 3 張角色圖、依來源設定套用）、圖片 bytes 與來源相同、ST 卡丟進重構卡入口被拒、59.1 MB 大卡匯出 3 秒、QuickLook 縮圖與 sips 正常、大卡經陣容欄匯入開卡 5.7 秒並套用放上 5 張圖。驅動腳本沒進 repo。

## Next action
已合併進 main。剩沒有環境的兩項實機驗收（[實測佇列](../reference/verification-queue.md)梯 1 第 4e 項）：Windows Explorer 看 #3 大卡縮圖與大小、SillyTavern 匯入 #2／#3 PNG 乾淨拒收。兩項驗完才算實測完成、可歸檔。

## 待使用者決定
（無）

## Constraints
- 匯入是信任邊界：整包原子驗證（CRC、hash、asset 對帳、張數與總大小上限、MIME 與尺寸），manifest 不含檔名欄位。
- #2/#3 不寫 chara/ccv3；匯出底圖剝舊卡片 chunk 與舊版自家 chunk。
- 檔名尾碼僅供玩家辨識，格式判定只認 chunk。
