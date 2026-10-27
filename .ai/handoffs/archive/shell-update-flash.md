> 結案 2026-10-27：Sol／Grok 驗收無必改，合進 main。

# shell-update-flash：卡片介面殼更新無閃白

Status: done

## 現況
- 殼更新改雙緩衝：新殼墊在舊殼下面載，殼內墊片 postMessage ready 才翻面，等待期擋操作、逾時 1500 ms 強制翻；首開與換桌／還原／切卡直接繪。殼 sandbox 維持 allow-scripts（無 allow-same-origin）。
- 作者裁決〔作者裁決 2026-10-27〕：先校準量測、量出現況；不看門檻直接做雙緩衝；接管段排第③。
- 量測方案、雙緩衝與量測結果：[plans/shell-update-flash.md](../../plans/shell-update-flash.md)。雙緩衝已實作（`src/features/card-interface/shell-frames/`），verify 綠、三步實測過；估計現況每回合約 80–100 ms 空白，改後 260 張快照未見空白。

## 待驗（排在實測佇列梯 1 第 4f／4g／4h）
- 殼內按鈕送行動、設定重開仍在、等待期舊殼行動被拒；等待期原生 Tab 進不了 iframe；離線 1500 ms 強制翻面。
