# Task
Task-ID: refactor-survey-spans
Title: 盤點四分類＋照搬零輸出：判官只出小抄（章＋分組＋命名權威），乾淨拆零呼叫
Status: in-progress
Created: 2026-08-13T00:30:01.207335+08:00
Updated: 2026-08-13T00:30:01.207335+08:00

## Summary
2026-08-11 立案，同日 orc-cave 實測後升格為重構主案（使用者裁決）：**中型卡重構總時長必須壓進 5 分鐘內，達不到＝重構按鈕存廢重議**（現況實測 ~24 分）。

實測病灶（prompt-cache.jsonl 實證＋畫面確認）：
- 展開 17 筆共 ~82k output tokens，條目重寫→介面序列鏈段獨佔 ~19 分；其中大半是把純設定（豺狼人／深藍狼／巨魔等）整篇重寫——這些內容照搬即可，根本不該輸出。
- 逐日機制「巴古克與古茲卡入侵劇情線」反而未接管。此誤判發生在 GM 檔上＝判準規格問題，換檔位救不了。

規格細節（設計（使用者定形＋2026-08-11 結構強化））見 [plans/refactor-survey-spans.md](../plans/refactor-survey-spans.md)。

## Next action
- 2026-08-11 五包實作完成（cargo 470／vitest 108／build／i18n 全綠、五 commit），**新對話照交接檔實測清單 T1–T4 驗收**（orc-cave <5 分硬指標；過＝與 refactor-dispatch 一起結案）

## Constraints
- 新格式規格放 survey 的 user 訊息端；survey／expand 共用 system 逐位元組相同的快取紅線零觸碰。
- knownFields 單一權威改由判官小抄一次頒布（並行拓撲 A 的鏈上累積語意由本案取代）。
- 驗收標準：orc-cave 總時長 <5 分＋照搬條目 byte 級不變＋入侵劇情線接管且可跑＋涵蓋與機制守恆稽核綠＋NorthHall 八角色分組正確、成品每人一張完整卡不碎裂＋淘汰清單可逐項復原。
- [refactor-dispatch](refactor-dispatch.md) 剩餘驗收（P4–P6／P8）在本案完成後合併驗。
