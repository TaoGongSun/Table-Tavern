//! 自帶底線（vendor-prefix-floor）：各 CLI／供應商會快取自己的固定開頭，那一段與我方送什麼無關，
//! 冷啟動也會中。從帳本裡把那個固定值認出來，額度分頁才分得出「我方內容中了」與「只中到自帶部分」。
//!
//! 純統計，不讀檔不寫檔：呼叫端（report）決定哪些行算樣本，這裡只依分組算底線。
//!
//! 判準：組內真實回報的正值裡，**出現 ≥3 次、且對上至少兩種不同 prompt_tokens** 的值當候選，
//! 取最小者。0 不當底線（那是沒中，不是自帶段）；沒有合格值＝未辨識，畫面維持現行。
//! - 要兩種輸入量：同一段輸入重送三次會中同樣的量，那是我方內容真的中了，不是自帶段。
//! - 取最小而不是眾數：共線穩定成功時成功量通常大於自帶段，眾數會被成功量搶走。
//!
//! 兩道護欄擋「我方共線前綴穩定命中」被誤認成底線——沒有自帶段的路，成功量本來就會在不同輸入量下
//! 重複出現同一個值（2026-10-28 重放正式帳本：API muse-spark 的 12,337×3 底下還有 15 筆更小的真命中）：
//! - 候選值底下的正值超過一成（至少容許一筆偶發，例：codex 換角色暖的 6,912）就不認：
//!   自帶段排在最前面，正常只會偶發地中得比它少。
//! - API 的底線上限 128（一個量化區塊）：API 供應商不加自帶提示詞，觀察到的固定小值只是計量零頭
//!   （ox-alpha 的 64），大於此的重複值是我方內容。

use std::collections::{BTreeMap, BTreeSet, HashMap};

/// 底線出現次數門檻。
const MIN_OCCURRENCES: usize = 3;
/// 底線要對上幾種不同的總輸入量。
const MIN_DISTINCT_PROMPTS: usize = 2;
/// 底線底下容許的正值比例（以十分之一計）；至少容許一筆。
const MAX_BELOW_TENTHS: usize = 1;
/// API 底線的上限：一個量化區塊。
const API_FLOOR_CAP: u64 = 128;

/// 分組鍵：（通道, 模型, 來源）。來源在 CLI 是 `cli_version`、在 API 是 `endpoint`（`主機:連接埠`）；
/// 舊行或取不到值的一律空字串，歸「版本未知／端點未知」那一組。
pub type FloorKey = (String, String, String);

/// 一組的底線：未辨識（樣本不足或沒有合格值）或已辨識的 token 數。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Floor {
    Unknown,
    Known(u64),
}

/// 逐行收樣本，收完一次算出每組底線。
#[derive(Debug, Default)]
pub struct FloorSamples {
    /// 組 → 讀到的快取量 → 對上過的總輸入量
    groups: HashMap<FloorKey, BTreeMap<u64, Vec<u64>>>,
}

impl FloorSamples {
    /// 收一筆真實回報的樣本。0 不收：讀到 0 是沒中，跟自帶段無關。
    pub fn add(&mut self, key: FloorKey, cached: u64, prompt: u64) {
        if cached == 0 {
            return;
        }
        self.groups
            .entry(key)
            .or_default()
            .entry(cached)
            .or_default()
            .push(prompt);
    }

    pub fn finish(self) -> Floors {
        let known = self
            .groups
            .into_iter()
            .filter_map(|(key, values)| {
                let floor = floor_of(&key, &values)?;
                Some((key, floor))
            })
            .collect();
        Floors { known }
    }
}

