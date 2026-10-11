> 結案 2026-10-11：已進 main，三方驗收通過。

# tail-worldbook-erase — 尾段世界書累積到門檻再整線重開

規格與結果：[plans/tail-worldbook-erase.md](../../plans/tail-worldbook-erase.md)。

- GM 續聊線與角色共線的尾段世界書回合後留在歷史吃快取；份數到 8，或至少 4 份且佔該路徑容量 15%，就以 `tail-worldbook` 整線重開壓平（Claude／Grok／Agy 一致，守 P2）。
- 計數存 `lanes.json`（`tail_stale`），舊檔缺欄當未知重開一次；帳本記 stale 四欄；Claude 角色線到門檻重開也刪舊 session 檔。
- 實機：haiku GM 十輪第 9 輪份數上限重開；自建 8k 條目第 5 輪比例重開。
