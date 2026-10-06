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
