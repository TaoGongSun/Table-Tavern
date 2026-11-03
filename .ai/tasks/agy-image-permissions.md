# agy-image-permissions — agy 生圖收緊執行權限

## Summary
agy 生圖帶 `--dangerously-skip-permissions`，卡作者文字（VN 卡外觀規格、畫風、`public_md`、`world.md`）進生圖提示詞時模型可執行工具不受限；惡意卡理論上能誘導它在玩家電腦上執行指令。Codex 來源有沙箱、API 來源只回圖，不受影響。VN 前就存在的既有曝險（出處：[包 9b 方案 §5](../plans/vn-mode-pkg9b.md)）。作者裁決另立案收緊〔作者裁決 2026-11-02〕。

## Next action
先問 agy 自己有沒有只授權生圖工具、或限制可寫目錄的參數；沒有就評估改用沙箱執行或關掉 agy 生圖來源。碰 spawn／程序控制，不論案別做方案送審。
