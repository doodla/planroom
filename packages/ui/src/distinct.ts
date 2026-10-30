/**
 * How far apart two colors look: the CIEDE2000 difference (Sharma, Wu and Dalal,
 * 2005), under normal vision and under simulated protan, deutan and tritan vision
 * (Machado, Oliveira and Fernandes, 2009, at full severity, applied in linear RGB).
 * Every theme's tone fills, states and lens yarns must stay this far apart
 * (DR-122). Pure math: no DOM, no storage, no Node.
 */
import { over, type Color } from "./tokens";

/** The three kinds of colour-deficient vision the bars are checked under. */
export type Deficiency = "protan" | "deutan" | "tritan";

/** Normal vision, or one of the three deficiencies. */
export type Vision = "normal" | Deficiency;

/** A color in CIELAB, against the D65 white. */
export interface Lab {
  readonly L: number;
  readonly a: number;
  readonly b: number;
}

type Row = readonly [number, number, number];
type Rgb = readonly [number, number, number];

/** Machado et al. (2009), severity 1.0, applied to linear RGB. */
const MACHADO: Record<Deficiency, readonly [Row, Row, Row]> = {
  protan: [
    [0.152286, 1.052583, -0.204868],
    [0.114503, 0.786281, 0.099216],
    [-0.003882, -0.048116, 1.051998],
  ],
  deutan: [
    [0.367322, 0.860646, -0.227968],
    [0.280085, 0.672501, 0.047413],
    [-0.01182, 0.04294, 0.968881],
  ],
  tritan: [
    [1.255528, -0.076749, -0.178779],
    [-0.078411, 0.930809, 0.147602],
    [0.004733, 0.691367, 0.3039],
  ],
};

/** `#rrggbb` as three numbers in [0, 1]. */
function channels01(hex: string): Rgb {
  const n = Number.parseInt(hex.slice(1), 16);
  return [((n >> 16) & 0xff) / 255, ((n >> 8) & 0xff) / 255, (n & 0xff) / 255];
}

function toLinear(c: number): number {
  return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
}

function toGamma(c: number): number {
  return c <= 0.0031308 ? 12.92 * c : 1.055 * c ** (1 / 2.4) - 0.055;
}

const clamp01 = (c: number): number => Math.min(1, Math.max(0, c));

function opaqueLinear(c: Color): Rgb {
  if (c.alpha !== 1) {
    throw new Error(
      "labOf takes an opaque color; composite it with over first",
    );
  }
  const [r, g, b] = channels01(c.hex);
  return [toLinear(r), toLinear(g), toLinear(b)];
}

/** Linear RGB as `vision` sees it, each channel clamped to [0, 1], with no rounding. */
function linearUnder(rgb: Rgb, vision: Vision): Rgb {
  if (vision === "normal") {
    return rgb;
  }
  const [r0, r1, r2] = MACHADO[vision];
  const dot = (row: Row): number =>
    clamp01(row[0] * rgb[0] + row[1] * rgb[1] + row[2] * rgb[2]);
  return [dot(r0), dot(r1), dot(r2)];
}

const WHITE = [0.95047, 1, 1.08883] as const;

function labF(t: number): number {
  return t > 216 / 24389 ? Math.cbrt(t) : ((24389 / 27) * t + 16) / 116;
}

/** An opaque color in CIELAB, as `vision` sees it. */
export function labOf(c: Color, vision: Vision = "normal"): Lab {
  const [r, g, b] = linearUnder(opaqueLinear(c), vision);
  const x = 0.4124564 * r + 0.3575761 * g + 0.1804375 * b;
  const y = 0.2126729 * r + 0.7151522 * g + 0.072175 * b;
  const z = 0.0193339 * r + 0.119192 * g + 0.9503041 * b;
  const fx = labF(x / WHITE[0]);
  const fy = labF(y / WHITE[1]);
  const fz = labF(z / WHITE[2]);
  return { L: 116 * fy - 16, a: 500 * (fx - fy), b: 200 * (fy - fz) };
}

// Written as literals, which a bundler drops from a bundle that never measures colour; an
// expression such as `Math.PI / 180` stays in every bundle that imports the package.
/** π / 180, exactly as `Math.PI / 180` computes it. */
const RAD = 0.017453292519943295;
/** 25 to the seventh. */
const POW25_7 = 6_103_515_625;

