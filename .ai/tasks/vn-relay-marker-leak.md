# vn-relay-marker-leak — VN 桌「下一位：X」接力行漏進對話框

## Summary
重播桌 `TestCards/sunsmell-replay.ttvn` 播放時，GM 回合尾的接力行「下一位：老婆婆」直接當成一格文字顯示在對話框（vn-ui-polish 審視輪截圖 32）。解析器（`chat_turn/vn_script.rs`／`gm.rs` 剝「下一位」行）沒剝到模型這種寫法。一般玩家每回合都可能遇到。

## Next action
拿該包的原始事件文字對 `vn_script.rs` 的剝除規則，補中文「下一位：」與其變體（全形冒號、前後空白、放在選項區塊之後）；單元測試用該原文當夾具。小案估核心 ≤40 行〔模型判斷·未裁決〕。
