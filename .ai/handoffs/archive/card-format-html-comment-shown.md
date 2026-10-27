> 結案 2026-10-27：三方（Opus、Sol、Grok）驗收通過，squash 進 main。

# card-format-html-comment-shown — 卡片格式裡的 HTML 註解被照抄後顯示成純文字

## Summary
卡片格式要求的 `<!-- 隐藏记忆栏 -->` 這類 HTML 註解，模型照抄進回覆後，畫面把它當純文字顯示；SillyTavern 會把 HTML 註解藏起來。gm-no-state-fence-worldbook-table 實測發現：舊中性版也會（現況組 2/5），新旁白措辭讓模型更常照抄卡片格式，測試通道 WestFantasy_NOUI-safe 4/4 都出現。

## 查明（2026-10-27）
- ST：showdown 原樣放行 `<!-- -->`，`messageFormatting` 末段 DOMPurify 因 `#comment` 不在白名單把註解節點丟掉（消毒階段）；`chat[].mes` 不動，送模型的上下文保留註解。code span／fence 裡的照顯示，未閉合的 `<!--` 跳脫成文字照顯示。`encode_tags`（預設關）開了才會顯示。
- 我們：`src/shared/ui/story-markdown.ts` 的 marked `html` renderer 把所有原始 HTML token `escapeHtml` 成可見文字，註解沒機會進 DOMPurify。桌面、網頁所有故事顯示面都走這支；後端儲存與提示詞不剝註解（同 ST）。
- 重現：卡片記憶欄片段（整段包在註解裡）ST 等價管線只剩正文與選項，我們把註解與記憶內容全顯示。探針 scratchpad `htmlc-probe/`。

## 做法（Opus、Sol、Grok 共識：選項 A）
- `story-markdown.ts` 的 `html` renderer 先剝已閉合註解 `/<!--(?:-?>|[\s\S]*?--!?>)/g`（含 `--!>` 結尾，遇第一個結尾就收），剩下照舊跳脫；沒閉合的不剝。儲存與上下文不動。
- 已知限制〔模型判斷·未裁決，Sol、Grok 同意列為本案範圍外〕：同一行接續跨行的註解（marked block 規則把第二個 `<!--` 收進前一個 token）；單獨一行內的 `前<!-- a --!>後文` 不成為 html token、照顯示；原始 HTML 屬性值裡的 `<!-- -->` 文字也會被剝。卡片記憶欄是獨立成行的註解，不受影響。
- 相鄰未納入：其他原始 HTML 標籤仍跳脫成文字（ST 會渲染白名單標籤），待主線裁決。

## 驗收方向
回覆裡的 HTML 註解不顯示在畫面上，行為與 ST 一致。
