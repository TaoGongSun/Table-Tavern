use std::fs;
use std::path::Path;

use super::super::paths::world_dir;
use super::super::state::read_state;
use super::super::{local_timestamp, DataResult};
use super::marker::{event_full_text, lang_key, player_fallback_name};
use super::transcript::{read_transcript, transcript_path, TranscriptEvent, TranscriptKind};
use crate::ui_msg::UiMsg;

/// 匯出檔的固定文字（十語系）。`{world}`／`{scene}`／`{time}` 單次代入。
struct ExportCopy {
    session_title: &'static str,
    scene_title: &'static str,
    scene_heading: &'static str,
    exported: &'static str,
    /// 發言者名與台詞之間
    separator: &'static str,
    /// 系統事件外框
    aside: (&'static str, &'static str),
}

fn export_copy(lang: &str) -> ExportCopy {
    const CJK_ASIDE: (&str, &str) = ("（", "）");
    const ASIDE: (&str, &str) = ("(", ")");
    match lang_key(lang) {
        "zh-TW" => ExportCopy {
            session_title: "{world} 跑團紀錄",
            scene_title: "{world} 場景 {scene}",
            scene_heading: "場景 {scene}",
            exported: "匯出時間：{time}",
            separator: "：",
            aside: CJK_ASIDE,
        },
        "zh-CN" => ExportCopy {
            session_title: "{world} 跑团记录",
            scene_title: "{world} 场景 {scene}",
            scene_heading: "场景 {scene}",
            exported: "导出时间：{time}",
            separator: "：",
            aside: CJK_ASIDE,
        },
        "ja" => ExportCopy {
            session_title: "{world} セッション記録",
            scene_title: "{world} シーン {scene}",
            scene_heading: "シーン {scene}",
            exported: "エクスポート日時：{time}",
            separator: "：",
            aside: CJK_ASIDE,
        },
        "ko" => ExportCopy {
            session_title: "{world} 세션 기록",
            scene_title: "{world} 장면 {scene}",
            scene_heading: "장면 {scene}",
            exported: "내보낸 시각: {time}",
            separator: ": ",
            aside: ASIDE,
        },
        "es" => ExportCopy {
            session_title: "{world} — Registro de la partida",
            scene_title: "{world} — Escena {scene}",
            scene_heading: "Escena {scene}",
            exported: "Exportado: {time}",
            separator: ": ",
            aside: ASIDE,
        },
        "pt-BR" => ExportCopy {
            session_title: "{world} — Registro da sessão",
            scene_title: "{world} — Cena {scene}",
            scene_heading: "Cena {scene}",
            exported: "Exportado em: {time}",
            separator: ": ",
            aside: ASIDE,
        },
        "de" => ExportCopy {
            session_title: "{world} — Sitzungsprotokoll",
            scene_title: "{world} — Szene {scene}",
            scene_heading: "Szene {scene}",
            exported: "Exportiert: {time}",
            separator: ": ",
            aside: ASIDE,
        },
        "fr" => ExportCopy {
            session_title: "{world} — Compte rendu de partie",
            scene_title: "{world} — Scène {scene}",
            scene_heading: "Scène {scene}",
            exported: "Exporté le\u{a0}: {time}",
            separator: "\u{a0}: ",
            aside: ASIDE,
        },
        "ru" => ExportCopy {
            session_title: "{world} — Журнал сессии",
            scene_title: "{world} — Сцена {scene}",
            scene_heading: "Сцена {scene}",
            exported: "Экспортировано: {time}",
            separator: ": ",
            aside: ASIDE,
        },
        _ => ExportCopy {
            session_title: "{world} — Session Transcript",
            scene_title: "{world} — Scene {scene}",
            scene_heading: "Scene {scene}",
            exported: "Exported: {time}",
            separator: ": ",
            aside: ASIDE,
        },
    }
}

