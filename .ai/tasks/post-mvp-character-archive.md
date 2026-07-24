# Task
Task-ID: post-mvp-character-archive
Title: MVP 後：角色卡隱藏區（軟刪除）＋真刪除警告
Status: completed
Created: 2026-07-20T09:52:17.567635+08:00
Updated: 2026-07-24T22:10:00+08:00

## Summary
2026-07-20 與使用者拍板：長篇故事配角會越積越多，但直接刪除不可復原、日後再出場難處理。做三層：
1. 「收起」角色卡到隱藏區（側欄不顯示、GM 上下文與點名 roster 排除、手動點名選單排除；transcript 舊台詞照留不動）
2. 隱藏區可隨時把角色拉回在場
3. 只有隱藏區內可真刪除，需警告確認（明示不可復原）
實作方向待開工時定：角色卡 frontmatter 加 archived 旗標，或移到 world 內 archived/ 子目錄（後者對 list_characters 的過濾最直觀）。注意 assemble_gm_messages 與 suggest_instruction 的 roster 都要排除隱藏角色，否則 GM 會點名不在場的人。

## Next action
- 無。2026-07-24 使用者實測三條路徑（收起、還原、刪除確認框）全數通過，結案。

## Constraints
真刪除必須有警告且明示不可復原（防資料遺失）；隱藏不得改動 transcript 歷史；GM 與手動點名一律只見在場角色。
