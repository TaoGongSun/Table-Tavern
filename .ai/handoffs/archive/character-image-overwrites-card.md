> 結案 2026-10-12：進 main，verify 12 步綠、三方驗收 PASS。

# 換／刪角色圖會蓋掉匯入的原卡 PNG

Status: done

## Summary
原卡改存 `characters/<id>.card.png`（或 `.import.json`），只在匯入時寫；顯示圖 `<id>.png` 另存剝掉 chara／ccv3 的一份，換圖、刪圖、重構 Portrait 只動顯示圖，存角色圖時也剝卡文字。卡片介面只讀原卡檔，`.card.png` 讀出一般圖算 unreadable。匯入寫檔失敗整次回錯並回滾，回滾也清不乾淨時回報殘檔；刪角色逐檔清理、彙總殘檔位置；撤銷刪不乾淨不彈收據、請玩家再按一次。舊桌不遷移。規格、失敗語意與已知限制見 [plans/character-image-overwrites-card.md](../../plans/character-image-overwrites-card.md)。
