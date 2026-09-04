use super::prompt_common::system_message;
use super::types::PrescanSignal;
use crate::transport::ChatMessage;

const SURVEY_BODY: &str = r#"現在是「盤點」階段。請把上面「世界書條目」整本讀完，產出一份給 App 照抄執行的「小抄」：
不重寫任何條目原文，只對每條內容蓋章分類、標好去處，實際搬移／改寫都由 App 依你的分類機械執行。

上面「世界書條目」裡每條內容都被切成一段一段，每段開頭都標了 `⟦s1⟧`、`⟦s2⟧`……（各條各自從 s1 起編）；
下面凡是要你引用「某條目的某一段」的地方，一律寫成 `<uid>#s<段號>`（例如條目 12 第 2 段就是 `12#s2`）。

一、人物（PERSONS）：條目在描述具體「人」的設定（性格、外觀、背景、關係……），可以升格成角色卡；單人專屬
條目、多人合集都要列，不要因為一條只寫一個人就跳過。組織、勢力、職稱／頭銜（例如「教會」「賢者議會」
「聖堂侍從」）不是人，不要列。同一個人的資料散在好幾條時（自己專屬的條目、跟別人共用的合集條目裡的一段
……）合併成一筆，把他所有的來源條目 uid 都列出來；同一人有多語言重複版本時（例如中文版與英文版），只留
跟玩家語言較接近的那一版 uid，被捨棄的那一版不要列進 PERSONS、也不要另外算一個人，改在 ENTRIES 給它
`action: drop rule: 4`（內容重複）。整張卡最多標記一人為疑似玩家本人（玩家
在玩的那個角色，內文常見 {{user}} 或明顯是「你」的視角）；沒把握是誰就不要標，最多一人。
額外判斷 mode（選填，沒把握就不寫）：
- clean：spans 引用的段落直接照原文拼起來就是一份完整角色卡。資料散在多條、多個區塊不算糾纏——只要
  各段原文本身完整可讀就是 clean；原文是什麼語言就照什麼語言拼裝，不因為想換語言、換格式而改判。
  spans 列出這個人全部段落（可以橫跨他的好幾個來源 uid），private 再從中挑出屬於祕密／只有扮演者該
  知道的段落，沒有私密內容就不寫這欄。
- tangled：同一段落裡多人混寫、非拆不可（一段裡同時寫好幾個人的設定，摘不出哪句屬於誰），只有這種
  才需要真的整理、才選 tangled。
沒把握該選哪個就不寫 mode，效果等同 tangled，不會出錯，只是這個人省不到這次的零呼叫組裝。

二、介面（INTERFACE）：條目在定義狀態欄／介面的格式（結構化欄位怎麼顯示），不是在說故事。每條加註
playable 判定——playable: yes 只給「卡片定義了一個玩家可以完全在裡面遊玩的介面」：有劇情正文顯示區、
有行動入口，遊玩過程發生在這個介面裡。只是狀態欄、屬性面板、資訊模板（顯示數值、天數、環境之類），一律
playable: no。沒把握就 no。

三、其餘條目分類（ENTRIES）：PERSONS 完整收走的人物專屬條目、INTERFACE 完整收走的介面格式條目不用再列；
其餘每一條世界書條目都要在這裡出現一行，action 四選一：
- carry（照搬，沒把握就選這個）：整條原文照搬進新世界書，不重寫、不加工。
- absorb（接管，僅限三種）：條目在描述「逐日排程」（第幾天發生什麼）、「觸發事件」（滿足某條件時演出某
  段內容——數值觸發、情境觸發都算）、或「App 需要追蹤並隨劇情更新的狀態欄位」。條目本身就是
  「trigger:／condition: 一行條件，接一段屆時照演的劇本」這種樣式的，直接 absorb；劇本包在 <story>
  之類的容器標籤裡也算——那是等條件滿足才演出的機制，不是歷史紀錄。靜態目錄（勢力／地點／物品列表，
  只是條列說明）與歷史年表（事件按時間排列、已經發生過的敘事）都不算，屬於設定，一律 carry。
- drop（淘汰，僅限四種，必附 rule 編號）：① 輸出容器紀律——原卡寫給別的平台的輸出格式指令（例如「請用
  以下格式回覆」），本 App 有自己的介面機制，這類指令沒有意義；② 版本標記／更新日誌——版本號、更新
  歷程、製作心得，不是遊玩內容；③ ST 引擎專屬鉤子——只有原平台看得懂的巨集／腳本語法，本 App 無法執行；
  ④ 內容重複——這條的內容已被另一條取用（語言重複版本、新舊版並存、作者複製貼上），取用的那條照常分類，
  被捨棄的這條標 drop rule: 4。
  不在這四種之列或拿不準——一律不淘汰，改選 carry；作者設計的世界觀、規則、劇情內容永遠不准淘汰。
