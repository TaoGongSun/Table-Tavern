//! 美術一覽的背景分頁（plans/vn-setup-wizard.md §4）：整桌地點清單（有圖的、記過名的、卡宣告的、佇列裡的）、
//! 以 slug 指定的生成目標與上傳替換。
use super::{valid_slug, Backdrop, Entry, Library, Places};
use crate::data;
use crate::data::vn_art::valid_art_key;
use serde::Serialize;
use std::collections::BTreeSet;

/// 一個地點：`location`＝顯示名（認不出名字的舊圖＝None，只能看）；
/// `base`＝已生好的基底時段（還沒有＝None）；`dedicated`＝有自己圖檔的時段（含基底）
#[derive(Debug, PartialEq, Eq, Serialize)]
pub(crate) struct Place {
    pub slug: String,
    pub location: Option<String>,
    pub periods: Vec<String>,
    pub base: Option<String>,
    pub dedicated: Vec<String>,
}

/// 鍵驗證：slug 格式、時段在這個地點的有效清單內（卡地點可有自訂時段）；回該地點的時段清單
pub(crate) fn checked(places: &Places, slug: &str, period: &str) -> Result<Vec<String>, String> {
    let periods = places.periods(slug);
    match valid_slug(slug) && periods.iter().any(|known| known == period) {
        true => Ok(periods),
        false => Err("VN_BACKGROUND_KEY".to_owned()),
    }
}

impl Library {
    /// 庫裡有圖檔的 slug（檔名 `<slug>-<時段>.png`，格式不對的略過）
    fn slugs(&self) -> BTreeSet<String> {
        let names = std::fs::read_dir(&self.dir).into_iter().flatten().flatten();
        names
            .filter_map(|entry| {
                let name = entry.file_name().into_string().ok()?;
                let (slug, period) = name.strip_suffix(".png")?.split_once('-')?;
                (valid_slug(slug) && valid_art_key(period)).then(|| slug.to_owned())
            })
            .collect()
    }

    /// 地點名：卡地點用卡上的名稱，自由地點用 `backgrounds.json` 記的原文，再沒有看佇列裡的工作
    fn location(&self, places: &Places, pending: &[Backdrop], slug: &str) -> Option<String> {
        places
            .by_slug(slug)
            .map(|place| place.name.clone())
            .or_else(|| {
                self.entries().remove(slug).map(|entry| match entry {
                    Entry::Name(name) | Entry::Place { name, .. } => name,
                })
            })
            .or_else(|| {
                pending
                    .iter()
                    .find(|job| job.slug == slug)
                    .map(|job| job.location.clone())
            })
    }

    /// 卡宣告的地點照卡上順序在前（沒生過圖也列，玩家可手動生），其餘（有圖、記過名、佇列中）依地名排
    pub(crate) fn places(&self, places: &Places, pending: &[Backdrop]) -> Vec<Place> {
        let declared: Vec<String> = places.slugs();
        let mut others: Vec<String> = self
            .slugs()
            .into_iter()
            .chain(self.entries().into_keys())
            .chain(pending.iter().map(|job| job.slug.clone()))
            .filter(|slug| !declared.contains(slug))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        others.sort_by_cached_key(|slug| self.location(places, pending, slug));
        declared
            .into_iter()
            .chain(others)
            .map(|slug| {
                let periods = places.periods(&slug);
                Place {
                    location: self.location(places, pending, &slug),
                    base: self.base_period(&slug, &periods),
                    dedicated: periods
                        .iter()
                        .filter(|period| self.has(&slug, period))
                        .cloned()
                        .collect(),
                    periods,
                    slug,
                }
            })
            .collect()
    }

    /// 一覽按生成／重生成／上傳的目標：身分是 slug（不從地名反推，卡地點與自由地點同名也不會對錯），地名只進提示詞
    pub(crate) fn target(
        &self,
        places: &Places,
        pending: &[Backdrop],
        slug: &str,
        period: &str,
    ) -> Result<Backdrop, String> {
        checked(places, slug, period)?;
        let location = self
            .location(places, pending, slug)
            .ok_or_else(|| "VN_BACKGROUND_KEY".to_owned())?;
        Ok(Backdrop {
            location,
            slug: slug.to_owned(),
            period: period.to_owned(),
        })
    }
}

/// 上傳替換的提交段（呼叫端持桌寫入許可）：重驗桌在、非重播、鍵，寫圖並取消同鍵排隊中的工作（免得蓋回）。
/// 地點還沒有基底時這張只當專屬圖（除非就是時段清單第一項），基底照舊等自動補；回寫入的鍵
pub(crate) fn commit_upload(
    root: &std::path::Path,
    world: &str,
    places: &Places,
    pending: &[Backdrop],
    slug: &str,
    period: &str,
    png: &[u8],
) -> Result<Backdrop, String> {
    data::read_state(root, world).map_err(|error| error.to_string())?;
    data::vn::refuse_if_replay(root, world)?;
    let library = Library::of(root, world)?;
    let backdrop = library.target(places, pending, slug, period)?;
    let base = library.intended_base(slug, &places.periods(slug));
    library.write(&backdrop, base.as_deref(), png)?;
    crate::vn_sprites::queue::cancel_background(world, slug, period);
    Ok(backdrop)
}
