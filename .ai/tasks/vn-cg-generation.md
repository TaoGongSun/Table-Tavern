# Task
Task-ID: vn-cg-generation
Title: VN 模式 CG 即時生成：外接吃到飽生圖訂閱（NAI 類）＋提示詞規範
Status: todo
Created: 2026-08-06T10:47:00+08:00
Updated: 2026-08-07T13:32:15+08:00

## Summary
2026-08-06 起意（源於 ST 玩家用 NovelAI 在酒館內生圖的做法）：長期可考慮，主要動機是 **VN 模式的 CG**，不是角色圖。

**價值主張**：現有 VN 型卡（訓帝卡）靠作者預先畫好 101 張圖掛免費圖床，圖床倒站即全滅（見 [interface-card-panel](interface-card-panel.md)）。若 CG 能在劇情節點即時生成，作者不必準備圖庫、也沒有圖床死穴——這是原生 VN 皮相對雲端流派的差異化。

**為何需要另一條生圖路**：現有 `transport::generate_image` 走 OpenRouter `/images`（`google/gemini-3.1-flash-image`）按張計費，撐不起「每個劇情節點一張 CG」的用量。吃到飽訂閱才撐得起——NAI Opus 約 $25/月，一般尺寸不扣 Anlas＝實質無限張。訂閱是**玩家自己的**（自填 token），app 不代購不代付。

**兩塊工程，難的是後者**：
1. **接口**：NAI 走自有端點、非 OpenAI 相容，回傳是壓縮檔要解，等於新增一條傳輸路徑並把生圖來源抽象化（現在寫死拿 `b64_json`，見 [transport.rs:1788](../../src-tauri/src/transport.rs)）。
2. **提示詞規範**：NAI 吃 Danbooru 標籤，不吃長篇散文；我們現在是把角色介紹整段中文丟給通用多模態模型自己讀懂（[lib.rs:1338](../../src-tauri/src/lib.rs)），那套餵 NAI 會出爛圖。要多一層「劇情／角色 → 標籤」轉換，並決定誰來轉（玩家的聊天模型轉＝品質好但每張多燒一次呼叫，ST 就是這樣做；內建模板硬轉＝零成本但品質不穩）。另需內建一套負面提示（`uc`），不給的話 NAI 出圖品質明顯掉。

**CG 與角色圖的差別**（影響設計）：角色立繪要跨場景一致（同角色反覆出現，靠 seed＋角色專屬前綴鎖），CG 是一次性場景大圖（構圖隨劇情走，不需鎖）。兩者共用接口、不共用提示詞策略。

## Next action
- 前置已立案：[vn-mode](vn-mode.md)（2026-08-07），本任務是它的 v3 分期；CG 一致性三道保險（參考圖＋外觀規格塊＋重骰把關）拍板在 vn-mode。
- 最省的可行性驗證：拿一把 NAI token 直接打一發看出圖品質與回應格式，併入 vn-mode 開工前置實測 c。

## Constraints
- 訂閱與 token 都是玩家自己的，app 不代購不代付、不碰計費（同 [sponsor-features](sponsor-features.md) 的 BYOK 原則）。
- 實作前必須照 NAI 官方文件核對 v4.5 當時的端點、參數結構與回傳格式，勿照舊記憶寫。
- 現有「模型拒畫」分流（`NO_IMAGE`／`REFUSED` 暗號）在此路徑用不到，錯誤處理要另想。
