# api-shared-lane — API 路徑改走 chars 共線

**狀態：查證完成，設計未拍板。開工前先跟 Sol 討論文末四題。**

## 問題

claude 路徑早已做過「全角色共用一條線」（prompt-cache-optimization 包 2），API／codex／grok 路徑沒有——被 [lib.rs:2201](../../src-tauri/src/lib.rs) 一行 `if chat_transport(&config) == "claude"` 擋在外面：

```rust
if chat_transport(&config) == "claude" {
    let frozen = transport::chars_lane_system(&cards, ...);  // 全部角色卡，共用凍結 system
    return lanes::run_turn(...);
}
let messages = transport::assemble_messages(&card, ...);      // 單一角色，system 是該角色專屬
```

兩份 system 從第一個字就不同：`chars_lane_system` 是「你是這場多人桌上角色扮演的**扮演引擎**」＋全部公開角色卡；`assemble_messages` 是「你正在扮演**「加爾」**」＋加爾自己的公開與私有設定。前綴快取逐字比對，於是 API 路徑**每換一個角色就整包重算**。

## 實測證據（2026-08-21，OpenRouter `stealth/ox-alpha`）

| 時間 | 間隔 | 輸入 | 讀到快取 | 命中率 | 這輪是誰 |
|---|---|---|---|---|---|
| 14:55:32 | — | 6,581 | 64 | 1.0% | 角色 |
| 15:16:41 | 21 分 | 9,235 | 64 | 0.7% | GM |
| 15:18:49 | 2 分 | 7,720 | 64 | 0.8% | 加爾 |
| 15:21:55 | 3 分 | 8,174 | **7,680** | **94.0%** | 加爾（同角色連續） |

同角色連續能到 94%，其餘一律掉到 64——那 64 是 provider 的固定系統前綴，不是玩家的內容，**掉到 64 等於全滅**。另在測試前綴上量到 98.7%，證明「送一次同樣的請求」確實能刷新快取。

## 障礙：transcript 的 role 分配也是角色專屬的

[transport.rs](../../src-tauri/src/transport.rs) 把歷史事件轉成 messages 時：

```rust
TranscriptKind::Dialogue if event.speaker_id == card.id => ("assistant", …)   // 自己說的
TranscriptKind::Dialogue => ("user", format!("{}：{}", 名字, 內容))            // 別人說的
```

同一則對白，對加爾是 `assistant`、對雷恩是帶前綴的 `user`。**光換 system 不夠**，整段歷史的形狀仍因角色而異。

claude lane 的解法（見 lanes.rs 模組頂註解）：session 裡所有角色台詞一律 `assistant`，回合後補「X：」名字前綴讓下一個角色知道那句是誰說的（`run_turn` 的 `prefix` 參數）。API 路徑要共線就得跟著換這條轉換規則。

## API 無狀態的優勢：私設抹除機制不用做

`chars_lane_turn` 回傳的 `LaneTurn.confidential` 是給 claude lane 在回合後把注入的私設從 session 檔抹掉用的（原子寫＋回讀驗證那一整套）。API 路徑每輪從正典 transcript 重新組裝，上一輪注入的私設不會出現在下一輪的 messages 裡——**天然乾淨，這段完全不用實作**。

## 數字（以健身兔子那桌為準）

角色卡合計約 1,810 tokens，單張約 280。

| | system 大小 | 換角色時 |
|---|---|---|
| 現在 | 約 280 | 全額重建（實測 7,720） |
| 共線後 | 約 1,810 | 命中 95%+，只付新增部分 |

每次呼叫輸入多約 1,500 tokens（+19%），換來換角色不再從頭算；四角色輪流的節奏下第二輪就回本。

## 風險

1. **單角色桌沒有增益**——一桌一張是主流玩法，那種桌現在就已是一條線；共線只會多一段沒用的名單結構，還把私設從 system 挪到尾端。可能需要「角色數 ≥ 2 才走共線」的條件分支。
2. **codex／grok 走同一條路徑**會一起改變，而它們的快取行為沒量過（codex 僅 1 筆紀錄、grok 0 筆）。
3. **私設從 system 移到尾端影響遵循度**——system 指令權重通常高於 user 訊息。claude lane 已實跑驗證過，但 API 路徑的模型不同。
4. **Anthropic 系模型的 cache_control 斷點**掛在 system 與「最後一則 assistant」（`chat_request_body`）。role 分配改變後最後一則 assistant 的位置會變，要重新確認斷點仍落在穩定前綴上。

## 開工前要跟 Sol 討論的四題

1. **role 分配改成「全部 assistant ＋名字前綴」有什麼副作用？** 模型會不會把別人的台詞當成自己說過的、產生角色混淆？claude lane 跑得動，但 API 路徑接的是任意第三方模型。
2. **有沒有中間方案值得取？** 例如只換共用 system、不動 role 分配——那樣共用的前綴只有 system 那 1,810 tokens（約 20% 命中），但改動小得多。20% vs 95% 的差距值不值得那條 role 規則的風險？
3. **單角色桌要不要條件分支？** 分支會讓兩條組裝路徑長期並存；不分支則主流玩法小幅變差。
4. **共線與保溫（見 api-cache-visibility plan 第 6 點）誰先做？** 共線省的是「每次換角色」，保溫省的是「玩家發呆超過 TTL」。以實測節奏看換角色頻繁得多，但兩者不衝突。

## 保溫的既有結論（api-cache-visibility 查證所得，供第 4 題參考）

- OpenRouter **不支援** `max_tokens: 0` 零輸出預熱：實測 HTTP 200 但照常生成完整回覆，等於把 0 當成沒有上限。
- `max_tokens: 1` 有效（實測設 16 就是 16，`finish_reason: "length"`），所以保溫＝重送 messages ＋ `max_tokens: 1` ＋ `stream: false`，成本是「讀價 × 輸入 ＋ 輸出價 × 1」。
- API 無狀態，保溫呼叫不留痕跡，不需要 claude lane 那套 `truncate_ping`。
- 這條線的快取 TTL 落在 3 分鐘（活）與 21 分鐘（死）之間，尚未收斂。

## 工作量估計

約 110–140 行：新組裝函式 40、lib.rs 改呼叫 15、測試 60。測試核心＝**兩個不同角色組裝出的 messages，除了最後一則必須逐字相同**。
