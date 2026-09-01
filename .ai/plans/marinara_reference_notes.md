# Marinara Engine 競品研究與 Table Tavern 參考策略

> 狀態：持續更新中的設計／競品研究文件  
> 目的：記錄 Marinara Engine 值得學習、應避免、已重疊與仍可形成 Table Tavern 差異化的設計，並作為後續實作時的 clean-room reference notes。  
> 原則：可以研究 Marinara 的問題拆解、產品流程、資料模型與 edge cases；若 Table Tavern 希望保留未來閉源／商業化選項，不直接複製 Marinara 的 AGPL-3.0 原始碼或近似改寫其具體實作。

---

## 1. Executive Summary

Marinara Engine 已經不是單純的 SillyTavern 替代前端，而是一套相當完整的 AI chat / roleplay / RPG / visual-novel-style engine。

它目前涵蓋：

- 角色卡
- Lorebook
- 多人角色
- Individual generation
- AI GM
- World State
- Quest
- Inventory
- Combat
- Sprites
- Expression
- Background
- AI Image Generation
- Storyboard
- Video
- Agents
- API Provider
- Local Model
- Claude / ChatGPT / Grok subscription connection

因此，Table Tavern 不應再把下列項目視為核心差異化：

- 多角色聊天
- 每角色獨立 LLM generation
- 每角色只送自己的 Character Card
- Character-specific Lorebook
- AI GM
- RPG state / inventory / quest / combat
- SillyTavern card compatibility
- Sprites / expressions / backgrounds
- AI image generation
- VN-style presentation
- Storyboard / CG generation
- CLI / subscription model connection
- OpenRouter / OpenAI-compatible API support

這些功能可以做，但應視為「產品能力／必要配備」，而不是護城河。

目前 Table Tavern 最值得強化的三條主線為：

1. **資訊模型（Information Boundaries）**  
   把「誰知道什麼」視為世界狀態的一級概念，而非只依角色卡 ownership 過濾 prompt。

2. **GM + Independent Actors**  
   GM 是世界裁定者／導演，而 NPC 是各自獨立的 actor；避免讓 GM 成為所有 NPC 的共同大腦。

3. **Cost-Aware Architecture**  
   不是提供幾個 token 設定，而是從 session、cache、scene checkpoint、context layout 到 model dispatch 都以「讓長期多人 AI RP 真的付得起」為架構目標。

再加上一個產品層優勢：

4. **低門檻原生桌面體驗**  
   玩家不需要理解 Node、server、prompt cache、context trimming、agent pipeline 才能開始玩。

Table Tavern 不應試圖在「功能數量」上追過 Marinara。

應該做得更窄、更明確、更自動化。

---

## 2. Marinara Engine 的定位

目前 Marinara 可以理解為：

> 一個 Power User 取向的 AI Chat + Roleplay + RPG Engine，並包含大量 Visual Novel presentation 能力。

主要產品模式包含：

- Conversation
- Roleplay
- Game

雖然目前不一定以獨立「VN Mode」作為主要產品模式，但 Roleplay / Game 已具備大量 VN 所需能力：

- Character sprites
- Expression switching
- Scene backgrounds
- Location / time / weather HUD
- AI-generated backgrounds
- Scene illustrations
- Storyboard
- Image-to-video
- Choice / adventure-like interaction

因此 Table Tavern 的 VN Mode 應重新定位為 **Renderer / Presentation Layer**，而不是核心引擎創新。

建議概念：

```text
Table Tavern Engine
│
├─ Actor / Knowledge Simulation
├─ GM Orchestration
├─ Structured World State
├─ Cost / Cache / Session Engine
│
└─ Renderer
   ├─ Chat View
   └─ VN View
      ├─ Background
      ├─ Sprite
      ├─ Expression
      ├─ Position
      ├─ Choice
      └─ CG
```

核心引擎不應依賴 VN UI 才成立。

---

## 3. 已確認高度重疊的功能

### 3.1 Multi-character generation

Marinara Individual Mode 已做到：

- 每個角色可以各自產生一個 response
- Sequential / Smart / Manual 等角色發言策略
- generation 時只包含目前 responder 的 Character Card
- 完整 group roster 仍可另外提供給 prompt system

因此：

