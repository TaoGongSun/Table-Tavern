# Task
Task-ID: card-export
Title: 角色卡匯出（SillyTavern chara_card_v2）
Status: completed
Created: 2026-07-28T00:00:00+08:00
Updated: 2026-07-28T00:00:00+08:00

## Summary
匯入已有（post-mvp-st-import），但卡片只進不出。目標：卡片編輯畫面加「匯出卡」，另存成 SillyTavern 規格的 PNG（tEXt chara chunk）或 JSON，讓玩家把在本 App 編好的卡帶去 ST 或分享。

## Next action
- 無。2026-07-28 實作完成，cargo test 110 綠（新增往返一致、手寫卡轉 JSON、底圖 chunk 長度與 CRC、舊 chara 被換掉四項），前端型別檢查通過；commit db0ec09。待使用者實機匯出一張卡丟回 SillyTavern 確認

## Constraints
內容一律由現在的卡重建，不倒出匯入時的原檔（改過的字要跟著出去）；公開五段回原欄位、手寫卡歸簡介，私有筆記轉角色卡自帶世界書；底圖取卡片圖→頭像→1×1 透明，並清掉底圖裡舊的 chara／ccv3；有未存修改時擋下。
