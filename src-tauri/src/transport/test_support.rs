use crate::data::{
    CharacterCard, Tier, TranscriptEvent, TranscriptKind, Visibility, WorldbookEntry,
};

pub(super) fn card(id: &str, name: &str, public_md: &str, private_md: &str) -> CharacterCard {
    CharacterCard {
        id: id.to_owned(),
        name: name.to_owned(),
        color: "#336699".to_owned(),
        avatar: "🦊".to_owned(),
        tier: Tier::Balanced,
        show_image: true,
        archived: false,
        gen_prompt: String::new(),
        public_md: public_md.to_owned(),
        private_md: private_md.to_owned(),
    }
}

pub(super) fn event(
    kind: TranscriptKind,
    speaker_id: &str,
    speaker_name: &str,
    text: &str,
) -> TranscriptEvent {
    TranscriptEvent {
        id: None,
        message_vars: None,
        vars_rev: None,
        vars_epoch: None,
        turn_key: None,
        action_id: None,
        raw: None,
        ts: "2026-07-19T12:00:00+08:00".to_owned(),
        speaker_id: speaker_id.to_owned(),
        speaker_name: speaker_name.to_owned(),
        kind,
        text: text.to_owned(),
        state: None,
        truncated: false,
        gm_only: false,
        marker: None,
        opening: false,
    }
}

pub(super) fn worldbook_entry(
    uid: u64,
    title: &str,
    keys: &[&str],
    constant: bool,
    order: i64,
    disabled: bool,
    visibility: Visibility,
) -> WorldbookEntry {
    WorldbookEntry {
        uid,
        title: title.to_owned(),
        keys: keys.iter().map(|key| (*key).to_owned()).collect(),
        content: format!("{title}內容"),
        constant,
        order,
        disabled,
        visibility,
        is_person: false,
        locked: false,
    }
}

/// 假串流伺服器：先回 200 標頭，再依腳本 `(送出前等待毫秒, 內容)` 逐段寫出，
/// 最後把連線再撐 `hold_ms` 才關（模擬上游卡住但連線沒斷）。回傳 base URL。
pub(super) fn stall_server(script: Vec<(u64, String)>, hold_ms: u64) -> String {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        let Ok((mut socket, _)) = listener.accept() else {
            return;
        };
        let mut request = [0u8; 8192];
        let _ = socket.read(&mut request);
        let head =
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n";
        if socket.write_all(head.as_bytes()).is_err() {
            return;
        }
        for (wait, text) in script {
            std::thread::sleep(std::time::Duration::from_millis(wait));
            if socket.write_all(text.as_bytes()).is_err() || socket.flush().is_err() {
                return;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(hold_ms));
    });
    format!("http://{address}")
}

/// 測試用：登場事件的代換（只換 `{{user}}`／`{{char}}`，與巨集引擎在這些文字上的結果相同）。
pub(crate) fn plain_fill(
    user: &str,
) -> impl Fn(&str, Option<&crate::data::CharacterCard>, bool) -> String + '_ {
    move |text, card, _| {
        super::messages::replace_st_macros(text, user, card.map(|card| card.name.as_str()))
    }
}

/// 測試用：舊簽名（直接吃精簡條目清單）的組裝入口。照實送同一支掃描（`world_scan::scan`）掃一次、`prepare`
/// 代換再組裝：沒有上限、量測用亂數、空計時表、空變數。各測試檔以具名 import 蓋過 glob 匯入的新簽名。
pub(crate) mod legacy {
    use crate::data::{CharacterCard, Mechanism, TableState, TranscriptEvent, WorldbookEntry};
    use crate::transport::{self, ChatMessage, Hoist, LaneTurn, StateScope};
    use crate::world_scan::{
        self, MacroInputs, MacroSession, Randomness, ScanRequest, TableBook, Viewer, WorldScan,
    };

    /// 掃描＋代換。`world_md` 只有 GM 視角用得到。
    pub(crate) fn prepared(
        worldbook: &[WorldbookEntry],
        viewer: Viewer<'_>,
        world_md: &str,
        cards: &[CharacterCard],
        player: Option<&CharacterCard>,
        events: &[TranscriptEvent],
        lang: &str,
    ) -> WorldScan {
        let book = TableBook::from_views(worldbook);
        prepared_book(&book, viewer, world_md, cards, player, events, lang)
    }

    pub(crate) fn prepared_book(
        book: &TableBook,
        viewer: Viewer<'_>,
        world_md: &str,
        cards: &[CharacterCard],
        player: Option<&CharacterCard>,
        events: &[TranscriptEvent],
        lang: &str,
    ) -> WorldScan {
        let session = MacroSession::new(
            MacroInputs::empty(),
            &viewer,
            world_md,
            book.world_card_name(),
            player,
            events,
            lang,
        );
        let mut scan = world_scan::scan(ScanRequest {
            book,
            viewer,
            sole_card: (cards.len() == 1).then(|| cards[0].name.as_str()),
            player,
            events,
            lang,
            timed: Default::default(),
            budget: None,
            random: Randomness::Measure,
            session: &session,
        });
        world_scan::prepare(&mut scan, &session, book, &viewer, cards, player);
        scan
    }

