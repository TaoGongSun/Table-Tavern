# Task
Task-ID: refactor-mode-split
Title: 重構雙軌定向：介面優先 vs 角色優先（兩段式選擇＋模式專屬解析）
Status: in-progress

## Summary
卡片介面與拆角色本質衝突（拆出的角色一開口就掉出介面），重構改成二選一雙軌：介面優先＝不拆角色、走接管軌、玩法與原卡完全相同（優先項目）；角色優先＝介面產物一律不建不顯示（含逐訊息狀態欄）、app 多角色對話接手。定向（2026-08-14 五項拍板）：本機三態偵測（none 直通角色線／supported 一律問玩家／unsupported 擋下）；判官兩段式——第一段帶全卡只出 RECOMMEND＋EVIDENCE 兩行，玩家選完第二段承快取（同檔位、獨立短命 session、MODE 回聲＋run id 指紋）出模式專屬小抄；選錯＝取消重跑。Sol 第 1 輪覆核（2026-08-14）抓到主洞已併入：模式必須持久化、角色優先明確停用介面 fallback（現行無殼時會退回原卡 regex 渲染，抽驗確認）。

設計底稿見 [plans/refactor-mode-split.md](../plans/refactor-mode-split.md)；2026-08-14 全數拍板（含對話框文案稿、初判失敗預設介面優先），規格齊備。

## Next action
- 包 1–3 全部實作完成（2026-08-14 主線直寫，三 commit 25ca9cd／8c2ce17／a711287，cargo 490／vitest 134／build／i18n 十語系全綠）；剩包 4 實機驗收矩陣歸使用者實跑：WestFantsy／bcd368／Transfur／NorthHall／TrainEmperor（該被擋）＋同卡連跑三次，清單見交接檔 Remaining。

## Constraints
- 有介面卡一律問玩家，只有無介面卡免問直接角色線；unsupported（雲端載入器型）擋下不進二選一；選項必須寫明各自會發生什麼。
- 兩段判官同檔位同 lane、獨立短命 session（不借遊玩 GM lane）、卡片資料排共用前綴；resume 失敗降級重送全卡。
- 模式持久化進產物與桌面狀態；角色優先明確停用卡片介面 fallback；匯出／匯入保存 mode；稽核 mode-aware。
- 穩定性驗收＝同卡連跑三次皆可運行（次數使用者實測時視額度調整）＋跨卡型矩陣。
- 實測供應商只認 claude／codex：grok／agy 兩條 lane 無隔離旗標，會吃使用者全域設定與跨會話記憶（見 refactor-survey-spans 交接檔環境陷阱）。