> 「每個角色各 call 一次 LLM」不是 Table Tavern 的獨特賣點。

### 3.2 Character-specific Lorebook

Marinara Lorebook 已支援：

- character-bound
- persona-bound
- chat-bound
- global

等 scope。

Individual generation 時會依目前 `characterIds` 過濾相關 Lorebook。

因此：

> 「Alice 有自己的 Lorebook、Bob 不直接拿 Alice 的 Lorebook」本身也不是 Table Tavern 的獨特能力。

但 Marinara 的 character ownership 不等於嚴格 privacy ACL，詳見後面的資訊模型章節。

### 3.3 RPG state

Marinara Game Mode 已涵蓋：

- World State
- Party
- NPC
- Inventory
- Quest
- Map
- Time
- Weather
- Combat
- Dice

不要因為競品有這些功能就立即全部補齊。

### 3.4 Visual / VN features

Marinara 已有：

- Sprite
- Expression Engine
- Background
- AI background generation
- Character portrait generation
- Storyboard
- Scene illustration
- Video generation

因此 Table Tavern 不應以「AI RP + VN」本身作為主要 differentiation。

### 3.5 Provider / subscription support

Marinara 已支援大量 API、Local 與 subscription connection。

因此 provider 數量不值得成為 Table Tavern 的競賽項目。

Table Tavern 應做到：

> 常用的很好用。

而不是：

> 支援列表最長。

---

## 4. Marinara 值得學習的設計

這一節是後續實作時最值得回頭看的 checklist。

### 4.1 Summary 必須真正取代 history

錯誤做法：

```text
Summary
+ 完整舊 history
+ 新 history
```

這只是增加 token。

Marinara 已處理：

- summary entries
- summarized message hiding
- recent tail retention
- summary combine
- hidden message tracking
- summary 刪除／還原時需要知道原先替代了哪些 messages

Table Tavern 應學習這些 edge cases，但不需要照搬 rolling-summary 架構。

Table Tavern 更適合：

```text
Scene N full transcript
        ↓
Scene checkpoint
        ↓
Canonical recap
        ↓
Scene N+1
```

並將 persistent structured state 與 narrative recap 分離。

### 4.2 Context fitting / overflow handling

Marinara 有成熟的 context fitting：

1. token estimate
2. 優先刪最舊 history
3. 必要時降低 output budget
4. 最後才動其他 prompt / system content

Table Tavern 應有同級安全網。

即使 scene checkpoint 能控制長期 context，也仍需處理：

- 單幕爆量
- 巨大角色卡
- Lorebook 突然大量 activation
- 使用者長時間不換幕
- 多角色同時需要大量 private context

建議 Table Tavern 明確定義 context priority：

```text
不可刪：
- core system contract
- current actor identity
- critical world state
- current action

高優先：
- relevant private state
- active scene facts
- active lore

中優先：
- current scene recent history

低優先：
- older current-scene history
- optional flavor lore
- redundant description
```

### 4.3 Lorebook activation engine

Marinara Lorebook 已有大量成熟能力，例如：

- keyword activation
- probability
- token budget
- sticky
- cooldown
- recursion
- character filter
- tag filter
- generation trigger filter
- state condition
- scheduling / dynamic conditions

Table Tavern 不必一次全部實作，但應把它當需求地圖。

優先考慮：

1. visibility
2. keyword / semantic relevance
3. state condition
4. token budget
5. priority
6. cooldown / sticky（若真的有需求）

不要一開始追求完整 ST-compatible power-user matrix。

### 4.4 Agent pipeline 分階段

Marinara 把部分工作交給 main response 周圍的小型 agents，例如：

- Character Tracker
- Expression Engine
- Background
- World State
- Storyboard

值得學的是「pipeline separation」，不是 agent 數量。

Table Tavern 應避免把所有工作都塞給 GM 一次輸出。

例如：

```text
GM orchestration
    ↓
Actor generation
    ↓
State extraction / validation
    ↓
Presentation decisions
    ↓
Renderer
```

但每增加一次 LLM call 都有成本，因此應優先使用 deterministic code；只有真的需要語意判斷時才增加 agent call。

### 4.5 Image provider abstraction

Marinara 支援大量圖片 provider，證明圖片 generation 應抽象成 provider layer，而不是把 VN Mode 綁死某一家服務。

