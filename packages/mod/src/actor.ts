/**
 * The person key and one-time action tokens (architecture.md, "Actors"; DR-64, DR-72). The key is
 * an ECDSA P-256 pair kept in the mod's `$.store`; the engine holds only the public half, which
 * `planroom person init` registers. A press, a submit or a typed `/room` signs
 * `command ‖ canonical(args) ‖ idem ‖ base ‖ at` and passes `<at>.<sig>` as
 * `PLANROOM_PERSON_TOKEN`, where `sig` is the base64url of the raw `r ‖ s` signature over SHA-256.
 *
 * A-17: the mod host's `crypto.subtle` has only `digest` (checked under `claude plugin test` on
 * Claude Code 2.1.310), so signing uses the audited pure-TypeScript `@noble/curves`, bundled.
 */

import { p256 } from "@noble/curves/nist.js";

/** The tag of the key as `$.store` keeps it. */
export const KEY_FORMAT = "planroom.person-key/v1";

/** The `$.store` key the person key lives under. */
export const KEY_STORE = "person-key";

/** The person key as the mod keeps it: never in `$.state`, never in an argv or a log. */
export interface PersonKey {
  format: typeof KEY_FORMAT;
  /** The private scalar, 32 bytes, base64url. */
  secret: string;
  /** The SEC1 uncompressed public point, 65 bytes, base64url: what `person init` reads. */
  public: string;
  /** The workspace roots whose store has registered this key. */
  registered: string[];
}

const B64URL =
  "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/** base64url without padding (RFC 4648 §5). */
export function base64url(bytes: Uint8Array): string {
  let out = "";
  for (let i = 0; i < bytes.length; i += 3) {
    const a = bytes[i] ?? 0;
    const b = bytes[i + 1] ?? 0;
    const c = bytes[i + 2] ?? 0;
    const n = (a << 16) | (b << 8) | c;
    const chars = [18, 12, 6, 0].map((s) => B64URL.charAt((n >> s) & 63));
    out += chars.slice(0, Math.min(4, bytes.length - i + 1)).join("");
  }
  return out;
}

/** The bytes of a base64url string; throws on a character outside the alphabet. */
export function fromBase64url(text: string): Uint8Array {
  const out: number[] = [];
  let bits = 0;
  let value = 0;
  for (const ch of text) {
    const v = B64URL.indexOf(ch);
    if (v < 0) throw new Error(`not base64url: ${JSON.stringify(ch)}`);
    value = (value << 6) | v;
    bits += 6;
    if (bits >= 8) {
      bits -= 8;
      out.push((value >> bits) & 255);
    }
  }
  return Uint8Array.from(out);
}

/** A new key pair, from the host's `crypto.getRandomValues`. */
export function newPersonKey(): PersonKey {
  const secret = p256.utils.randomSecretKey();
  return {
    format: KEY_FORMAT,
    secret: base64url(secret),
    public: base64url(p256.getPublicKey(secret, false)),
    registered: [],
  };
}

/** Whether a stored value is a usable person key: its public half matches its secret. */
export function isPersonKey(value: unknown): value is PersonKey {
  if (typeof value !== "object" || value === null) return false;
  const v = value as Partial<PersonKey>;
  if (
    v.format !== KEY_FORMAT ||
    typeof v.secret !== "string" ||
    typeof v.public !== "string" ||
    !Array.isArray(v.registered) ||
    !v.registered.every((r) => typeof r === "string")
  ) {
    return false;
  }
  try {
    return (
      base64url(p256.getPublicKey(fromBase64url(v.secret), false)) === v.public
    );
  } catch {
    return false;
  }
}

/** JSON with every object's keys sorted, as the engine's `kernel::keys::canonical` writes it. */
export function canonical(value: unknown): string {
  const sort = (v: unknown): unknown => {
    if (Array.isArray(v)) return v.map(sort);
    if (typeof v === "object" && v !== null) {
      const o = v as Record<string, unknown>;
      return Object.fromEntries(
        Object.keys(o)
          .sort()
          .map((k) => [k, sort(o[k])]),
      );
    }
    return v;
  };
  return JSON.stringify(sort(value));
}

/** What one token signs. `idem` and `base` are the flags' values, or "" when none is passed. */
export interface TokenFields {
  /** The catalogue command name, its path joined with spaces: `map`, `person init`. */
  command: string;
  /** The CLI arguments after the command's path, less `--json`, `--base` and `--idem`. */
  args: readonly string[];
  idem: string;
  base: string;
  /** Milliseconds since the epoch; the engine refuses a token older than 30 s. */
  at: number;
}

/** The signed bytes: each field's text followed by a NUL, as the engine's `sha256_hex` joins parts. */
export function tokenMessage(f: TokenFields): Uint8Array {
  const parts = [f.command, canonical(f.args), f.idem, f.base, String(f.at)];
  return new TextEncoder().encode(parts.map((p) => `${p}\0`).join(""));
}

/** A one-time action token, `<at>.<sig>`. */
export function signToken(key: PersonKey, f: TokenFields): string {
  const sig = p256.sign(tokenMessage(f), fromBase64url(key.secret), {
    prehash: true,
    lowS: true,
    format: "compact",
  });
  return `${f.at}.${base64url(sig)}`;
}

/** Whether `token` signs `f` under the public key: the engine's check, for tests. */
export function verifyToken(
  publicKey: string,
  token: string,
  f: Omit<TokenFields, "at">,
): boolean {
  const dot = token.indexOf(".");
  if (dot < 0) return false;
  const at = Number(token.slice(0, dot));
  try {
    return p256.verify(
      fromBase64url(token.slice(dot + 1)),
      tokenMessage({ ...f, at }),
      fromBase64url(publicKey),
      { prehash: true },
    );
  } catch {
    return false;
  }
}

/** The fields of a CLI line after the binary, given how many words its command's path takes. */
export function lineFields(
  argv: readonly string[],
  pathWords: number,
): Omit<TokenFields, "at"> {
  const command = argv.slice(0, pathWords).join(" ");
  const args: string[] = [];
  let idem = "";
  let base = "";
  for (let i = pathWords; i < argv.length; i++) {
    const word = argv[i] ?? "";
    if (word === "--json") continue;
    if (word === "--idem" || word === "--base") {
      const value = argv[++i] ?? "";
      if (word === "--idem") idem = value;
      else base = value;
      continue;
    }
    args.push(word);
  }
  return { command, args, idem, base };
}
