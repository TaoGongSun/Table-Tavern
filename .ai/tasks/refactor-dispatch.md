# Task
Task-ID: refactor-dispatch
Title: AI 重構提速省費：展開並行（時間）＋展開下放檔位（費用）＋取消真停
Status: in_progress
Created: 2026-08-08T03:35:00+08:00
Updated: 2026-08-11T12:00:00+08:00

## Summary
重構管線（survey → 人物展開 → PLAN 重寫 → 介面展開）現況全走 GM 檔、序列執行、取消殺不掉在途呼叫。三個改動，全傳輸通用（CLI 與 API 直連皆受益）：

1. **並行展開省時間**：survey 先行建快取後，展開類呼叫改有界並行（上限 4），分鐘級壓到一分鐘上下。
2. **分檔位省費用**：survey 留 GM 檔（認人／規劃是智力核心）；三種展開下放 balanced（−40%）；API 未設 balanced 模型退 GM 檔。
3. **取消真停**（2026-08-11 Dark Wolf 實測補立）：取消＝中止全部在途（CLI 殺子程序、API 斷流），已完成照樣進結果卡；順帶修 Cmd-Q 孤兒子程序。

「模型內部自派子代理」評估後不採：重建脈絡 token 更多、拆掉逐條解析與取消保底、不可信卡片內容會接觸有工具的代理、僅 claude CLI 支援。

2026-08-11 拓撲拍板 A 後包 1–3 當日實作完成（三 commit，cargo 442／vitest 94／build／i18n 全綠）；剩包 4 實機驗收。

## Next action
- 使用者 `claude login`（CLI OAuth 已過期）→ 拍板 P7 基線時序 a/b → 照[交接檔](../handoffs/refactor-dispatch.md) P1–P8 實測，全過結案；P2／P7 可與 [ai-card-refactor](ai-card-refactor.md) B 段真跑合併省額度。

## Constraints
- survey／expand 共用 system 逐位元組相同的快取紅線照舊，不碰 system 組裝。
- 下放檔位後的展開品質須與實測基線 A/B 比對後才定案；機制重寫是最敏感點。
- knownFields 欄位命名單一權威不得犧牲。