Table Tavern 可以學 interface 設計概念，但不必追十幾家 provider。

優先：

- OpenAI-compatible image API
- OpenRouter Images（若實際穩定）
- 一個 local provider / ComfyUI 類型
- 未來再加 subscription bridge

### 4.6 Group generation strategies

Marinara 已證明多人 generation 至少有：

- sequential
- smart speaker selection
- manual
- merged narrator

Table Tavern GM orchestration 應吸收這個需求，但不必暴露成一堆 power-user switch。

玩家真正需要的是：

- 自動導演
- 指定某角色
- 全員反應／必要角色反應
- 暫停某角色

底層可以更複雜，UI 不必更複雜。

---

## 5. Marinara 不應跟著做的方向

### 5.1 不要追 feature count

Marinara 已有大量 agents、provider、RPG systems、image/video features。

Table Tavern 若採用：

> Marinara 有 → 我也做

會快速變成永遠追不完的 checklist。

不要以以下數字作 KPI：

- provider 數量
- agent 數量
- Lorebook option 數量
- RPG subsystem 數量
- generation backend 數量

### 5.2 不要把 Power User 複雜度直接搬到 UI

Marinara 很適合喜歡調設定的人。

Table Tavern 應該刻意服務另一種需求：

> 我只想放角色進桌上，選模型，開始玩。

Advanced options 可以存在，但預設路徑必須簡單。

### 5.3 不要讓 GM 演所有 NPC

Game Mode 若由 GM 同時負責世界與所有 NPC，容易產生：

- 所有 NPC 共享同一認知
- 秘密 leakage
- NPC 個性被 GM narrative objective 壓過
- NPC 缺乏真正獨立意圖

Table Tavern 應堅持：

> GM owns world truth and adjudication; actors own themselves.

### 5.4 不要把 Tracker 當成 cognition

AI 從 transcript 推測：

> Alice 現在很嫉妒。

和 Alice 自己持有：

```text
private emotion = jealousy
private goal = sabotage Bob
```

不是同一回事。

Table Tavern 應讓重要私人狀態成為權威資料，而不是只依賴旁路 agent 猜測。

### 5.5 不要無限 rolling summary

Rolling summary 很方便，但長期仍可能變成：

```text
Summary 1
Summary 2
Summary 3
...
Summary 100
```

Table Tavern 應優先維持 episodic / scene-based memory。

舊幕應被壓成固定成本，而不是永久增加 context。

### 5.6 不要為了 AI 感而濫用 AI call

Expression、speaker selection、state update、背景選擇等事情，如果 deterministic code 可以可靠完成，就不要額外 call LLM。

多人 RP 的成本天然會乘上角色數。

每一個 auxiliary agent 都必須回答：

> 這個 call 帶來的品質提升，值得它增加的成本嗎？

---

## 6. Table Tavern 的核心差異：Information Boundary

這可能是目前最重要的架構差異。

### 6.1 Character ownership 不等於 privacy

Marinara 的 character-specific Lorebook 表示：

> 這份 lore 與 Bob 相關。

但不必然表示：

> 只有 Bob 有權知道。

原始碼研究顯示 Marinara 有 Referenced Character Context：

當 prompt / history / card 等內容明確 reference 某角色時，可以載入該角色的 character context，並進一步處理 attached Lorebook。

因此 character scope 更接近 contextual relevance，而不是 security-style knowledge ACL。

Table Tavern 應明確區分：

```text
ownership != visibility
```

### 6.2 建議的 visibility model

不要只支援：

```text
public
private-to-character
GM-only
```

應允許：

```yaml
Fact / Lore / State:
  visible_to:
    - gm
    - alice
    - claire
```

Bob 不在列表，就永遠不應出現在 Bob 的有效 context。

這可以支援：

- 秘密
- 陣營情報
- 共同經歷
- 偷聽／目擊
- 誤會
- 戀愛三角
- 推理
- 狼人殺
- 宮鬥
- 私人任務
- 玩家不知道但 NPC 知道的情報

### 6.3 Hard boundary，而不是 prompt instruction

不要：

```text
Bob knows the queen is a vampire.
Alice does not know this.
Please roleplay Alice without using this information.
```

要：

