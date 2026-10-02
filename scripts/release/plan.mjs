// 發版彙整的純邏輯。搬檔、叫 minisign、叫 gh 都在 finalize.mjs。

const ENDPOINT =
  /^https:\/\/github\.com\/([^/]+)\/([^/]+)\/releases\/latest\/download\/latest\.json$/;

// 數字與預發布段的數字識別不允許前導零；build metadata（+ 之後）允許。
const SEMVER = new RegExp(
  "^(0|[1-9]\\d*)\\.(0|[1-9]\\d*)\\.(0|[1-9]\\d*)" +
    "(?:-([0-9A-Za-z-]+(?:\\.[0-9A-Za-z-]+)*))?" +
    "(?:\\+([0-9A-Za-z-]+(?:\\.[0-9A-Za-z-]+)*))?$",
);

function identOk(ident) {
  if (ident == null) return true;
  return ident.split(".").every((part) => part !== "" && !/^0\d+$/.test(part));
}

/** @returns {boolean} 有預發布段就是 true。不是 SemVer 就丟錯。 */
export function hasPrerelease(version) {
  const match = SEMVER.exec(version);
  if (!match || !identOk(match[4])) {
    throw new Error(`不是 SemVer：${version}`);
  }
  return match[4] != null;
}

export function repoFromEndpoint(endpoint) {
  const match = ENDPOINT.exec(endpoint);
  if (!match) return null;
  const owner = match[1];
  const repo = match[2];
  return {
    owner,
    repo,
    slug: `${owner}/${repo}`,
    repoBase: `https://github.com/${owner}/${repo}`,
  };
}

export function readUpdaterConfig(conf) {
  const updater = conf?.plugins?.updater;
  if (!updater || typeof updater !== "object") {
    throw new Error("tauri.conf.json 沒有 plugins.updater");
  }
  const endpoints = updater.endpoints;
  if (!Array.isArray(endpoints) || endpoints.length !== 1) {
    throw new Error("plugins.updater.endpoints 應恰好一個網址");
  }
  const repo = repoFromEndpoint(endpoints[0]);
  if (!repo) {
    throw new Error(
      "plugins.updater.endpoints 不是預期的 GitHub latest.json 網址：" +
        String(endpoints[0]),
    );
  }
  return { pubkey: updater.pubkey, endpoint: endpoints[0], ...repo };
}

/** Tauri 把 minisign 檔再 base64 一次。解碼後應以 untrusted comment: 開頭。 */
export function decodeMinisignArmor(label, text) {
  const trimmed = String(text ?? "").trim();
  if (!/^[A-Za-z0-9+/]+={0,2}$/.test(trimmed)) {
    throw new Error(
      `${label} 不是 base64。Tauri 的 .sig 與 pubkey 都是 base64，` +
        "解碼後才是 minisign 檔。",
    );
  }
  const decoded = Buffer.from(trimmed, "base64").toString("utf8");
  if (!decoded.startsWith("untrusted comment:")) {
    throw new Error(
      `${label} 解碼後不是 minisign 檔（應以 untrusted comment: 開頭）。`,
    );
  }
  return decoded.endsWith("\n") ? decoded : `${decoded}\n`;
}

