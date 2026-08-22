# no-cache-model-optout — 零命中的模型不走共線

**狀態：已立案。2026-08-22 與 Sol 兩輪討論挖出兩個坑並收斂出方向，四項待拍板未定，尚未動工。**

## 問題

共線（[api-shared-lane](api-shared-lane.md) 包 B）每輪多送約 1,500 tokens 的全角色名單，換取換角色時不必整包重算。對**真的沒有快取**的模型，這筆支出零回收。

**立案證據已被推翻（2026-08-22）**：`deepseek/deepseek-v4-pro-0813-free` 那 27 筆帳本行**全部缺 `cache_reporting` 欄**（原記「全部 reported」是錯的）。加欄之前的 api 行會把「沒回報」壓平成 `cached_tokens: 0`（見 [api-cache-visibility](api-cache-visibility.md)），所以那 27 個 0 **分不出是真的沒中還是量不到**——`usage-diag-non-claude` 修完之後它們一律歸 `unknown`。

也就是說，本案目前**沒有任何模型不支援快取的實測證據**。開工前的第一件事是重新立證：拿帶 `cache_reporting: "reported"` 的 eligible zero 累積出來，才談得上退回。

| 路徑 | 平均輸入 | 加 1,500 佔比 | 有無快取 |
|---|---|---|---|
| `deepseek-v4-pro-0813-free` | 4,411 | **+34.0%** | **未知**（27 筆量不到） |
| api `stealth/ox-alpha` | 7,927 | +18.9% | 有 |
| codex | 21,308 | +7.0% | 有 |
| grok | 68,692 | +2.2% | 有 |

deepseek 輸入最小，佔比因此最大。它是免費模型，代價不是錢而是延遲與 context 佔用。

**這張表是包 B 之前的數字**：帳本非 claude 路徑最後一筆停在 2026-08-21 23:14，deepseek 那 27 筆全落在同日 11:39–14:31，都早於包 B 上線。共線後究竟多花多少要等 api-shared-lane 的實機驗收，動工前把 +34% 當推估看待。deepseek 那一列連「有沒有快取」都還沒證實。

## 兩個坑（2026-08-22 挖出）

### 坑 1：退回的目標已經被刪掉了

包 B 把舊的單角色組裝器 `transport::assemble_messages` **整支移除**——`fn assemble_messages` 現在零命中，只剩 [lib.rs:2253](../../src-tauri/src/lib.rs)、[transport.rs:2](../../src-tauri/src/transport.rs)、[cli.rs:2](../../src-tauri/src/cli.rs) 三處過時註解還在引用它。

不必復活：改用 `cards=[本輪角色]` 呼叫同一支 `assemble_shared_messages`，函式內既有的 `cards.len() <= 1` 分支會自動把該角色私設提回 system。零新組裝路徑。

但**這不等於包 B 之前的行為**（Sol 指出）：舊路是本角台詞 `assistant`、他角 `user`＋名字，現版全部台詞 `assistant`＋名字前綴，世界書擺位也不同。正確的名字是「solo roster 模式」，不是「退回舊路」。

### 坑 2：退回之後不會自然自癒

原設想「判定窗口照常更新，供應商日後開快取會自然翻回來」——**不成立**。退回單角色後 system 每換一個角色就變一次，結構上不存在能命中的輪次，於是永遠量不到快取、永遠翻不回共線，卡死在退回狀態。要恢復就得主動試（見下方 probe）。

## 收斂的方向（與 Sol 2026-08-22）

1. **只在 `api` 路徑做自動退回，CLI 三條不做。** CLI 的 model 欄在沒有 tier 覆寫時會寫成 `(CLI 預設)`（帳本目前 0 筆，但 `model.unwrap_or("(CLI 預設)")` 確實產得出來），認不出模型就無法 per-model 判定；而 codex +7.0%、grok +2.2% 本來就實測有快取，不值得為它們解識別問題。key＝`(正規化 endpoint, 完整 model slug)`，endpoint 去掉 query 與憑證。
2. **N＝連續 3 次 eligible zero。** eligible＝shared 模式、同 system hash、距上輪在 TTL 內、`cache_reporting` 有回報、前綴長度過得了最低可快取門檻。任何 `cached_tokens > 0` 立刻清零重數；`absent` 永不進判定。
3. **恢復靠滾動 seed probe。** 冷卻到期時，下一個 eligible 輪送 shared 當 seed；下一輪同 endpoint／model／hash 且距 seed 完成 < 120 秒＝驗證輪。中了就恢復共線；`cached=0` 但 `created>0` 把該輪滾成新 seed 再驗一次；`cached=0` 且 `created=0` 退回 solo、重啟冷卻；hash 變了或超時不算失敗，直接成為新 seed。玩家每輪都隔超過 TTL 時維持 solo 是誠實結果，不偷發付費 synthetic probe。
4. **狀態放 process 內的 map，不讀 JSONL 做決策。** [usage_log.rs:249](../../src-tauri/src/usage_log.rs) 的落檔是 `let _ =`，寫入失敗被吞掉——它是 telemetry，不能當策略的唯一狀態來源。重啟後保守回 shared。包 B 之前那 27 筆 deepseek 是不可判讀的舊觀察，不得拿來初始化 miss streak。

## 待拍板三項

1. **solo 模式的 role 分配**：沿用共線那套（全 assistant＋名字前綴，維持一支組裝器），還是復活舊的 per-character（本角 assistant／他角 user，兩支組裝器）。沒快取的模型前綴穩定性零價值，理論上該挑品質好的那套——但「哪套品質好」在 API 路徑還沒實測，那正是 api-shared-lane 欠的驗收項。
2. **要不要讓玩家看見退回**：省成本，但換角色會變慢；額度分頁要不要標。
3. **冷卻週期多長**：probe 那兩輪多久試一次。

`usage-diag-non-claude` 已於 2026-08-22 結案，本案的 eligibility 判定可以直接接它建好的兩軸（`mode`／`cache`／`cache_reason`）與 `PromptShape` 那組唯讀 metadata，不必自己再造一份。

## 邊界

- 相依 api-shared-lane 包 B（已落地，實機驗收未做）。
- 與「單角色桌把私設留在 system」（api-shared-lane 結論 3）是**不同的分支條件**，勿混為一談：那條看角色數，這條看模型快取能力。
