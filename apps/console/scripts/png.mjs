import { deflateSync } from "node:zlib";

const CRC_TABLE = Array.from({ length: 256 }, (_, n) => {
  let c = n;
  for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  return c >>> 0;
});

const PALETTES = [
  [
    [255, 138, 76],
    [214, 52, 110],
    [255, 214, 165],
  ],
  [
    [38, 166, 154],
    [26, 82, 118],
    [178, 235, 225],
  ],
  [
    [124, 92, 255],
    [42, 31, 112],
    [205, 190, 255],
  ],
  [
    [250, 204, 21],
    [234, 88, 12],
    [255, 241, 196],
  ],
  [
    [56, 189, 248],
    [30, 64, 175],
    [204, 236, 255],
  ],
  [
    [74, 222, 128],
    [21, 94, 76],
    [210, 250, 220],
  ],
  [
    [244, 114, 182],
    [109, 40, 217],
    [252, 220, 240],
  ],
  [
    [148, 163, 184],
    [30, 41, 59],
    [226, 232, 240],
  ],
];

function crc32(bytes) {
  let c = 0xffffffff;
  for (const b of bytes) c = CRC_TABLE[(c ^ b) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

function chunk(type, data) {
  const out = Buffer.alloc(12 + data.length);
  out.writeUInt32BE(data.length, 0);
  out.write(type, 4, "ascii");
  data.copy(out, 8);
  out.writeUInt32BE(crc32(out.subarray(4, 8 + data.length)), 8 + data.length);
  return out;
}

function encode(size, pixel) {
  const rows = [];
  for (let y = 0; y < size; y++) {
    const row = Buffer.alloc(1 + size * 3);
    for (let x = 0; x < size; x++) row.set(pixel(x, y), 1 + x * 3);
    rows.push(row);
  }
  const header = Buffer.alloc(13);
  header.writeUInt32BE(size, 0);
  header.writeUInt32BE(size, 4);
  header[8] = 8;
  header[9] = 2;
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", header),
    chunk("IDAT", deflateSync(Buffer.concat(rows))),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

const mix = (a, b, t) => a.map((v, i) => v + (b[i] - v) * t);

function random(seed) {
  let state = seed * 2654435761 || 1;
  return () => {
    state ^= state << 13;
    state ^= state >>> 17;
    state ^= state << 5;
    return (state >>> 0) / 4294967296;
  };
}

export function abstractPicture(seed, size = 128) {
  const next = random(seed + 1);
  const [light, deep, glow] = PALETTES[seed % PALETTES.length];
  const angle = next() * Math.PI * 2;
  const shapes = Array.from({ length: 3 }, () => ({ x: next(), y: next(), r: 0.18 + next() * 0.3, alpha: 0.35 + next() * 0.4 }));
  const color = (u, v) => {
    const t = Math.min(1, Math.max(0, 0.5 + ((u - 0.5) * Math.cos(angle) + (v - 0.5) * Math.sin(angle)) * 1.1));
    let c = mix(light, deep, t);
    for (const s of shapes) {
      const d = Math.hypot(u - s.x, v - s.y) / s.r;
      if (d < 1) c = mix(c, glow, s.alpha * (1 - d * d));
    }
    return c;
  };
  const samples = 3;
  return encode(size, (x, y) => {
    let sum = [0, 0, 0];
    for (let i = 0; i < samples; i++)
      for (let j = 0; j < samples; j++) {
        const c = color((x + (i + 0.5) / samples) / size, (y + (j + 0.5) / samples) / size);
        sum = sum.map((v, k) => v + c[k]);
      }
    return sum.map((v) => Math.round(v / (samples * samples)));
  });
}

export function pictureUpload(seed) {
  return { content_type: "image/png", data: abstractPicture(seed).toString("base64") };
}