function escapeRegExp(text) {
  return text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

/**
 * 從 `## [版本]` 標題的下一行收到下一個二級標題之前。
 * 標題可以是 `## [0.2.0] — 2026-07-24（內部測試版）` 這種後面帶日期的形式。
 * 回傳的 notes 不含標題本身，頭尾空白去掉。
 */
export function extractChangelogNotes(markdown, version) {
  const lines = String(markdown).replaceAll("\r\n", "\n").replaceAll("\r", "\n").split("\n");
  const heading = new RegExp(`^## \\[${escapeRegExp(version)}\\](?:\\s.*)?$`);
  const start = lines.findIndex((line) => heading.test(line));
  if (start < 0) return { found: false, notes: "" };
  let end = lines.length;
  for (let i = start + 1; i < lines.length; i++) {
    if (lines[i].startsWith("## ")) {
      end = i;
      break;
    }
  }
  return { found: true, notes: lines.slice(start + 1, end).join("\n").trim() };
}

export function resolveNotes({ found, notes, mode, version }) {
  if (!found && mode === "release") {
    throw new Error(
      `CHANGELOG.md 找不到 ## [${version}] 段落，正式發版中止。`,
    );
  }
  if (!found) {
    return {
      notes: "",
      warning: `演練：CHANGELOG 沒有 ## [${version}]，notes 留空。`,
    };
  }
  return { notes, warning: null };
}

export function releaseFilenames(version) {
  if (!/^[0-9A-Za-z.+-]+$/.test(version)) {
    throw new Error(`版本號不能當檔名：${version}`);
  }
  const stem = `TableTavern_${version}`;
  return {
    exe: `${stem}_x64-setup.exe`,
    appTarGz: `${stem}_aarch64.app.tar.gz`,
    dmg: `${stem}_aarch64.dmg`,
  };
}

export function classifyArtifact(filename) {
  const base = String(filename).split(/[/\\]/).pop();
  if (base.startsWith(".")) return "skip";
  const rules = [
    [".app.tar.gz.sig", "app-sig"],
    [".app.tar.gz", "app"],
    [".dmg", "dmg"],
    [".exe.sig", "exe-sig"],
    [".exe", "exe"],
  ];
  for (const [suffix, kind] of rules) {
    if (base.endsWith(suffix)) return kind;
  }
  return "unknown";
}

/**
 * @param {{ windows: string[], macos: string[] }} files 各目錄內的相對路徑
 * @param {string} version
 */
export function planReleaseFiles(files, version) {
  const names = releaseFilenames(version);
  const buckets = {
    exe: [],
    "exe-sig": [],
    app: [],
    "app-sig": [],
    dmg: [],
  };
  const unknowns = [];

  const consume = (list, platform) => {
    for (const file of list) {
      const kind = classifyArtifact(file);
      if (kind === "skip") continue;
      if (kind === "unknown") {
        unknowns.push(file);
        continue;
      }
      buckets[kind].push({ file, platform });
    }
  };
  consume(files.windows, "windows");
  consume(files.macos, "macos");

  if (unknowns.length) {
    throw new Error(`不認得的產物（不默默丟掉）：${unknowns.join("、")}`);
  }

  const one = (kind, label) => {
    const found = buckets[kind];
    if (found.length !== 1) {
      const listed = found.map((item) => item.file).join("、") || "（無）";
      throw new Error(`${label} 應恰好 1 個，實際 ${found.length}：${listed}`);
    }
    return found[0];
  };

  const exe = one("exe", "Windows NSIS（.exe）");
  const exeSig = one("exe-sig", "Windows .exe.sig");
  const app = one("app", "macOS .app.tar.gz");
  const appSig = one("app-sig", "macOS .app.tar.gz.sig");
  const dmg = one("dmg", "macOS .dmg");

  const copies = [
    { from: exe.file, to: names.exe, platform: exe.platform },
    { from: exeSig.file, to: `${names.exe}.sig`, platform: exeSig.platform },
    { from: app.file, to: names.appTarGz, platform: app.platform },
    { from: appSig.file, to: `${names.appTarGz}.sig`, platform: appSig.platform },
    { from: dmg.file, to: names.dmg, platform: dmg.platform },
  ];
  const requireSig = [
    { file: names.exe, sig: `${names.exe}.sig` },
    { file: names.appTarGz, sig: `${names.appTarGz}.sig` },
  ];

  return { copies, names, requireSig };
}

/** 從 marker.rs 原文抓 `CURRENT_FORMAT`。不呼叫 current_format()。 */
export function readCurrentFormat(source) {
  const match = String(source).match(/pub const CURRENT_FORMAT:\s*u64\s*=\s*(\d+)\s*;/);
  if (!match) throw new Error("marker.rs 找不到 CURRENT_FORMAT");
  const value = Number(match[1]);
  if (!Number.isInteger(value) || value <= 0) {
    throw new Error(`CURRENT_FORMAT 不是正整數：${match[1]}`);
  }
  return value;
}

export function buildLatestJson({
  version,
  notes,
  pubDate,
  tag,
  repoBase,
  exeSignature,
  appSignature,
  formatVersion,
}) {
  if (!tag) throw new Error("latest.json 需要 tag");
  if (!Number.isInteger(formatVersion) || formatVersion <= 0) {
    throw new Error(`format_version 不是正整數：${formatVersion}`);
  }
  const names = releaseFilenames(version);
  const fileUrl = (name) =>
    `${repoBase}/releases/download/${encodeURIComponent(tag)}/${encodeURIComponent(name)}`;
  return {
    version,
    notes,
    pub_date: pubDate,
    format_version: formatVersion,
    platforms: {
      "windows-x86_64": {
        url: fileUrl(names.exe),
        signature: exeSignature,
      },
      "darwin-aarch64": {
        url: fileUrl(names.appTarGz),
        signature: appSignature,
      },
    },
  };
}

/**
 * 同名 release 已公開就失敗；是草稿就先刪再重建。
 * @param {{ version: string, existing: { isDraft: boolean }|null, tag: string }} input
 */
export function githubReleasePlan({ version, existing, tag }) {
  if (existing && existing.isDraft === false) {
    return { ok: false, error: `同名 release 已公開（${tag}），不覆蓋。` };
  }
  const prerelease = hasPrerelease(version);
  return {
    ok: true,
    deleteDraft: Boolean(existing?.isDraft),
    prerelease,
    latest: !prerelease,
  };
}