function hueDegrees(b: number, a: number): number {
  if (a === 0 && b === 0) {
    return 0;
  }
  const h = Math.atan2(b, a) / RAD;
  return h < 0 ? h + 360 : h;
}

/** CIEDE2000 between two CIELAB colors, with kL = kC = kH = 1 (Sharma et al., 2005, eqs. 1-17). */
export function deltaE2000Lab(x: Lab, y: Lab): number {
  const c1 = Math.hypot(x.a, x.b);
  const c2 = Math.hypot(y.a, y.b);
  const cBar7 = ((c1 + c2) / 2) ** 7;
  const g = 0.5 * (1 - Math.sqrt(cBar7 / (cBar7 + POW25_7)));
  const a1 = (1 + g) * x.a;
  const a2 = (1 + g) * y.a;
  const cp1 = Math.hypot(a1, x.b);
  const cp2 = Math.hypot(a2, y.b);
  const hp1 = hueDegrees(x.b, a1);
  const hp2 = hueDegrees(y.b, a2);

  const dLp = y.L - x.L;
  const dCp = cp2 - cp1;
  const chromaProduct = cp1 * cp2;
  let dhp = 0;
  if (chromaProduct !== 0) {
    dhp = hp2 - hp1;
    if (dhp > 180) {
      dhp -= 360;
    } else if (dhp < -180) {
      dhp += 360;
    }
  }
  const dHp = 2 * Math.sqrt(chromaProduct) * Math.sin((dhp * RAD) / 2);

  const lBarp = (x.L + y.L) / 2;
  const cBarp = (cp1 + cp2) / 2;
  let hBarp = hp1 + hp2;
  if (chromaProduct !== 0) {
    if (Math.abs(hp1 - hp2) > 180) {
      hBarp += hBarp < 360 ? 360 : -360;
    }
    hBarp /= 2;
  }

  const t =
    1 -
    0.17 * Math.cos((hBarp - 30) * RAD) +
    0.24 * Math.cos(2 * hBarp * RAD) +
    0.32 * Math.cos((3 * hBarp + 6) * RAD) -
    0.2 * Math.cos((4 * hBarp - 63) * RAD);
  const dTheta = 30 * Math.exp(-(((hBarp - 275) / 25) ** 2));
  const cBarp7 = cBarp ** 7;
  const rC = 2 * Math.sqrt(cBarp7 / (cBarp7 + POW25_7));
  const l50 = (lBarp - 50) ** 2;
  const sL = 1 + (0.015 * l50) / Math.sqrt(20 + l50);
  const sC = 1 + 0.045 * cBarp;
  const sH = 1 + 0.015 * cBarp * t;
  const rT = -Math.sin(2 * dTheta * RAD) * rC;

  const l = dLp / sL;
  const c = dCp / sC;
  const h = dHp / sH;
  return Math.sqrt(l * l + c * c + h * h + rT * c * h);
}

/**
 * The CIEDE2000 difference between two colors as `vision` sees them. A
 * translucent color is composited over `surface` first; with none given it throws.
 * The simulation is never rounded to 8 bits.
 */
export function deltaE2000(
  a: Color,
  b: Color,
  vision: Vision = "normal",
  surface?: Color,
): number {
  const flat = (c: Color): Color => {
    if (c.alpha === 1) {
      return c;
    }
    if (surface === undefined) {
      throw new Error("a translucent color needs the surface it sits on");
    }
    return over(c, surface);
  };
  return deltaE2000Lab(labOf(flat(a), vision), labOf(flat(b), vision));
}

/**
 * `c` as a person with `kind` would see it, rounded to `#rrggbb`, for showing a
 * simulated color. The bars use `deltaE2000(…, vision)`, which never rounds.
 */
export function simulateVision(c: Color, kind: Deficiency): Color {
  const rgb = linearUnder(opaqueLinear(c), kind);
  const hex = rgb
    .map((v) =>
      Math.round(clamp01(toGamma(v)) * 255)
        .toString(16)
        .padStart(2, "0"),
    )
    .join("");
  return { hex: `#${hex}`, alpha: 1 };
}