```text
Alice request:

根本沒有「queen is a vampire」這個 token sequence。
```

這是 architectural guarantee。

### 6.4 Referenced Character 必須尊重 visibility

如果 Alice prompt 提到 Bob：

- 可以取得 Bob 的公開名稱／外觀／已公開資訊
- 不應因此取得 Bob private lore
- 不應因此取得 Bob private state
- 不應取得 GM-only interpretation

Reference resolution 必須經過 ACL，而不是繞過 ACL。

建議建立單一入口：

```text
resolve_visible_context(viewer_actor_id, target_entity_id)
```

所有 prompt assembly、Lorebook activation、reference resolution 都只能透過這一層取得資料。

---

## 7. Table Tavern 的核心差異：GM + Independent Actors

目標架構：

```text
                 GM
          world / rules / direction
                 │
       ┌─────────┼─────────┐
       ↓         ↓         ↓
     Alice      Bob      Claire
      Actor     Actor      Actor
       │         │          │
    private   private    private
    context   context    context
```

GM 不應擁有 NPC cognition。

GM 可以：

- 決定場景發生什麼
- 裁定玩家行動
- 決定哪些角色應該反應
- 更新 canonical world state
- 控制 scene transition
- 控制 NPC entrance / exit

GM 不應：

- 代替 Alice 決定 Alice 的私人想法
- 直接撰寫所有 NPC 對話（除非使用者選擇 merged / cheap mode）

Actor 可以：

- 根據自己可見資訊形成反應
- 撒謊
- 誤會
- 隱瞞
- 不同意 GM narrative expectation
- 基於 private goal 做決策

### 可考慮 Economy / Actor 模式

成本敏感時可提供：

```text
Economy Mode:
GM / narrator merged generation

Actor Mode:
GM + independent actor generations
```

但產品預設定位仍應圍繞真正 actor simulation，而不是把它做成隱藏的 advanced checkbox。

---

## 8. Table Tavern 的核心差異：Cost-Aware Architecture

這是目前非常值得強化的競爭軸。

Marinara 有：

- context trimming
- summary
- prompt caching
- Claude subscription resume

因此「有 cache」本身不是差異。

Table Tavern 的機會是：

> 整個引擎從一開始就為多人長期 RP 的成本最佳化，而不是讓使用者自己調 token knobs。

### 8.1 Shared Public Cache + Private Ephemeral Context

目前 Table Tavern 的 Claude lane 概念值得保留：

```text
同模型 actors
      ↓
shared public session
      ↓
輪到 Alice
      ↓
注入 Alice private overlay
      ↓
generate
      ↓
erase confidential segment
      ↓
下一位 actor resume public session
```

這是一個重要 trade-off：

- 公開 transcript 可以共享 cache
- private knowledge 不需要永久複製到每個角色 session
- 不同 actor 不必各自重送完整共同歷史

可將這個概念明確定義為：

> **Shared Public Cache + Private Ephemeral Context**

### 8.2 Session 不應承擔權威私人記憶

共用 session 的代價是：

Alice 上一輪未說出口的 latent internal state 不會自然保留在 Alice 專屬 session。

不要試圖依賴模型 latent session memory 解決。

重要資料應寫回 Table Tavern 自己的 state：

- private goal
- relationship
- emotion（若玩法需要）
- secret
- plan
- discovered facts

下一輪再經 visibility layer 注入。

優點：

- deterministic
- 可存檔
- 可 rollback
- 可換模型
- 可 debug
- 不依賴 provider session semantics

### 8.3 Scene checkpoint / 換幕

這可能是最值得產品化的成本能力之一。

目標：

```text
Scene 1
full transcript
      ↓
scene close
      ↓
canonical recap
+ structured state
      ↓
Scene 2 starts fresh
```

下一幕不再重送 Scene 1 全 transcript。

Recap 應區分：

- public recap
- GM recap / canonical hidden facts
- actor-specific remembered facts（若必要）

避免用一份 summary 把 private/public knowledge 混在一起。

### 8.4 Recap 應有固定 budget

不要讓 recap 永久線性膨脹。

建議：

- scene recap 有 token ceiling
- 多幕後做 arc-level consolidation
- structured facts 不需要全部重複寫入 prose recap

例如：

