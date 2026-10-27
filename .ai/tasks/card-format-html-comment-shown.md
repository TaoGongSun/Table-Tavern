# card-format-html-comment-shown — 卡片格式裡的 HTML 註解被照抄後顯示成純文字

## Summary
卡片格式要求的 `<!-- 隐藏记忆栏 -->` 這類 HTML 註解，模型照抄進回覆後，畫面把它當純文字顯示；SillyTavern 會把 HTML 註解藏起來。gm-no-state-fence-worldbook-table 實測發現：舊中性版也會（現況組 2/5），新旁白措辭讓模型更常照抄卡片格式，測試通道 WestFantasy_NOUI-safe 4/4 都出現。

## 已知線索
- 出處：gm-no-state-fence-worldbook-table 交接檔與 scratchpad/fence-tools/shots/noui-safe-turn4.png（暫存，可能已清）。
- 照酒館行為：顯示時隱藏 HTML 註解；要確認 ST 是在 markdown 渲染階段還是消毒階段處理，以及送回模型的上下文要不要保留。

## Next action
未排程；開工先重現，查 ST 對 HTML 註解的顯示與上下文處理。

## 驗收方向
回覆裡的 HTML 註解不顯示在畫面上，行為與 ST 一致。
