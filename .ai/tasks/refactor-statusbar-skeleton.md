# refactor-statusbar-skeleton — 狀態欄型的卡也產骨架

## 要做的事
重構判 playable: no（狀態欄型）的卡也產骨架：app 用狀態樹把原卡狀態區塊的格式填好，再交給原卡畫面顯示。模型只回報變動，不必每回合重寫整份狀態區塊，符合重構「固定格式交給 app 接管、省模型輸出」的目的〔作者裁決 2026-10-02〕。

## 背景
「不接管時保留介面來源條目」已作廢：那樣模型每回合都得重寫整份狀態區塊〔作者裁決 2026-10-02〕。[card-chat-messages-shim](../plans/card-chat-messages-shim.md) 已讓這類桌改回原卡畫面，但只有開場白那一樓有值；後續回合有值要靠本案。

## 排程
排在 card-chat-messages-shim 之後。
