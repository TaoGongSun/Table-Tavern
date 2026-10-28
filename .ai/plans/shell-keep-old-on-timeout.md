# shell-keep-old-on-timeout：新殼逾時或載入失敗時保留舊殼

雙緩衝現行設計見 [shell-update-flash](shell-update-flash.md)。本案只改「新殼沒載好」那一段。

## 拍板
- 逾時（1500 ms）：舊殼留在前面、新殼在背面續載、解鎖；新殼之後 ready 再翻〔作者裁決 2026-10-28〕。
- 新殼載入失敗：比照逾時保留舊殼，失敗的殼不翻上去〔作者裁決 2026-10-28〕。判定＝殼內墊片在捕獲階段記下外部 script 載入錯誤，ready 帶 `failed`；樣式表、字型、圖片不算（毛絨轉變桌引用的 zeoseven 字型站本機連不到，算進來的話每份殼都是失敗）〔模型判斷·未裁決〕。
- 卡住期間舊殼改的卡片設定不保存：storage 只收最新殼（鎖不鎖都一樣）〔作者裁決 2026-10-28〕。
- 解鎖後舊殼的文字輸入（input）可以送出〔作者裁決 2026-10-28〕。
- 保留舊殼的前提是舊殼本身回報過正常 ready（沒回報或載入失敗過的舊殼，逾時／失敗照舊翻到新殼）：不然一張引用了永久失效資源的卡，介面會永遠停在第一份殼〔模型判斷·未裁決〕。

## 狀態機（shell-frames-state.ts）
- 狀態加 `frontHealthy`：front 回報正常 ready（或由正常 ready 翻上來）為 true；新掛的 front、強制翻上來的 front 為 false，直到它自己回報 ready。
- `timeout`：沒 pending 回同一 state；front 健康→`lockedAt: null`、pending 留著；否則照舊翻到 pending。
- `ready {key, failed}`：pending 的 ready——失敗且 front 健康→同逾時保留；否則翻面（新 front 健康＝!failed）。front 的 ready→只更新 `frontHealthy`。被換掉的 pending 的 ready（key 不符）不動。
- 逾時後又來新殼：換掉背面那支、重新起算鎖定期（`lockedAt ?? now`）。
- 鎖定一律看 `lockedAt !== null`：擋板、`registry.locked`、焦點移擋板、front 的 inert／tabIndex；pending 永遠 inert＋tabIndex −1。
- 量測事件：逾時保留記 `timeout-keep`、失敗保留記 `failed-keep`、逾時翻面仍記 `forced`。

## 宿主
- 晚到翻面補推最新 chat／MVU：ready 的補推直接送給已核對過的那個 iframe 視窗（一般 ready 用 event.source、早到 ready 用登記當下的 contentWindow），不靠 registry——早到 ready 補報時 registry 還沒寫入這支。早到這條在 happy-dom 觸發不了，沒有單元測試。
- 舊殼不推狀態（殼內 HTML 是上一樓產的、token 也對不上）。
- 舊 token 的 MVU 寫入回 `mvu-settle` 拒絕 `{requestId, ok:false, error:"stale-shell"}`；舊 token 的 `mvu-eval`／`mvu-parse` 同樣回 `ok:false, error:"stale-shell"`，不讓殼端請求懸著。
- 換桌、還原、換卡（含多卡桌自動換卡）、收回回到 front、關面板：行為不變；shell-swap-flash-multicard 不受影響。

## 已知限制
- 卡住期間殼上數值停在上一回合（app 自己的狀態列是新的）。
- 卡住期間在舊殼改的卡片設定不保存。
- 卡住期間關開面板走首開直繪，會空白。
- 晚到翻面時舊殼內打到一半的字與焦點會丟。
- 第一次開桌就卡住（沒有舊殼）仍空白。
- 引用永久失效外部 script 的卡，舊殼不算載好過，逾時／失敗照舊翻面（等同改前）。
- 舊殼載好過之後，某支 script 中途永久失效（例：CDN 長時間中斷）：一路停在舊殼，直到恢復或關開面板（作者要的 CDN 中斷行為）。
- 只看 ready 之前的外部 script 載入錯誤：ready 之後才失敗的 async script、動態 `import()` 不判失敗。
- 樣式表、字型失敗不判失敗，可能翻出破版的殼。

## 驗收
- 單元：逾時 front 不變、lockedAt null、pending 仍在；逾時後新殼新 lockedAt；晚到 ready 翻面；被換掉的 ready 不翻；failed ready 保留舊殼；front 不健康時照舊翻。
- 元件：逾時後 registry 解鎖、擋板消失、pending inert、front 可操作；晚到 ready 翻面並推最新快照。
- 控制器：舊 token 寫入收到 stale-shell 拒絕。
- 測試通道（CDN 代理加「暫扣」「快速失敗」模式）：暫扣後放行截圖確認新殼畫出新數值；快速失敗保留舊殼；暫扣期間連送兩回合只剩舊殼＋最新 pending；暫扣期間關開面板記錄行為。verify 12 步。

## 驗收結果（2026-10-28）
- verify 12 步綠。
- 測試通道（毛絨轉變桌安全仿製卡，CDN 網址改走本機代理；本機假 GM 端點，零真模型）10/10：線上開介面基準；暫扣→`timeout-keep`、舊殼（第 14 回）在前可操作、新殼背面 inert／tabIndex −1、無擋板；暫扣中再送一回合→只剩舊殼＋最新 pending、再逾時仍保留；放行→`flip`、畫出最新數值（第 16 回）；503 與拒連各一次→`failed-keep` 保留舊殼、下一回合恢復正常翻面；暫扣中關開面板→首開直繪空白（已知限制），放行後畫出。
- 工具與截圖（不入庫）：scratchpad `keepold-tools/`（cdn-proxy.mjs 加 hold／fail 模式、rewrite_cdn.py、run.mjs、shots/）。
- 順帶發現：4h 用的 `vq/tools/rewrite_cdn.py` 把 CDN 網址改成「代理根目錄＋路徑」、少了主機名那段，代理抓不到，4h 那次連線上模式也是全部 502；`keepold-tools/rewrite_cdn.py` 已修。

## 業界做法與差距
- 新畫面備好前保留舊畫面（stale-while-revalidate：React `useTransition`、Turbo Drive、播放器停在最後一幀）。業界多配進度指示，本案不加（作者選改動小）。
- 根治是把 CDN 相依打包或本機快取；目前只內嵌 jQuery／lodash，其餘仍走 CDN。
