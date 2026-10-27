# zod 版 MVU 卡的更新語意改以 mvu_zod 為基準

Status: in-progress（分支 mvu-zod-semantics）

## Summary
mvu-array-index-path 驗收時查到：zod 版 MVU 卡（registerMvuSchema，TestCards 帶 [initvar] 的四張全是）實際由 StageDog/tavern_resource 的 util/mvu_zod.ts 在 `mag_command_parsed_for_zod` 自行 applyCommand 並清空指令，MagVarUpdate 的 applySet／applyInsert／applyDelete／applyAdd 一條都收不到。本地 mvu-replace-numeric、mechanism-error-feedback、mvu-array-index-path 都以 MagVarUpdate 為基準。mvu_zod 差異：set 直接 `_.set`（不做 VWD 第 0 項、不轉 Number、路徑不存在照建）；add 只收數字、無 toPrecision(12)、無 VWD；insert 容器 nil 先試 {} 再 []、頂層不查父路徑；delete 陣列用 pullAt；move 會搬（delete 再 set）；`_` 開頭路徑靜默忽略；每條指令最後過卡片 zod schema（safeParse，含轉型、夾值、預設值），不通過整條不套用。

## 拍板（2026-10-27，Sol／Grok 兩輪討論後）
- zod 卡照上游 mvu_zod 行為做，以真 zod 執行卡片自帶 schema，公開前納入〔作者裁決 2026-10-27，修訂 2026-10-12「公開前維持現行基準」〕。
- 卡片自帶的變數 schema 准在隔離環境執行（無網路、無檔案存取、有時間與記憶體上限）；其餘卡片腳本照舊不執行〔作者裁決 2026-10-27〕。
- mvu-replace-numeric 的〔作者裁決 2026-10-04：不夾值、轉不成數字寫 null〕縮小為只管沒重構、非 zod 的卡；zod 卡照上游：夾值、轉不成數字整條拒收並保留舊值；重構桌照現行規則〔作者裁決 2026-10-27〕。
- 桌面與網頁共用同一份 MVU 引擎（不在 Rust 另寫第三份）。宿主先短驗證再定：先抽出無 DOM 的共用核心，再實測 QuickJS 能否跑 mathjs、zod 並守住記憶體上限；能就桌面用 QuickJS，不能就用 Worker 加可驗證的隔離方案〔作者裁決 2026-10-27〕。
- 提交契約（兩位審查方共識）：Rust 固定輸入→鎖外呼叫引擎→短鎖內重驗回合／幕／世代／輸入版本才寫；引擎回傳完整變數表＋逐條結果＋變動，Rust 不再二次套用原指令；帳本由逐條結果映射；開場白同一路徑；收回讀已存快照、不重跑 transform；單條 schema 拒收保留同批其他成功指令，整次執行失敗整批不提交並回饋；後處理（重擲、觸發、衍生）對 zod 已驗資料的規則要先明定。
- mvu-command-syntax、mvu-meta-schema 改成「接上共用引擎」，排在共用引擎之後；實卡缺，用乾淨測試卡＋DongeonMaster initvar 去 zod 腳本的衍生測試卡（不算原生實卡驗收）。

## 規格
[plans/mvu-zod-semantics.md](../plans/mvu-zod-semantics.md)（宿主 QuickJS、分包 P1–P6、驗收要點）。

## Next action
P1 已驗收；P2（zod 迴圈、回傳契約與 schema 選擇規則定版）在分支完成、verify 12 步綠，等主線驗收；通過後做 P3。討論紀錄：Sol 串 01a17baf-eea3-7561-9b5a-91e70855970b、Grok 串 01a17baf-eacf-7651-b1b8-6a502d1e7400。