- split（需拆）：一條裡混了兩種以上去處（例如介面格式段落＋機制段落、人物段落＋設定段落、該接管的機制＋
  該照搬的設定），沒辦法整條歸一類——這裡標 split，實際去處逐段寫進 SPLITS。
下面「結構預掃訊號」列出的段落如果落在你判給 carry 的條目裡，這行要附一句 reason 說明為什麼不需要
absorb（例如訊號誤判、或那段其實是歷史紀錄不是即時機制）。

四、拆條目（SPLITS）：ENTRIES 標 split 的條目，把它每一段都指定去處，route 七選一：
- statusbar：這段在定義狀態欄／介面格式（跟 INTERFACE 條目性質一樣，只是混在別的條目裡）。
- gm：這段是敘事性的行為指令（GM 該怎麼描述、怎麼引導劇情），原文會照搬進一條「GM 規則」條目。
- drop rule: <1|2|3|4>：這段屬於上面淘汰清單四種之一。
- person name: <人名>：這段是某個人的專屬內容，人名必須是 PERSONS 裡出現過的名字，會併入他的角色卡。
- entry title: <新條目標題>：這段是設定內容，同一個 title 底下的所有段會依原文順序串接成一條新設定
  條目，原文照搬、你不用重寫。
- group id: <gN>：這段要跟其他條目的段一起組成新條目，id 對應下面 GROUPS 區塊的宣告，用在同一個機制
  被拆散在好幾條原始條目裡的情況。
- unabsorbed note: <一句話>：這段描述的機制目前 App 還沒有對應的執行機構，抽不成結構化規則，但也不是
  要丟掉——原文會照搬進 GM 規則條目，note 講清楚這是什麼機制，方便之後告訴玩家哪些機制還沒被系統接管。
一段只能有一個去處；沒把握歸哪一類，設定內容就走 entry（原文照搬永遠安全），機制內容就走 unabsorbed。

五、合組（GROUPS）：SPLITS 裡標 group 的段，在這裡宣告它們組成的新條目：id 要跟 SPLITS 對應，kind 決定
這組內容怎麼處理（setting｜mechanism），spans 是這組全部成員（逗號分隔，順序就是新條目裡的原文順序，
可以橫跨好幾個不同的來源 uid）。

六、狀態欄位命名（FIELDS）：把 ENTRIES／SPLITS 會用到的全部狀態欄位名（statusbar、absorb、group 裡
kind=mechanism 會抽出來的欄位）在這裡一次列全，一行一個——這是這張卡全部狀態欄位命名的唯一權威，後面
每一次實際執行都只能沿用這裡列出的名字，不准另創同義詞。欄位名一律玩家語言，機器好處理（數字欄位就是
數字，不要「第 1 天」這種帶敘述的字串）。

嚴格照以下標記輸出，標記之外不要有任何文字：

## PERSONS
- name: <人名> uids: <來源條目 uid，逗號分隔> player: <疑似玩家本人寫 yes，最多一人；其餘不寫> mode: <clean|tangled，沒把握不寫> spans: <mode: clean 才需要，逗號分隔> private: <從 spans 挑出的私密段，沒有不寫>
（一行一人；一個都沒有就留空，不要寫「無」）

## INTERFACE
- uid=<條目 uid> playable: <yes|no>
（一行一條；沒有就留空）

## ENTRIES
- uid=<條目 uid> action: <carry|absorb|drop|split> rule: <僅 drop 需要，1|2|3|4> reason: <選填，跟預掃訊號衝突時必填>
（一行一條，見上面說明哪些條目不用列；沒有就留空）

## SPLITS
- span: <uid#s段號> route: <statusbar|gm|drop rule: N|person name: X|entry title: T|group id: gN|unabsorbed note: 說明>
（只有 ENTRIES 標 split 的條目才需要；那條目的每一段都要出現一次；沒有 split 條目就留空）

## GROUPS
- id: <gN> title: <新條目標題> kind: <setting|mechanism> spans: <成員段落，逗號分隔>
（只有 SPLITS 用到的 group id 才需要在這裡宣告；沒有就留空）

## FIELDS
- <欄位名>
（一行一個；完全沒有狀態欄位就留空）"#;

