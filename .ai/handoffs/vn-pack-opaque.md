# vn-pack-opaque — 作品包不透明容器＋音效進包

## Status
已合 main（2026-11-07）、分支已刪；Sol 方案審 1 輪＋驗收 1 輪皆無必改、verify 17 步綠；等作者實機（實測佇列梯 3）。方案與結果：[plans/vn-pack-opaque.md](../plans/vn-pack-opaque.md)。

## Summary
作者 2026-11-07 裁決：`.ttvn` 改成一般人打不開的容器（固定檔頭＋對 zip 串流做簡單 keystream 混淆，雙擊只得「未知格式」；特地破解的不管），盡到 Freesound API 條款 4(a)「Content 只能從 Application 取得」的合理義務後，音效（CC0／CC BY）隨作品包帶走並進感謝名單。

- 匯出：`vn_work/export.rs` 寫入層包混淆；不再過濾 `sfx-*`，`audio.json` 的 sfx 筆一起帶。
- 匯入：認檔頭，舊明文 zip 照收；`vn_audio::import_entries` 放行 kind `sfx`；`LIMITS` 檔數照算。
- 只解「授權可散布、平台條款要 app 內取用」這類素材；授權綁帳號的（ElevenLabs 免費層等）不因此可用。匯入後 mp3 仍明文落在對方資料夾，門檻只到一般人。
- Freesound 條款 1(b)「中間副本用完即刪」的灰色地帶與本案無關，不處理。

案別：中案（估 150–170，實作核心約 270）；匯入契約是信任邊界，方案送 Sol 一審。前置 [vn-credits](vn-credits.md)（名單讀同一份 audio.json，音效進包後自動列入）。

## Next action
作者實機：匯出 `.ttvn` 後雙擊或解壓軟體打不開；匯入新包的重播桌有音效、「音樂」分頁列音效作者；舊明文包仍可匯入。
