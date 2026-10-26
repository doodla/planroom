import { expect, test } from "vitest";

import {
  base64url,
  canonical,
  fromBase64url,
  isPersonKey,
  lineFields,
  newPersonKey,
  signToken,
  tokenMessage,
  verifyToken,
} from "./actor";

const enc = (s: string): Uint8Array => new TextEncoder().encode(s);

test("base64url matches RFC 4648's vectors, without padding, both ways", () => {
  const vectors: [string, string][] = [
    ["", ""],
    ["f", "Zg"],
    ["fo", "Zm8"],
    ["foo", "Zm9v"],
    ["foob", "Zm9vYg"],
    ["fooba", "Zm9vYmE"],
    ["foobar", "Zm9vYmFy"],
  ];
  for (const [plain, coded] of vectors) {
    expect(base64url(enc(plain))).toBe(coded);
    expect(fromBase64url(coded)).toEqual(enc(plain));
  }
  expect(base64url(Uint8Array.from([0xfb, 0xff]))).toBe("-_8");
  expect(() => fromBase64url("a+b")).toThrow();
});

test("a new key is a P-256 pair whose public half is the 65-byte SEC1 point", () => {
  const key = newPersonKey();
  expect(fromBase64url(key.secret)).toHaveLength(32);
  const pub = fromBase64url(key.public);
  expect(pub).toHaveLength(65);
  expect(pub[0]).toBe(4);
  expect(isPersonKey(key)).toBe(true);
  expect(isPersonKey({ ...key, public: newPersonKey().public })).toBe(false);
  expect(isPersonKey({ ...key, format: "other" })).toBe(false);
  expect(isPersonKey(null)).toBe(false);
});

test("canonical JSON sorts keys at every depth, as the engine's does", () => {
  expect(canonical({ b: 1, a: { d: 2, c: [{ f: 1, e: 0 }] } })).toBe(
    '{"a":{"c":[{"e":0,"f":1}],"d":2},"b":1}',
  );
  expect(canonical(["--level", "container"])).toBe('["--level","container"]');
});

test("the signed message is each field and a NUL", () => {
  const message = new TextDecoder().decode(
    tokenMessage({
      command: "map",
      args: ["--level", "container"],
      idem: "",
      base: "412",
      at: 1_800_000_000_000,
    }),
  );
  const NUL = String.fromCharCode(0);
  expect(message).toBe(
    ["map", '["--level","container"]', "", "412", "1800000000000", ""].join(
      NUL,
    ),
  );
});

test("a_token_works_only_for_its_action: it verifies under its key, for its fields alone", () => {
  const key = newPersonKey();
  const fields = {
    command: "decide",
    args: ["card_1", "accept"],
    idem: "i-1",
    base: "412",
  };
  const token = signToken(key, { ...fields, at: 1_800_000_000_000 });
  expect(token).toMatch(/^1800000000000\.[A-Za-z0-9_-]{86}$/);
  expect(verifyToken(key.public, token, fields)).toBe(true);
  expect(verifyToken(key.public, token, { ...fields, command: "map" })).toBe(
    false,
  );
  expect(
    verifyToken(key.public, token, { ...fields, args: ["card_1", "reject"] }),
  ).toBe(false);
  expect(verifyToken(key.public, token, { ...fields, base: "413" })).toBe(
    false,
  );
  expect(verifyToken(newPersonKey().public, token, fields)).toBe(false);
  // The time is signed too.
  expect(verifyToken(key.public, token.replace(/^1/, "2"), fields)).toBe(false);
});

test("a CLI line splits into the token's fields", () => {
  expect(
    lineFields(
      ["handoff", "merge", "K1", "--base", "9", "--idem", "x", "--json"],
      2,
    ),
  ).toEqual({ command: "handoff merge", args: ["K1"], idem: "x", base: "9" });
  expect(lineFields(["map", "--level", "system", "--json"], 1)).toEqual({
    command: "map",
    args: ["--level", "system"],
    idem: "",
    base: "",
  });
});