```text
Persistent State
- 王死亡
- Alice 持有鑰匙
- Bob 不知道 Alice 是兇手

Arc Recap
- 只保留敘事上需要理解下一幕的資訊
```

### 8.5 Stable prefix / cache-friendly prompt layout

Prompt layout 應刻意安排：

```text
[stable system]
[stable game rules]
[stable world schema]
[stable actor public definition]
[slow-changing recap/state]
---------------- cache-friendly boundary
[current scene]
[current actor private overlay]
[current action]
```

越常變的資料越往後。

不要讓：

- timestamp
- random IDs
- 動態 debug 資訊
- 不必要的重新排序

污染前綴造成 cache miss。

### 8.6 Persistent Claude session + delta transmission

Table Tavern 現有 lane 設計相較 Marinara 值得強化：

- persistent session ID
- sent-event watermark
- 每輪只送新事件
- scene change reopen
- frozen system handling
- system patch
- cache TTL rebase
- keepalive

Marinara Claude subscription 目前偏向每 request 重新建立 synthetic session history，再 resume current turn。

Table Tavern 若能穩定維持真正 persistent lane，這是工程差異。

### 8.7 Keepalive

若 provider 的 prompt cache 有短 TTL，而使用者 RP 常停下來想幾分鐘，keepalive 有實際價值。

但必須：

- 成本極低
- 不污染 narrative history
- UI 可關閉
- provider-aware
- 不為沒有 cache benefit 的 provider 發送

### 8.8 Model tier dispatch

多人桌不需要所有工作都用最貴模型。

例如：

```text
GM              → Opus / high tier
重要 NPC         → Sonnet / medium-high
普通 NPC         → cheap model
summary          → cheap model
state extraction → cheap / deterministic
expression       → deterministic
speaker select   → deterministic / cheap
```

甚至允許：

```text
Alice  → Claude
Bob    → Gemini
Claire → Local
GM     → Opus
```

但 UI 應提供簡單 preset，而不是要求一般玩家理解 routing graph。

### 8.9 Cost telemetry 是產品功能

不要只顯示 raw token count。

應顯示：

- 本局 input/output
- cache hit
- cache saving estimate
- 每角色成本
- GM 成本
- auxiliary call 成本
- 本幕成本
- 全桌成本

對一般玩家可呈現：

```text
本局估計原始成本：$3.82
快取／壓縮已節省：$2.41
實際估計成本：$1.41
```

玩家不需要懂 cache read token 才能感受到價值。

---

## 9. Cost Architecture 與 Information Isolation 的衝突

這是 Table Tavern 必須正面解決的核心工程問題。

最強 isolation：

```text
每 actor 完全獨立 session
```

成本最高。

最便宜：

```text
所有 actor 共用完整 session/context
```

privacy 最差。

Table Tavern 的目標應是找到中間點：

```text
共享：
- public transcript
- public world facts
- stable rules

隔離：
- private lore
- private state
- hidden goal
- GM-only truth
```

也就是：

> **共用可共用的 token；隔離必須隔離的 knowledge。**

任何成本最佳化都不得突破 visibility boundary。

若發生衝突，correctness / privacy 應優先於 cache saving。

---

## 10. Desktop / UX 優勢

Table Tavern 是 Tauri 原生桌面 App，這個優勢值得保留，但不能單獨成為護城河。

理想 onboarding：

```text
下載
↓
拖進 Applications / 安裝
↓
選模型來源
↓
匯入角色
↓
開始玩
```

不要要求玩家先理解：

- Node
- pnpm
- localhost server
- reverse proxy
- cache breakpoint
- token budget
- agent phase
- prompt assembly

Advanced users 可以打開進階設定。

Default path 應由 Table Tavern 自動做合理決策。

---

## 11. 商業化方向

Marinara 本體免費且 AGPL 開源，因此 Table Tavern 若只是：

> 功能較少的付費 Marinara

商業價值很弱。

可收費的價值必須來自：

- 更低使用門檻
- 更可靠的 desktop packaging
- 自動成本最佳化
- 更好的 actor / knowledge simulation
- 更好的 onboarding / presets
- 更一致的遊戲體驗
- 玩家不需要自己研究 prompt engineering

不要以：

> Marinara 免費，所以 Table Tavern 一定不能收費。

