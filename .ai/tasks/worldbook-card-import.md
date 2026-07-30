# Task
Task-ID: worldbook-card-import
Title: 世界書卡（PNG）匯入：世界書匯入鈕直接吃社群發佈的世界書卡
Status: in_progress
Created: 2026-07-30T21:30:00+08:00
Updated: 2026-07-30T21:30:00+08:00

## Summary
社群常把世界書包成 PNG 假角色卡發佈，先前只能走角色卡匯入、內容進不了桌子的世界書。2026-07-30 改為：世界設定 → 世界書「匯入」同時接受 .json 與 .png——PNG 先解出內嵌卡片 JSON，剝到 `character_book` 層再走既有匯入；一般世界書 JSON 路徑不變。以 TestCards/b3d7fd3600ab58d3252e8b38340390c4.png（17 條）煙霧驗證通過。

## Next action
- 使用者在 app 實測：開任一桌 → 世界設定 → 世界書「匯入」選該 PNG，確認 17 條進入清單且標題／常駐正確，回報後結案

## Constraints
匯入不自動開新桌（維持併入當前桌）；keys 為 null 的常駐條目須正常匯入
