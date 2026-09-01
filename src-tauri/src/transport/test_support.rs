use crate::data::{CharacterCard, Tier, TranscriptEvent, TranscriptKind, Visibility, WorldbookEntry};

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
        raw: None,
        ts: "2026-07-19T12:00:00+08:00".to_owned(),
        speaker_id: speaker_id.to_owned(),
        speaker_name: speaker_name.to_owned(),
        kind,
        text: text.to_owned(),
        state: None,
        gm_only: false,
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