作結論。

真正問題是：

> Table Tavern 是否替使用者省下足夠多的時間、設定成本、API 費用或認知負擔？

若答案否，則不應強行商業化。

若答案是：

> Table Tavern 每月替長期 RP 玩家省下的 API 成本就超過軟體價格。

那成本最佳化甚至本身可以成為商業價值。

---

## 12. AGPL / Clean-room Reference 原則

Marinara Engine 為 AGPL-3.0 開源專案。

Table Tavern 可以：

- 研究功能
- 研究 UX
- 研究資料流
- 研究公開文件
- 研究 bug / issue / edge case
- 理解演算法與問題拆解
- 用自己的架構重新實作相同類型功能

若希望保留 Table Tavern 未來 proprietary / closed-source 的可能性，應避免：

- copy Marinara source code
- 複製後改變數名稱
- 大段近似翻寫 function
- 將 Marinara AGPL module 直接整合進 proprietary core
- 複製其品牌素材／Logo

建議每個參考功能都在本文件留下紀錄：

```text
Feature:

Marinara 解決的問題：

Marinara 採用的概念：

值得學習：

不採用：

Table Tavern 自己的設計：

Implementation note:
Clean-room rewrite; no Marinara source copied.
```

這份文件本身應成為設計來源追蹤紀錄。

正式商業 release 前應做一次完整 dependency / license audit。

---

## 13. 後續功能研究模板

未來實作任何 Marinara 已有功能前，先填：

### Feature: `<name>`

**問題**

這個功能真正要解決什麼玩家問題？

**Marinara 做法**

只記錄概念、資料流、UX、edge cases，不複製 source。

**優點**

- ...

**缺點 / 限制**

- ...

**Table Tavern 是否需要**

- 必要
- 可延後
- 不需要

**Table Tavern 差異**

- ...

**成本影響**

- LLM call 數量
- input token
- output token
- cache 影響
- persistent state 影響

**Information Boundary 影響**

- public
- actor-private
- subset-visible
- GM-only

**實作原則**

Clean-room rewrite; no Marinara source copied.

---

## 14. 建議優先研究／借鏡清單

### P0 — 直接影響核心架構

- Prompt assembler / context priority
- Lorebook activation / filtering
- Referenced Character Context
- Summary replacement / hidden message lifecycle
- Context fitting
- Group generation orchestration
- Provider cache handling
- Claude subscription resume behavior

### P1 — Table Tavern 很快會需要

- ST card importer edge cases
- Image provider abstraction
- Expression selection
- Background selection
- State extraction / validation
- Save / rollback / branch model
- Error recovery / interrupted generation

### P2 — 有需求再做

- Inventory
- Quest
- Map
- Weather
- Combat
- Dice
- Storyboard
- Video
- 大量 auxiliary agents

不要因為 Marinara 已經有 P2 就提前做 P2。

---

## 15. 建議 Table Tavern 接下來強化順序

### 第一優先：把 Information Boundary 做成正式規格

需要明確定義：

- public
- GM-only
- actor-only
- arbitrary actor subset
- reference resolution rules
- summary visibility
- state visibility
- Lorebook visibility

並寫測試證明 secret 不會跨 actor leakage。

### 第二優先：Cost Engine

把目前已有的 cache / lane / scene checkpoint 從零散最佳化提升成正式 subsystem。

應有：

- cache metrics
- lane lifecycle
- scene checkpoint
- recap budget
- context budget
- model tier routing
- cost estimate
- provider-aware optimization

### 第三優先：GM + Actor contract

正式規定：

- GM 負責什麼
- Actor 負責什麼
- 誰能改 canonical state
- Actor 如何提出 action
- GM 如何 adjudicate
- NPC 如何取得結果

避免 GM 與 actor 職責慢慢混在一起。

### 第四優先：VN Renderer

在 engine contract 穩定後，再把：

- entrance / exit
- scene change
- speaker
- expression
- background
- CG

映射成 renderer events。

不要讓 VN Mode 反過來綁死 engine。

---

## 16. Table Tavern 應避免的定位

不要：

> SillyTavern but prettier.

不要：

> Marinara but desktop.

不要：

> AI Visual Novel Generator.

不要：

> 支援最多 AI Provider 的 RP 工具。