    pub(crate) fn scan_of(
        worldbook: &[WorldbookEntry],
        viewer: Viewer<'_>,
        cards: &[CharacterCard],
        player: Option<&CharacterCard>,
        events: &[TranscriptEvent],
        lang: &str,
    ) -> WorldScan {
        prepared(worldbook, viewer, "", cards, player, events, lang)
    }

    /// 角色共線 system 用的掃描：卡片文字與共用快照不看視角，借第一張卡（沒有卡時借一張空卡）。
    pub(crate) fn system_scan(
        worldbook: &[WorldbookEntry],
        cards: &[CharacterCard],
        player: Option<&CharacterCard>,
        lang: &str,
    ) -> WorldScan {
        let blank = CharacterCard {
            id: String::new(),
            name: String::new(),
            color: String::new(),
            avatar: String::new(),
            tier: crate::data::Tier::Fast,
            show_image: true,
            archived: false,
            gen_prompt: String::new(),
            public_md: String::new(),
            private_md: String::new(),
        };
        let viewer = cards.first().unwrap_or(&blank);
        prepared(
            worldbook,
            Viewer::Character(viewer),
            "",
            cards,
            player,
            &[],
            lang,
        )
    }

    pub(crate) fn gm_lane_system(
        world_md: &str,
        cards: &[CharacterCard],
        player: Option<&CharacterCard>,
        worldbook: &[WorldbookEntry],
        mechanism: &Mechanism,
        lang: &str,
    ) -> String {
        let scan = prepared(worldbook, Viewer::Gm, world_md, cards, player, &[], lang);
        transport::gm_lane_system(cards, player, &scan, mechanism, lang)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn gm_lane_turn(
        events: &[TranscriptEvent],
        worldbook: &[WorldbookEntry],
        player: Option<&CharacterCard>,
        state: &TableState,
        mechanism: &Mechanism,
        scope: &StateScope,
        instruction: &str,
        lang: &str,
    ) -> LaneTurn {
        let scan = scan_of(worldbook, Viewer::Gm, &[], player, events, lang);
        transport::gm_lane_turn(&scan, player, state, mechanism, scope, instruction, lang)
    }

    pub(crate) fn chars_lane_system(
        cards: &[CharacterCard],
        player: Option<&CharacterCard>,
        worldbook: &[WorldbookEntry],
        lang: &str,
    ) -> String {
        let scan = system_scan(worldbook, cards, player, lang);
        transport::chars_lane_system(cards, player, &scan, lang)
    }

    /// `hoist`：舊的 `hoist_private`，對應 API 單卡那種（私設與穩定的機密條目進 system）。
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn chars_lane_turn(
        card: &CharacterCard,
        player: Option<&CharacterCard>,
        events: &[TranscriptEvent],
        worldbook: &[WorldbookEntry],
        state: &TableState,
        mechanism: &Mechanism,
        branch: Option<&[String]>,
        lang: &str,
        hoist: bool,
    ) -> LaneTurn {
        let cards = std::slice::from_ref(card);
        let scan = scan_of(
            worldbook,
            Viewer::Character(card),
            cards,
            player,
            events,
            lang,
        );
        let hoist = match hoist {
            true => Hoist::StableConfidential,
            false => Hoist::None,
        };
        transport::chars_lane_turn(
            card, cards, player, &scan, state, mechanism, branch, lang, hoist,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn assemble_gm_messages(
        world_md: &str,
        cards: &[CharacterCard],
        player: Option<&CharacterCard>,
        events: &[TranscriptEvent],
        worldbook: &[WorldbookEntry],
        state: &TableState,
        mechanism: &Mechanism,
        scope: &StateScope,
        lang: &str,
    ) -> Vec<ChatMessage> {
        let scan = prepared(worldbook, Viewer::Gm, world_md, cards, player, events, lang);
        transport::assemble_gm_messages(cards, player, events, &scan, state, mechanism, scope, lang)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn assemble_shared_messages(
        card: &CharacterCard,
        cards: &[CharacterCard],
        player: Option<&CharacterCard>,
        events: &[TranscriptEvent],
        worldbook: &[WorldbookEntry],
        state: &TableState,
        mechanism: &Mechanism,
        branch: Option<&[String]>,
        lang: &str,
    ) -> Vec<ChatMessage> {
        let scan = scan_of(
            worldbook,
            Viewer::Character(card),
            cards,
            player,
            events,
            lang,
        );
        transport::assemble_shared_messages(
            card, cards, player, events, &scan, state, mechanism, branch, lang,
        )
    }
}
