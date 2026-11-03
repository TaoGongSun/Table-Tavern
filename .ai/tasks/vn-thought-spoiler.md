# vn-thought-spoiler — VN 角色線內心獨白洩漏私設

## Summary
VN 桌角色線可寫 `名字 (表情) thought: …` 內心獨白，播放器照常顯示給玩家；角色會把私設裡的秘密寫進獨白（包 9a 實測：老婆婆 thought 寫出「和也的孩子」），等於繞過劇透鎖與揭露時機提前說破。一般 VN 桌（任何有私設的角色）都會遇到。

## Next action
開工時查角色線規範（`transport/vn_format.rs` 的 `vn_character_rules`、`vn_assigned_line`）與 thought 格的定位，決定是提示詞約束（獨白只寫已揭露或角色當下會想的事）還是播放層處理。出處：[包 9a 方案 §5](../plans/vn-mode-pkg9.md)。中案估〔模型判斷·未裁決〕。
