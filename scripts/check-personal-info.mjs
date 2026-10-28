// repo 是公開的：本機帳號名與 git email 不得出現在會進版控的檔案（含還沒 add 的新檔）。
// 本機路徑請寫成 ~、<user> 或 <scratchpad>。帳號名太短或是 CI 的通用帳號就不比對，免得誤擋。
// 其他不能寫進 repo 的字（真名、私人 email）一行一個放 ~/.config/table-tavern/private-words，各 worktree 共用。
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = fileURLToPath(new URL("..", import.meta.url));
const GENERIC = new Set(["runner", "runneradmin", "root", "admin", "user", "ubuntu", "vscode", "node"]);

const git = (args) => execFileSync("git", args, { cwd: ROOT, encoding: "utf8" });
const escape = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

const words = [];
const name = os.userInfo().username;
if (name.length >= 4 && !GENERIC.has(name.toLowerCase())) words.push(name);
let email = "";
try {
  email = git(["config", "user.email"]).trim();
} catch {}
if (email && !email.endsWith("@users.noreply.github.com")) words.push(email);
try {
  const extra = fs.readFileSync(path.join(os.homedir(), ".config/table-tavern/private-words"), "utf8");
  words.push(...extra.split("\n").map((w) => w.trim()).filter((w) => w.length >= 4));
} catch {}
if (words.length === 0) {
  console.log("個資檢查略過（本機帳號名為通用名稱，也沒設 git email）");
  process.exit(0);
}
const pattern = new RegExp(
  words.map((w) => `(?<![A-Za-z0-9])${escape(w)}(?![A-Za-z0-9])`).join("|"),
  "i",
);

const files = git(["ls-files", "-z", "--cached", "--others", "--exclude-standard"])
  .split("\0")
  .filter(Boolean);
const hits = [];
for (const file of files) {
  const full = path.join(ROOT, file);
  let buf;
  try {
    buf = fs.readFileSync(full);
  } catch {
    continue; // 已刪但還沒 git rm
  }
  if (buf.includes(0)) continue; // 二進位檔
  const lines = buf.toString("utf8").split("\n");
  lines.forEach((line, i) => {
    if (pattern.test(line)) hits.push(`${file}:${i + 1}`);
  });
}
if (hits.length > 0) {
  console.error(`以下位置含本機帳號名、git email 或 private-words 裡的字（改寫成 ~、<user> 或 <scratchpad>）：\n${hits.join("\n")}`);
  process.exit(1);
}
console.log(`個資檢查通過（${files.length} 個檔案）`);
