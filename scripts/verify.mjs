// 本地與 CI 共用的驗證入口：`npm run verify`。任一步非 0 立即停止。
// 順序有相依：Tauri 的 generate_context! 編譯期要讀前端產物，所以 cargo check／test 必須排在
// npm run build 之後。cargo fmt 只解析語法、不編譯，放最前面能用幾秒攔掉純格式錯誤。
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const ROOT = fileURLToPath(new URL("..", import.meta.url));
const TAURI = fileURLToPath(new URL("../src-tauri/", import.meta.url));
// Windows 下 npm 是 .cmd，Node 18.20 起 spawn 不准直接執行批次檔（EINVAL），要走 shell。
// cargo 是 .exe，不需要。args 都是寫死的字面值，沒有引號注入問題。
const WIN = process.platform === "win32";

const steps = [
  { name: "cargo fmt", cmd: "cargo", args: ["fmt", "--check"], cwd: TAURI },
  { name: "vitest", cmd: "npm", args: ["test"], cwd: ROOT, shell: WIN },
  { name: "i18n", cmd: "npm", args: ["run", "check:i18n"], cwd: ROOT, shell: WIN },
  { name: "build", cmd: "npm", args: ["run", "build"], cwd: ROOT, shell: WIN },
  { name: "cargo check", cmd: "cargo", args: ["check"], cwd: TAURI },
  { name: "cargo test", cmd: "cargo", args: ["test"], cwd: TAURI },
];

for (const [i, step] of steps.entries()) {
  console.log(`\n[${i + 1}/${steps.length}] ${step.name}: ${step.cmd} ${step.args.join(" ")}`);
  const run = spawnSync(step.cmd, step.args, {
    cwd: step.cwd,
    stdio: "inherit",
    shell: step.shell ?? false,
  });
  if (run.error) {
    console.error(`\n✗ ${step.name} 起不來：${run.error.message}`);
    process.exit(1);
  }
  if (run.status !== 0) {
    console.error(`\n✗ ${step.name} 失敗（exit ${run.status ?? run.signal}）`);
    process.exit(run.status ?? 1);
  }
}
console.log(`\n✓ verify 全部通過（${steps.length} 步）`);
