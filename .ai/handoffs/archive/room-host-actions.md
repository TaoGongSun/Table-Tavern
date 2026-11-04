> 結案 2026-11-04：包 1–3 進 main（一包一筆），Sol 驗收各包 PASS、常識審查併入。

# room-host-actions — 多人房間房主操作

Status: done

案別：大案（立案估核心約 620）。規格、分包、驗收、施工觀察都在 [plans/room-host-actions.md](../../plans/room-host-actions.md)。

## 成品
- 包 1 房主 GM 鈕（旁白／推進／請發言、「讓 X 接話」）。
- 包 2 收回／重生上一回合（只收一層、重生可換路線模型、被收回的行動原文只給本人帶回或丟掉）。
- 包 3 重開補送：關房後單人玩的段落，重開時補送到手機（同步閘門、`story.sync`／`story.truncated`、跨幕比照換幕、前後分隔）；「用這張桌開房」玩過的桌也能開，種子在第一個玩出來的段落前標「房主之前自己玩的」。

## 部署注意（作者部署時看這段）
房間伺服器必須跟這版桌面版一起部署，舊伺服器不認得本案的協定增補：
- 執行端錯誤種類 `model_unavailable`；`host.gm`、`round.undo`、`job.undo`、`round.undone`、`vol.returned`、`action.returned_dismiss { draft }`。
- 包 3 的 `story.sync`、`story.truncated`、執行端 `hello.paused`、`ExecutorView.syncing`、說話者標記 `solo_before`／`solo_paused`／`room_reopened`：舊伺服器收到會回 4400，桌面版走 `E_SERVER_OUTDATED` 直接停。

## 已知限制〔模型判斷·未裁決〕
- 只收一層：收回或重生之後，再往前一回合收不了。
- 收回改字後，桌上在場名單（`set_party`）要到下一個派工才歸位。
- 往回翻頁跨 200 筆邊界時，已收回或補送撤掉的段落可能被翻回來。
- 桌面版房間畫面的故事區很擠，另立版面整理案。
- 「讓 X 接話」與「朋友的卡在單人段被 GM 代演」測試通道沒實機看到，排在實測佇列。
- 手機把 GM 正文的 Markdown 圍欄照原文顯示（手機純文字渲染，既有問題，已回報主線另排）。