/// 一組的底線：最小的合格候選，再過兩道護欄；不過就整組未辨識（更大的候選底下只會更多）。
fn floor_of(key: &FloorKey, values: &BTreeMap<u64, Vec<u64>>) -> Option<u64> {
    // BTreeMap 由小到大，第一個合格的就是最小值
    let floor = values.iter().find(|(_, prompts)| {
        prompts.len() >= MIN_OCCURRENCES
            && prompts.iter().collect::<BTreeSet<_>>().len() >= MIN_DISTINCT_PROMPTS
    })?;
    let floor = *floor.0;
    let total: usize = values.values().map(Vec::len).sum();
    let below: usize = values
        .range(..floor)
        .map(|(_, prompts)| prompts.len())
        .sum();
    if below * 10 > (total * MAX_BELOW_TENTHS).max(10) {
        return None;
    }
    if key.0 == "api" && floor > API_FLOOR_CAP {
        return None;
    }
    Some(floor)
}

#[derive(Debug, Default)]
pub struct Floors {
    known: HashMap<FloorKey, u64>,
}

impl Floors {
    pub fn get(&self, key: &FloorKey) -> Floor {
        match self.known.get(key) {
            Some(&floor) => Floor::Known(floor),
            None => Floor::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(origin: &str) -> FloorKey {
        ("codex".to_owned(), "gpt".to_owned(), origin.to_owned())
    }

    fn floors(samples: &[(&str, u64, u64)]) -> Floors {
        let mut collected = FloorSamples::default();
        for (origin, cached, prompt) in samples {
            collected.add(key(origin), *cached, *prompt);
        }
        collected.finish()
    }

    /// 10-28 實測序列：自帶 11,008 出現 7 次、換角色暖一次 6,912——取不到最小的偶發值
    #[test]
    fn floor_is_the_smallest_value_repeated_across_inputs() {
        let samples = [
            ("0.164.0", 11_008, 18_478),
            ("0.164.0", 11_008, 18_621),
            ("0.164.0", 11_008, 14_578),
            ("0.164.0", 6_912, 18_920),
            ("0.164.0", 11_008, 15_187),
            ("0.164.0", 11_008, 15_336),
        ];
        assert_eq!(floors(&samples).get(&key("0.164.0")), Floor::Known(11_008));
    }

    /// 成功量出現得比自帶量多（眾數會選錯），最小合格值仍是自帶段
    #[test]
    fn frequent_success_does_not_displace_the_floor() {
        let mut samples = vec![
            ("v", 9_984, 20_000),
            ("v", 9_984, 21_000),
            ("v", 9_984, 22_000),
        ];
        for prompt in 0..6 {
            samples.push(("v", 19_200, 30_000 + prompt));
        }
        assert_eq!(floors(&samples).get(&key("v")), Floor::Known(9_984));
    }

    /// 同一段輸入重送三次中同樣的量＝我方內容真的中了，不是自帶段
    #[test]
    fn identical_repeats_of_one_input_are_not_a_floor() {
        let samples = [
            ("v", 4_096, 8_000),
            ("v", 4_096, 8_000),
            ("v", 4_096, 8_000),
        ];
        assert_eq!(floors(&samples).get(&key("v")), Floor::Unknown);
    }

    /// 0 再多也不是底線；樣本不足兩次也不判
    #[test]
    fn zeros_and_thin_samples_leave_the_group_unknown() {
        let samples = [
            ("v", 0, 1_000),
            ("v", 0, 2_000),
            ("v", 0, 3_000),
            ("v", 2_048, 4_000),
            ("v", 2_048, 5_000),
        ];
        assert_eq!(floors(&samples).get(&key("v")), Floor::Unknown);
    }

    /// 不同版本各算各的：新版樣本不足時不拿舊版底線來判
    #[test]
    fn versions_are_separate_groups() {
        let samples = [
            ("old", 9_984, 1),
            ("old", 9_984, 2),
            ("old", 9_984, 3),
            ("new", 11_008, 1),
            ("new", 11_008, 2),
        ];
        let floors = floors(&samples);
        assert_eq!(floors.get(&key("old")), Floor::Known(9_984));
        assert_eq!(floors.get(&key("new")), Floor::Unknown);
    }

    /// 照讀到量序列建一組（每筆輸入量都不同），回這組的底線。
    fn replay(transport: &str, cached: &[u64]) -> Floor {
        let key = (transport.to_owned(), "m".to_owned(), String::new());
        let mut samples = FloorSamples::default();
        for (index, &value) in cached.iter().enumerate() {
            samples.add(key.clone(), value, 10_000 + index as u64);
        }
        samples.finish().get(&key)
    }

    fn repeat(value: u64, times: usize) -> Vec<u64> {
        vec![value; times]
    }

    /// 2026-10-28 重放正式帳本（只讀）的各組讀到量：自帶段判對，我方共線前綴的重複值不認
    #[test]
    fn production_ledger_groups_are_judged_right() {
        // codex 舊版：9,984×12，其餘是共線真的中
        let mut codex = repeat(9_984, 12);
        codex.extend([
            19_200, 19_200, 19_200, 18_176, 18_176, 20_224, 12_032, 14_080,
        ]);
        assert_eq!(replay("codex", &codex), Floor::Known(9_984));
        // codex 0.164.0（實測佇列第 14 項）：11,008×7，底下偶發一筆 6,912
        let mut new_codex = repeat(11_008, 7);
        new_codex.push(6_912);
        assert_eq!(replay("codex", &new_codex), Floor::Known(11_008));
        let mut grok = repeat(128, 17);
        grok.extend([4_224, 4_224, 4_224, 9_856, 10_752, 16_128, 18_560]);
        assert_eq!(replay("grok", &grok), Floor::Known(128));
        let mut ox = repeat(64, 58);
        ox.extend([7_680, 2_176, 11_264, 2_240, 17_600]);
        assert_eq!(replay("api", &ox), Floor::Known(64));
    }

    /// 我方共線前綴穩定命中的路：重複值底下還有大量更小的真命中，不認
    #[test]
    fn growing_shared_prefix_hits_are_not_a_floor() {
        let muse = [
            4_465, 4_273, 5_873, 5_681, 7_281, 7_089, 11_249, 11_057, 11_569, 12_337, 12_337,
            12_337, 13_041, 13_681, 14_065, 2_161, 3_121, 4_593, 5_745, 9_521, 11_121,
        ];
        assert_eq!(replay("api", &muse), Floor::Unknown);
        // claude-opus-4-6 舊 single 行：11,266×10，底下 20 筆更小
        let mut opus = repeat(11_266, 10);
        opus.extend((0..20).map(|index| 4_000 + index * 100));
        opus.extend((0..95).map(|index| 20_000 + index * 128));
        assert_eq!(replay("claude", &opus), Floor::Unknown);
    }

    /// API 只認量化零頭：union-alpha 的 2,028×4 是我方幾乎整段 system，不是供應商自帶
    #[test]
    fn api_floor_is_capped_at_one_block() {
        assert_eq!(replay("api", &repeat(2_028, 4)), Floor::Unknown);
        assert_eq!(replay("api", &repeat(128, 3)), Floor::Known(128));
        assert_eq!(replay("codex", &repeat(2_028, 4)), Floor::Known(2_028));
    }

    /// 已知限制（暫態、自癒）：新版本剛上線、自帶段還沒滿 3 次時，成功量可能先被當底線
    /// （Sol 反例：11,008×1＋18,176×3）；自帶段每次冷啟動都會出現，補滿 3 次後最小合格值
    /// 自動回到自帶段，讀帳時舊輪一併重判
    #[test]
    fn early_success_floor_heals_once_the_vendor_part_repeats() {
        let mut cached = vec![11_008, 18_176, 18_176, 18_176];
        assert_eq!(replay("codex", &cached), Floor::Known(18_176));
        cached.extend([11_008, 11_008]);
        assert_eq!(replay("codex", &cached), Floor::Known(11_008));
    }
}
