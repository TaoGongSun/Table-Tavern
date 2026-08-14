# Task
Task-ID: refactor-survey-spans
Title: 盤點四分類＋照搬零輸出：判官只出小抄（章＋分組＋命名權威），乾淨拆零呼叫
Status: in-progress

## Summary
2026-08-11 立案，同日 orc-cave 實測後升格為重構主案（使用者裁決）：**中型卡重構總時長必須壓進 5 分鐘內，達不到＝重構按鈕存廢重議**（現況實測 ~24 分）。

實測病灶（prompt-cache.jsonl 實證＋畫面確認）：
- 展開 17 筆共 ~82k output tokens，條目重寫→介面序列鏈段獨佔 ~19 分；其中大半是把純設定（豺狼人／深藍狼／巨魔等）整篇重寫——這些內容照搬即可，根本不該輸出。
- 逐日機制「巴古克與古茲卡入侵劇情線」反而未接管。此誤判發生在 GM 檔上＝判準規格問題，換檔位救不了。

規格細節（設計（使用者定形＋2026-08-11 結構強化））見 [plans/refactor-survey-spans.md](../plans/refactor-survey-spans.md)。

## Next action
- **T4 通用 2026-08-14 收工**：①取消在途＋Cmd-Q 無孤兒過（並行取消殺得乾淨、零孤兒、未完成不計費）、③舊產物相容過、④十語系面板骨架過、②API 退 GM 檔＝單元測試綠但 CLI 模式測不到，實機延後到哪天真用 API 模式時看 jsonl lane。refactor-dispatch 的 P4–P6 隨 ① 綠、P8 同 ② 延後。
- 同輪抓到並修好一個洞：**取消後仍彈「重構完成」半成品面板**（缺件不缺畫面，可直接套用）。改成標題「已取消（部分產出）」＋紅字說明＋主按鈕換「不要」；tsc／vitest 130／十語系／build 全綠，**未 commit、待實機看畫面**。
- T1–T3（卡片盤點品質）仍擱置到重構按鈕做完（refactor-mode-split 落地＋介面接管收尾）後補測，本案屆時才結案。

## Constraints
- 新格式規格放 survey 的 user 訊息端；survey／expand 共用 system 逐位元組相同的快取紅線零觸碰。
- knownFields 單一權威改由判官小抄一次頒布（並行拓撲 A 的鏈上累積語意由本案取代）。
- 驗收標準：orc-cave 總時長 <5 分＋照搬條目 byte 級不變＋入侵劇情線接管且可跑＋涵蓋與機制守恆稽核綠＋NorthHall 八角色分組正確、成品每人一張完整卡不碎裂＋淘汰清單可逐項復原。
- **供應商隔離**：claude 帶 `--safe-mode`（實機確認命令列有掛）、codex 帶 `--ignore-user-config`＋`--ephemeral`；grok／agy 兩條 lane 無等價旗標（cli.rs `grok_args`／`agy_args`），會吃使用者全域設定與跨會話記憶，症狀是默默產出爛盤點而非報錯——重構實測只認 claude／codex，補隔離延後到重構按鈕做完。
- [refactor-dispatch](refactor-dispatch.md) 剩餘驗收（P4–P6／P8）在本案完成後合併驗。