/// 結構預掃訊號列成一段文字，隨 survey user 訊息注入判官參考；沒有訊號也要講清楚沒有，避免
/// AI 誤以為是漏列。
fn signals_line(signals: &[PrescanSignal]) -> String {
    if signals.is_empty() {
        "結構預掃訊號：（無）".to_owned()
    } else {
        let mut lines = vec![
            "結構預掃訊號（app 對條目原文做的關鍵字掃描，僅供參考，不代表一定要 absorb；你的判定\
            如果跟訊號衝突，例如訊號命中的段落所在條目你判給 carry，ENTRIES 那一行要附 reason 說明\
            為什麼）："
                .to_owned(),
        ];
        for signal in signals {
            lines.push(format!("- uid={} 含 {}", signal.span, signal.pattern));
        }
        lines.join("\n")
    }
}

/// 模式段（refactor-mode-split）：玩家選定玩法後的判定調整＋MODE 回聲指示。放 user 訊息端，
/// system 逐位元組相同的快取紅線零觸碰。
const INTERFACE_MODE_BODY: &str = r#"玩家已為這張卡選定玩法：保留原卡玩法（interface）。本次盤點依此調整：
- 人物一律不拆出：PERSONS 區塊固定留空（一行都不要寫）。純人物條目在 ENTRIES 給 `action: carry`
  （照搬進世界書，遊玩時由 GM 代言這些人物）；人物與其他內容混寫的條目照樣走 split，但人物段落改走
  `entry title: <人名>`（照搬成以人名為題的設定條目），`person name:` 這個 route 本次不可使用。
- 介面（INTERFACE）、狀態欄段落（statusbar）與其他判定照常。
輸出時在最前面多一行（放在 ## PERSONS 之前），逐字照寫，供 App 核對玩法沒有跑錯：

## MODE: interface"#;

const CHARACTERS_MODE_BODY: &str = r#"玩家已為這張卡選定玩法：改成多角色對話（characters）。本次盤點依此調整：
- 人物照常認（PERSONS 完整輸出），他們會被拆成本 App 的角色卡。
- 介面判定照常輸出（INTERFACE 區塊與 statusbar route 照判）：App 不會為這張卡建任何介面產物，
  這些判定只用來記錄條目下落，介面格式內容會進「已淘汰」清單保留。
輸出時在最前面多一行（放在 ## PERSONS 之前），逐字照寫，供 App 核對玩法沒有跑錯：

## MODE: characters"#;

fn mode_body(mode: &str) -> &'static str {
    if mode == "characters" {
        CHARACTERS_MODE_BODY
    } else {
        INTERFACE_MODE_BODY
    }
}

/// 盤點階段訊息：signals＝app 結構預掃訊號（見 `prescan_worldbook`），隨 user 訊息注入判官參考；
/// mode＝玩家選定玩法（interface｜characters），決定模式段文本與 MODE 回聲要求。
pub fn survey_messages(
    context: &str,
    signals: &[PrescanSignal],
    lang: &str,
    mode: &str,
) -> Vec<ChatMessage> {
    vec![
        system_message(context),
        ChatMessage {
            role: "user".to_owned(),
            content: format!(
                "{SURVEY_BODY}\n\n{}\n\n{}\n\n全部內容（人名與專有名詞以外）使用 BCP-47 語言代碼「{lang}」對應的語言；GROUPS／SPLITS 的 title 也用這個語言。",
                mode_body(mode),
                signals_line(signals)
            ),
        },
    ]
}

/// 定向階段（初判）：supported 卡在玩家二選一之前先跑的快速判斷，帶全卡只出兩行。
/// system 與盤點逐位元組相同（同一 session 內第二段承前綴快取）。
const RECOMMEND_BODY: &str = r#"現在是「定向」階段。這張卡接下來會被重構成兩種玩法之一，由玩家二選一：
- interface（保留原卡玩法）：卡片自帶的遊戲介面照原樣接管，人物不拆出，玩法與原卡完全相同。
- characters（多角色對話）：卡裡的人物拆成本 App 的角色卡，用多角色對話玩，卡片介面不再使用。

請把上面整張卡讀完，判斷哪種玩法比較符合這張卡的設計意圖。參考基準：卡片有完整可遊玩介面（有劇情
正文顯示區、有行動入口，遊玩過程發生在介面裡）通常代表作者設計就是介面玩法；卡片以多位帶完整設定的
人物為主、介面只是狀態欄點綴，則適合多角色對話。

嚴格只輸出以下兩行，此外不要有任何文字：

RECOMMEND: <interface|characters>
EVIDENCE: <一句人話證據，講這張卡最關鍵的特徵，例如「這張卡有完整遊戲介面」或「卡內有 8 位帶完整設定的人物」>"#;

pub fn recommend_messages(context: &str, lang: &str) -> Vec<ChatMessage> {
    vec![
        system_message(context),
        ChatMessage {
            role: "user".to_owned(),
            content: format!(
                "{RECOMMEND_BODY}\n\nEVIDENCE 使用 BCP-47 語言代碼「{lang}」對應的語言。"
            ),
        },
    ]
}
