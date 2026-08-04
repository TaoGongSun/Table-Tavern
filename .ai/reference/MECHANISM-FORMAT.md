# 機制格式規範（v1）

狀態欄的資料標準：一棵**狀態樹**（值）＋一份**機制**（規則）。ST 各家協定匯入時被翻譯成它，
本 app 自己的模板也直接用它寫。判斷邏輯一律資料驅動——程式讀規則決定怎麼收更新，不寫死欄位名。

翻譯判準一條：卡片有硬標記（`<status…>`、`<UpdateVariable>`、`[initvar]` 這類）就走本地解析；
沒硬標記的長尾留給「AI 卡重構」按鈕，產物一律人審。

## 狀態樹

葉子存值（字串）、分支存子節點，深度不限。序列化就是自然的 JSON：

```json
{ "World": { "Time": "清晨", "侵略度": "35" }, "亞瑟": { "好感": "40", "HP": "480/500" } }
```

- 存在每則對話事件的快照裡（`transcript/<scene>.jsonl` 的 `state.tree`），收回上一句＝整棵樹回到那一刻。
- 桌上的目前值是最後一則事件的快照（`state.json` 的 `state.tree`），兩者恆等。
- 路徑寫法：點分字串 `World.侵略度`。欄位名本身含 `.` 者對不上規則，對不上就是沒規則、走預設行為。

## 機制

存在 `state.json` 的 `mechanism`，**不進逐則快照**——規則是這桌的設定，不隨對話變動。

```json
{ "version": 1, "rules": { "亞瑟.HP": { "kind": "pair", "min": 0, "update": "delta", "inject": "turn", "branch": "亞瑟" } } }
```

### 欄位規則

| 欄位 | 意義 |
|---|---|
| `kind` | 型別，見下表 |
| `min`／`max` | 數值上下限，夾住模型給的更新；抽不到就不填、不夾 |
| `update` | 怎麼收更新：`delta`（只收增減量）／`replace`（照收新值）／`local`（app 自己算，模型唯讀）／`reject`（全拒） |
| `inject` | 何時進模型上下文：`snapshot`（長文字欄，進凍結快照）／`turn`（動態數值，走回合尾）／`rare`（純記錄，提及才送） |
| `branch` | 分支歸屬（樹的第一層節點名）。角色卡匯出只帶走自己那支 |

型別與預設值（建規則沒特別指定時就是這組）：

| `kind` | 說明 | 預設 `update` | 預設 `inject` |
|---|---|---|---|
| `number` | 純數字 | delta | turn |
| `pair` | 現值/上限對（`"500/500"`） | delta | turn |
| `roll` | 骰值欄，app 每回合本地重擲 | local | turn |
| `text` | 字串 | replace | snapshot |
| `list` | 清單／字典 | replace | turn |
| `counter` | 計數器，允許大跳（時間跳躍） | delta | turn |
| `read_only` | 唯讀 | reject | rare |
| `derived` | 衍生值：schema 預留，未實作 | reject | rare |

## 匯出匯入

角色卡（SillyTavern chara_card_v2）的 `extensions.table_tavern`：

```json
{ "version": 1, "rules": { "HP": { … } }, "initial": { "HP": "500/500" } }
```

`rules` 的 key 是**去掉分支前綴**的相對路徑，`initial` 是這張卡那一支的初始子樹。
匯入時補回 `<卡名>.` 前綴、`branch` 填卡名，初始子樹掛進 `state.tree.<卡名>`。
解析失敗一律略過、不擋匯入——一張卡壞掉不能害整批匯不進來。

## 待補（隨包完成後補進本檔）

- 統一協定聲明（面向模型的那半：數值 delta-only、現值/上限對下 delta、骰值唯讀）＋拒收記帳 — 包 4。
- 觸發表 schema（數值區間／字串包含／旗標／計數器門檻，可 AND；區間型與一次性事件兩種語意）— 包 7。
