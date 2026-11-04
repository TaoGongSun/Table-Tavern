// 房間碼、密鑰、憑證、配對碼與 ULID 的產生，以及雜湊與定長比較（協定 §1.2、§10）。只用 WebCrypto。
const CROCKFORD = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";
export const CROCKFORD8 = /^[0-9A-HJKMNP-TV-Z]{8}$/;

export function crockford(bytes: Uint8Array, length: number): string {
  let out = "";
  for (let i = 0; i < length; i++) out += CROCKFORD[bytes[i % bytes.length] & 31];
  return out;
}

export function base64url(bytes: Uint8Array): string {
  let bin = "";
  for (const b of bytes) bin += String.fromCharCode(b);
  return btoa(bin).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

/** 128 位元亂數 → 22 字（加入密鑰）；256 位元 → 43 字（執行端憑證） */
export const secret = (rng: (n: number) => Uint8Array, bytes: 16 | 32) => base64url(rng(bytes));

/** ULID：48 位元毫秒時間＋80 位元亂數，Crockford base32 26 字 */
export function ulid(now: number, rng: (n: number) => Uint8Array): string {
  let time = "";
  for (let t = now, i = 0; i < 10; i++, t = Math.floor(t / 32)) time = CROCKFORD[t % 32] + time;
  return time + crockford(rng(16), 16);
}

export async function sha256Hex(text: string): Promise<string> {
  const digest = new Uint8Array(
    await crypto.subtle.digest("SHA-256", new TextEncoder().encode(text)),
  );
  return [...digest].map((b) => b.toString(16).padStart(2, "0")).join("");
}

/** 定長比較：兩邊都是同長度的十六進位雜湊，逐字 XOR 不提早結束 */
export function sameHash(a: string, b: string): boolean {
  if (a.length !== b.length) return false;
  let diff = 0;
  for (let i = 0; i < a.length; i++) diff |= a.charCodeAt(i) ^ b.charCodeAt(i);
  return diff === 0;
}