目前較有希望的定位是：

> **一張由真正獨立 AI 角色共同參與的桌。**

技術描述：

> **Multi-actor dramatic simulation with explicit information boundaries and cost-aware orchestration.**

玩家描述可以更簡單：

> **不是一個 AI 演所有角色。每個角色真的只知道自己該知道的事。**

並加上成本承諾：

> **桌子會自己管理上下文、快取與換幕，不要求玩家為了省 token 研究 LLM 工程。**

---

## 17. 判斷新功能是否值得做的四個問題

未來每次看到 Marinara 或其他競品有新功能，不要立即加入 backlog。

先問：

1. **它有沒有強化「角色真的彼此獨立」？**
2. **它有沒有降低長期多人 RP 的實際成本？**
3. **它有沒有讓普通玩家更容易開始／繼續遊戲？**
4. **它是不是 engine 必須有，而不是單純 feature checkbox？**

四個都不是，就大概率可以延後。

---

## 18. 目前競爭判斷

Marinara 的存在證明：

> AI RP + RPG + VN presentation 已經不是空白市場。

這不是 Table Tavern 應該放棄的理由，但代表產品必須停止依靠表面功能差異。

目前最值得驗證的假設是：

### Hypothesis A

嚴格的 actor-specific / subset-specific knowledge boundary 能產生比一般 group RP 更可信的戲劇互動。

### Hypothesis B

Shared Public Cache + Private Ephemeral Context 可以在不破壞 knowledge isolation 的情況下，大幅降低多人 actor 的成本。

### Hypothesis C

Scene Checkpoint 可以讓數十小時 campaign 的 context 成本維持在可預測範圍，而不是隨總 transcript 線性成長。

### Hypothesis D

普通玩家願意為「不用理解上述任何東西也能得到效果」的桌面產品付費。

這四項比再增加十個 provider 或五個 agent 更值得優先驗證。

---

## 19. 最重要的設計原則

> **不要跟 Marinara 比誰功能多。**

> **學它已經踩過的坑，但不要照著它長成同一個產品。**

> **所有重要世界資訊都有明確 owner 與 visibility。**

> **GM 是裁定者，不是所有角色的共同大腦。**

> **共用可共用的 token；隔離必須隔離的 knowledge。**

> **換幕不是 UI 特效，而是 memory / cost checkpoint。**

> **重要狀態存在 Table Tavern，不依賴模型不可見的 session 記憶。**

> **玩家不應該需要懂 prompt caching 才能省錢。**

> **VN 是 renderer；actor simulation 才是 engine。**

> **研究 Marinara 的設計，不複製 Marinara 的 AGPL 程式碼。**

---

## 20. 後續研究待辦

- [ ] 量測 Marinara 4-character Individual Mode 的實際 input token amplification
- [ ] 量測 Table Tavern shared lane 在相同 transcript 的 token / cache 成本
- [ ] 建立 1h / 10h / 50h campaign 成本模型
- [ ] 測試 Marinara Referenced Character 是否造成實際 private lore leakage
- [ ] 定義 Table Tavern visibility ACL schema
- [ ] 定義 scene recap 的 public / GM / actor-private 分層
- [ ] 定義 recap token ceiling 與 arc consolidation
- [ ] 為 visibility 建立 automated leakage tests
- [ ] 為 cache lane 建立 crash / rewrite / TTL tests
- [ ] 比較 Marinara 與 Table Tavern 的 branch / rollback 資料模型
- [ ] 研究 Marinara image provider abstraction，但只採用必要 interface concepts
- [ ] 商業 release 前執行完整 dependency / OSS license audit

---

## 21. Reference Policy

本文件記錄的是競品研究結果與設計概念，不應作為複製 Marinara source 的指示。

實作任何相似功能時：

1. 先從本文件整理需求與 edge cases。
2. 關閉／不依賴 Marinara 具體 function 寫法。
3. 依 Table Tavern 自身資料模型與架構重新設計。
4. 加入 Table Tavern 自己的 tests。
5. 在本文件記錄採用／不採用的設計理由。

目標不是做一個 Marinara clone。

目標是讓 Marinara 已經支付過的「設計探索成本」成為 Table Tavern 可以學習的產業知識，同時保留自己的架構、授權與產品方向。