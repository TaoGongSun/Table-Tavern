> 結案 2026-10-27：CardFormat 桌換幕不再誤藏主角色，已進 main，Sol／Grok 驗收無必改。已知限制見下。

# 沒重構的顯示腳本桌換幕後把原卡主角色自動隱藏

Status: done

## 定案
- 換幕結算：present 缺席且這桌沒被要求報 present（`data::requires_present_report`＝`transport::gm_turn_format` 回 Narration 以外）就不結算、旗標原樣保留。判定輸入與 `chat_assembly::gm_turn_instruction` 同一份（可用顯示腳本、接管、refactor_mode）。present 有值（含空字串）一律照常結算。
- `is_interface_takeover` 其他呼叫點（摘要禁標記、角色回合拒收、前端 GM 單線）不動〔模型判斷·未裁決〕。
- 舊桌不拉回：已被誤藏的角色由玩家手動拉回〔作者裁決 2026-10-27〕。

## 已知限制
- 主角色已被藏的顯示腳本桌：`read_card_interfaces` 略過 auto_hidden 的卡 → 判定掉回 Narration，之後換幕仍結算成隱藏，只能手動改旗標拉回。
- Narration 桌 GM 漏報 present 仍會把全程在場的卡判成沒出現（既有限制，見 `settle_card_visibility` 註解）。

## 驗證
- 單元測試 `missing_present_skips_settlement_only_where_present_is_not_requested`：接管、characters、plain、CardFormat（真顯示腳本）、regex_allowed=false（真腳本、無骨架）、CardFormat present=""、CardFormat present 命中配角，九列。

- 測試通道（2026-10-27）：自造乾淨 JSON 卡「莉亞」（一支啟用中的 placement 2 顯示腳本），card_interfaces 認得 1 支、非接管、present 缺席；追一則 GM 旁白後 advance_scene（claude CLI Haiku 5.5 一次）換到第 2 幕，莉亞 auto_hidden 仍 false、桌面主區照常顯示。
