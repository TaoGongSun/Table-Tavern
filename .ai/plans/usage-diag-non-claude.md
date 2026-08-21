# usage-diag-non-claude — claude 以外的路徑全部只標「單發」

**狀態：已立案，僅完成根因定位。完整盤點與設計留給新對話。**

## 問題

額度分頁對 API／codex／grok／agy 一律顯示「單發」，包含命中率 81.3%、91.8% 的那些輪次。玩家看到的文案是「整包重新送出」，與旁邊的高命中率直接矛盾。

## 根因（一行）

[usage_log.rs:88](../../src-tauri/src/usage_log.rs) 的 `diagnose`：

```rust
fn diagnose(lane: Option<&LaneContext>, usage: &PromptCacheUsage) -> Diag {
    let Some(lane) = lane else {
        return Diag::Single;      // ← 還沒看 usage 就回傳了
    };
    ...
}
```

`lane` 只有 claude 續聊路徑會傳。其餘四條路在**碰到快取資料之前**就短路成 `Single`，於是整套診斷詞彙（`ok`／`expired`／`prefix-broken`／`no-cache`／`cache-skipped`）對它們**全部不可達**。

`Single` 因此身兼二職：它描述的是「呼叫模式＝不走續聊」，卻被當成快取診斷顯示。這是兩個正交的軸：

| 軸 | claude lane | 其他四條 |
|---|---|---|
| 呼叫模式（續聊／單發） | 有 | 有 |
| 快取狀態（命中／過期／前綴斷／不支援） | 有 | **沒有** |

## 影響範圍（實測）

帳本 638 筆中，`diag=single` 且命中率 >50% 的有 **146 筆**——這些輪次都在對玩家說反話。

各路徑的 diag 分布：`agy` 23 筆全 single、`api` 31 筆全 single、`codex` 15 筆全 single、`grok` 4 筆全 single；`claude` 是唯一有完整診斷分布的（ok 229／warmup 65／expired 17／prefix-broken 8／ping 12／single 234／其他 4）。

注意 claude 自己也有 234 筆 single——換幕摘要與開桌生成本來就是單發，那些同樣吃得到快取，同樣被誤述。

## 已知可用的訊號（供設計參考，未驗證完整性）

- `cache_reporting`（reported／unreported）已獨立表達「看不看得見」，與 diag 不衝突。
- 三條路各自量到穩定的「供應商固定前綴」底線：api 64、codex 9,984、grok 約 48%。有底線就能區分「只中供應商前綴」與「我方內容也中」——那正是 `prefix-broken` 想表達的東西。

## 新對話要做的事

1. 把 `Diag` 的每個標籤逐一對照四條非 claude 路徑，判斷哪些可重用、哪些需要新的。
2. 決定「呼叫模式」與「快取狀態」要拆成兩個欄位，還是合併成一組新標籤。
3. 十語系文案同步改寫；現行 `usageWhySingle` 十語全部寫著「整包重新送出／the whole bundle is sent again」，保留事實但要補上「不代表沒命中」。

## 邊界

只動診斷與顯示，不動組裝與傳輸；與 api-shared-lane 無相依，可並行。
