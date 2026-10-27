# story-html-style-class — 訊息支援 <style> 區塊與 class（狀態欄卡）

## Summary
blackboard、Status_block、StatusData 這類靠 `<style>`＋class 排版的狀態欄卡，在 story-html-whitelist 後內容都在但外觀是素的。ST 會把 `<style>` 解析、選擇器加 `.mes_text` 前綴限縮，class 加 `custom-` 前綴。

## 已知線索
- 需要 CSS 解析器（ST 用 css、Marinara 用 postcss）＝新依賴。
- 網頁版要改走 CSSOM insertRule 才過得了 CSP。
- 與 story-html-style-attr 共用 CSS 消毒，可一起做。

## Next action
未排程；等 story-html-whitelist 結案後評估。

## 驗收方向
TestCards 的狀態欄卡外觀接近 ST；樣式不外溢到 app 介面。