/// 單次代入：先代入的值（例如桌名）裡就算有 `{scene}` 也不會再被換。
fn fill(template: &str, values: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let tail = &rest[start..];
        match values
            .iter()
            .find(|(key, _)| tail.starts_with(&format!("{{{key}}}")))
        {
            Some((key, value)) => {
                out.push_str(value);
                rest = &tail[key.len() + 2..];
            }
            None => {
                out.push('{');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// 標題列＋匯出時間。
fn export_header(title: String, copy: &ExportCopy, timestamp: &str) -> String {
    format!(
        "# {title}

{}",
        fill(copy.exported, &[("time", timestamp)])
    )
}

/// 把單一事件渲染成一行（或多行）Markdown，整桌／單場匯出共用同一份格式。
/// 事件本文帶標頭代碼時照匯出語系組標頭；沒有名字的玩家發言退回該語系的玩家稱呼。
fn render_transcript_entry(event: &TranscriptEvent, lang: &str, copy: &ExportCopy) -> String {
    let text = event_full_text(event, lang);
    match event.kind {
        TranscriptKind::Dialogue | TranscriptKind::Player => {
            let speaker =
                if event.kind == TranscriptKind::Player && event.speaker_name.trim().is_empty() {
                    player_fallback_name(lang)
                } else {
                    event.speaker_name.as_str()
                };
            format!("**{speaker}**{}{text}", copy.separator)
        }
        TranscriptKind::Narration => {
            if text.is_empty() {
                "> ".to_owned()
            } else {
                text.lines()
                    .map(|line| format!("> {line}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        }
        TranscriptKind::System => format!("*{}{text}{}*", copy.aside.0, copy.aside.1),
    }
}

fn render_entries(events: &[TranscriptEvent], lang: &str, copy: &ExportCopy) -> Vec<String> {
    events
        .iter()
        .map(|event| render_transcript_entry(event, lang, copy))
        .collect()
}

pub fn export_transcript_markdown(root: &Path, world_id: &str, lang: &str) -> DataResult<String> {
    let world_name = read_state(root, world_id)?.name;
    let transcript_dir = world_dir(root, world_id)?.join("transcript");
    if !transcript_dir.is_dir() {
        return Err(UiMsg::TranscriptEmpty.into_error());
    }

    let mut scenes = Vec::new();
    for entry in fs::read_dir(transcript_dir)? {
        let entry = entry?;
        let path = entry.path();
        let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
            continue;
        };
        if path.extension().and_then(|value| value.to_str()) != Some("jsonl") {
            continue;
        }
        if let Ok(scene) = stem.parse::<u64>() {
            scenes.push(scene);
        }
    }
    scenes.sort_unstable();
    scenes.dedup();
    if scenes.is_empty() {
        return Err(UiMsg::TranscriptEmpty.into_error());
    }

    let copy = export_copy(lang);
    let title = export_header(
        fill(copy.session_title, &[("world", &world_name)]),
        &copy,
        &local_timestamp()?,
    );
    let mut sections = Vec::new();
    for scene in scenes {
        let scene_text = scene.to_string();
        let heading = format!("## {}", fill(copy.scene_heading, &[("scene", &scene_text)]));
        let entries = render_entries(&read_transcript(root, world_id, scene)?, lang, &copy);
        sections.push(if entries.is_empty() {
            heading
        } else {
            format!("{heading}\n\n{}", entries.join("\n\n"))
        });
    }

    Ok(format!("{title}\n\n{}\n", sections.join("\n\n")))
}

/// 匯出單一場景的紀錄，格式與整桌匯出一致，供「過去的場」單場匯出使用。
/// 場景不存在（無該檔）視為錯誤，避免誤匯出空白文件。
pub fn export_scene_markdown(
    root: &Path,
    world_id: &str,
    scene: u64,
    lang: &str,
) -> DataResult<String> {
    let path = transcript_path(root, world_id, scene)?;
    if !path.exists() {
        return Err(UiMsg::SceneNotFound { scene }.into_error());
    }

    let world_name = read_state(root, world_id)?.name;
    let copy = export_copy(lang);
    let scene_text = scene.to_string();
    let title = export_header(
        fill(
            copy.scene_title,
            &[("world", &world_name), ("scene", &scene_text)],
        ),
        &copy,
        &local_timestamp()?,
    );
    let entries = render_entries(&read_transcript(root, world_id, scene)?, lang, &copy);
    Ok(format!("{title}\n\n{}\n", entries.join("\n\n")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::test_support::*;
    use crate::data::*;

    #[test]
    fn exports_all_transcript_scenes_as_localized_markdown() {
        let root = TestRoot::new("transcript-export");
        let world_id = create_world(root.path(), "海風桌").unwrap();
        for (scene, event) in [
            (
                1,
                TranscriptEvent {
                    raw: None,
                    ts: "now".to_owned(),
                    speaker_id: "船長代碼".to_owned(),
                    speaker_name: "船長".to_owned(),
                    kind: TranscriptKind::Dialogue,
                    text: "我們啟航。".to_owned(),
                    state: None,
                    truncated: false,
                    gm_only: false,
                    marker: None,
                },
            ),
            (
                0,
                TranscriptEvent {
                    raw: None,
                    ts: "now".to_owned(),
                    speaker_id: String::new(),
                    speaker_name: "GM".to_owned(),
                    kind: TranscriptKind::Narration,
                    text: "霧氣升起。\n港口安靜。".to_owned(),
                    state: None,
                    truncated: false,
                    gm_only: false,
                    marker: None,
                },
            ),
            (
                1,
                TranscriptEvent {
                    raw: None,
                    ts: "now".to_owned(),
                    speaker_id: String::new(),
                    speaker_name: "玩家".to_owned(),
                    kind: TranscriptKind::Player,
                    text: "我登上甲板。".to_owned(),
                    state: None,
                    truncated: false,
                    gm_only: false,
                    marker: None,
                },
            ),
            (
                0,
                TranscriptEvent {
                    raw: None,
                    ts: "now".to_owned(),
                    speaker_id: String::new(),
                    speaker_name: "GM".to_owned(),
                    kind: TranscriptKind::System,
                    text: "第一幕開始".to_owned(),
                    state: None,
                    truncated: false,
                    gm_only: false,
                    marker: None,
                },
            ),
        ] {
            append_transcript(root.path(), &world_id, scene, &event).unwrap();
        }

        let zh = export_transcript_markdown(root.path(), &world_id, "zh-TW").unwrap();
        assert!(zh.starts_with("# 海風桌 跑團紀錄\n\n匯出時間："));
        assert!(zh.find("## 場景 0").unwrap() < zh.find("## 場景 1").unwrap());
        assert!(zh.contains("> 霧氣升起。\n> 港口安靜。"));
        assert!(zh.contains("*（第一幕開始）*"));
        assert!(zh.contains("**玩家**：我登上甲板。"));
        assert!(zh.contains("**船長**：我們啟航。"));

        let en = export_transcript_markdown(root.path(), &world_id, "en").unwrap();
        assert!(en.starts_with("# 海風桌 — Session Transcript\n\nExported: "));
        assert!(en.contains("## Scene 0"));
        assert!(en.contains("## Scene 1"));
        assert!(en.contains("**船長**: 我們啟航。"));
        assert!(en.contains("*(第一幕開始)*"));
    }

    #[test]
    fn transcript_export_rejects_a_world_without_scenes() {
        let root = TestRoot::new("empty-transcript-export");
        let world_id = create_world(root.path(), "空桌").unwrap();
        assert!(export_transcript_markdown(root.path(), &world_id, "zh-TW").is_err());
    }

    #[test]
    fn scene_export_contains_only_that_scenes_events() {
        let root = TestRoot::new("scene-export");
        let world_id = create_world(root.path(), "海風桌").unwrap();
        for (scene, event) in [
            (
                0,
                TranscriptEvent {
                    raw: None,
                    ts: "now".to_owned(),
                    speaker_id: String::new(),
                    speaker_name: "GM".to_owned(),
                    kind: TranscriptKind::Narration,
                    text: "霧氣升起。".to_owned(),
                    state: None,
                    truncated: false,
                    gm_only: false,
                    marker: None,
                },
            ),
            (
                1,
                TranscriptEvent {
                    raw: None,
                    ts: "now".to_owned(),
                    speaker_id: "船長代碼".to_owned(),
                    speaker_name: "船長".to_owned(),
                    kind: TranscriptKind::Dialogue,
                    text: "我們啟航。".to_owned(),
                    state: None,
                    truncated: false,
                    gm_only: false,
                    marker: None,
                },
            ),
        ] {
            append_transcript(root.path(), &world_id, scene, &event).unwrap();
        }

        let zh = export_scene_markdown(root.path(), &world_id, 0, "zh-TW").unwrap();
        assert!(zh.starts_with("# 海風桌 場景 0\n\n匯出時間："));
        assert!(zh.contains("> 霧氣升起。"));
        assert!(!zh.contains("船長"));

        let en = export_scene_markdown(root.path(), &world_id, 1, "en").unwrap();
        assert!(en.starts_with("# 海風桌 — Scene 1\n\nExported: "));
        assert!(en.contains("**船長**: 我們啟航。"));
        assert!(!en.contains("霧氣升起"));
    }

    fn marked(
        kind: TranscriptKind,
        marker: Option<EventMarker>,
        speaker: &str,
        text: &str,
    ) -> TranscriptEvent {
        TranscriptEvent {
            raw: None,
            ts: "now".to_owned(),
            speaker_id: String::new(),
            speaker_name: speaker.to_owned(),
            kind,
            text: text.to_owned(),
            state: None,
            truncated: false,
            gm_only: false,
            marker,
        }
    }

    /// 十語系匯出：標題、匯出時間、場景標頭、分隔符逐字；標頭代碼照匯出語系組；沒名字的玩家退回該語系稱呼。
    #[test]
    fn exports_follow_the_export_language_for_every_fixed_text() {
        let root = TestRoot::new("transcript-export-langs");
        let world_id = create_world(root.path(), "桌").unwrap();
        for event in [
            marked(
                TranscriptKind::Narration,
                Some(EventMarker::SceneSummary),
                "GM",
                "摘要",
            ),
            marked(
                TranscriptKind::System,
                Some(EventMarker::CardArrival {
                    name: "狐狸".to_owned(),
                }),
                "GM",
                "尾巴",
            ),
            marked(TranscriptKind::Player, None, "", "嗨"),
        ] {
            append_transcript(root.path(), &world_id, 0, &event).unwrap();
        }
        let cases = [
            (
                "zh-TW",
                "# 桌 跑團紀錄\n\n匯出時間：",
                "## 場景 0",
                "> 【前情提要】\n> 摘要",
                "*（（角色回歸）〈狐狸〉\n公開設定：\n尾巴）*",
                "**玩家**：嗨",
            ),
            (
                "zh-CN",
                "# 桌 跑团记录\n\n导出时间：",
                "## 场景 0",
                "> 【前情提要】",
                "*（（角色回归）〈狐狸〉\n公开设定：\n尾巴）*",
                "**玩家**：嗨",
            ),
            (
                "en",
                "# 桌 — Session Transcript\n\nExported: ",
                "## Scene 0",
                "> Previously:",
                "*((Character returns) “狐狸”\nPublic profile:\n尾巴)*",
                "**Player**: 嗨",
            ),
            (
                "ja",
                "# 桌 セッション記録\n\nエクスポート日時：",
                "## シーン 0",
                "> 【前回までのあらすじ】",
                "*（（キャラクター再登場）〈狐狸〉\n公開設定：\n尾巴）*",
                "**プレイヤー**：嗨",
            ),
            (
                "ko",
                "# 桌 세션 기록\n\n내보낸 시각: ",
                "## 장면 0",
                "> 【지난 이야기】",
                "*((캐릭터 복귀) 〈狐狸〉\n공개 설정:\n尾巴)*",
                "**플레이어**: 嗨",
            ),
            (
                "es",
                "# 桌 — Registro de la partida\n\nExportado: ",
                "## Escena 0",
                "> Anteriormente:",
                "*((Regresa un personaje) «狐狸»\nPerfil público:\n尾巴)*",
                "**Jugador**: 嗨",
            ),
            (
                "pt-BR",
                "# 桌 — Registro da sessão\n\nExportado em: ",
                "## Cena 0",
                "> Anteriormente:",
                "*((Personagem retorna) “狐狸”\nPerfil público:\n尾巴)*",
                "**Jogador**: 嗨",
            ),
            (
                "de",
                "# 桌 — Sitzungsprotokoll\n\nExportiert: ",
                "## Szene 0",
                "> Bisher:",
                "*((Figur kehrt zurück) „狐狸“\nÖffentliches Profil:\n尾巴)*",
                "**Spieler**: 嗨",
            ),
            (
                "fr",
                "# 桌 — Compte rendu de partie\n\nExporté le\u{a0}: ",
                "## Scène 0",
                "> Précédemment\u{a0}:",
                "*((Retour d’un personnage) «\u{a0}狐狸\u{a0}»\nProfil public\u{a0}:\n尾巴)*",
                "**Joueur**\u{a0}: 嗨",
            ),
            (
                "ru",
                "# 桌 — Журнал сессии\n\nЭкспортировано: ",
                "## Сцена 0",
                "> Ранее:",
                "*((Персонаж возвращается) «狐狸»\nОткрытый профиль:\n尾巴)*",
                "**Игрок**: 嗨",
            ),
        ];
        for (lang, header, heading, summary, arrival, player) in cases {
            let markdown = export_transcript_markdown(root.path(), &world_id, lang).unwrap();
            assert!(markdown.starts_with(header), "{lang}: {markdown}");
            for part in [heading, summary, arrival, player] {
                assert!(markdown.contains(part), "{lang} 缺 {part}: {markdown}");
            }
        }
        let scene = export_scene_markdown(root.path(), &world_id, 0, "de").unwrap();
        assert!(scene.starts_with("# 桌 — Szene 0\n\nExportiert: "));
    }

    /// 桌名含佔位符字樣也不會被二次代入。
    #[test]
    fn world_name_with_placeholder_text_is_not_substituted_twice() {
        assert_eq!(
            fill(
                "{world} 場景 {scene}",
                &[("world", "{scene}桌"), ("scene", "3")]
            ),
            "{scene}桌 場景 3"
        );
        assert_eq!(fill("{other} {time}", &[("time", "t")]), "{other} t");
    }

    #[test]
    fn scene_export_rejects_a_missing_scene() {
        let root = TestRoot::new("scene-export-missing");
        let world_id = create_world(root.path(), "空桌").unwrap();
        assert!(export_scene_markdown(root.path(), &world_id, 0, "zh-TW").is_err());
    }
}
