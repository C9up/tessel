/**
 * Hermes (React Native 0.87) has TextEncoder but no TextDecoder, which @msgpack/msgpack
 * builds when it loads. This installs a UTF-8-only one, as the WHATWG decoder does with
 * its defaults: malformed sequences become U+FFFD, a leading BOM is dropped. Imported
 * before msgpack; does nothing where the platform has its own.
 */

const REPLACEMENT = 0xfffd

/** Decodes UTF-8 bytes as the WHATWG decoder does with `fatal: false`. */
export function decodeUtf8(bytes: Uint8Array): string {
  const units: number[] = []
  let out = ''
  let i = bytes[0] === 0xef && bytes[1] === 0xbb && bytes[2] === 0xbf ? 3 : 0
  while (i < bytes.length) {
    const first = bytes[i] ?? 0
    let needed = 0
    let point = 0
    let lower = 0x80
    let upper = 0xbf
    if (first < 0x80) {
      point = first
    } else if (first >= 0xc2 && first <= 0xdf) {
      needed = 1
      point = first & 0x1f
    } else if (first >= 0xe0 && first <= 0xef) {
      needed = 2
      point = first & 0x0f
      if (first === 0xe0) lower = 0xa0
      if (first === 0xed) upper = 0x9f
    } else if (first >= 0xf0 && first <= 0xf4) {
      needed = 3
      point = first & 0x07
      if (first === 0xf0) lower = 0x90
      if (first === 0xf4) upper = 0x8f
    } else {
      units.push(REPLACEMENT)
      i += 1
      continue
    }
    i += 1
    let complete = true
    for (let k = 0; k < needed; k++) {
      const next = bytes[i]
      if (next === undefined || next < lower || next > upper) {
        complete = false
        break
      }
      point = (point << 6) | (next & 0x3f)
      lower = 0x80
      upper = 0xbf
      i += 1
    }
    if (!complete) {
      units.push(REPLACEMENT)
    } else if (point > 0xffff) {
      point -= 0x10000
      units.push(0xd800 | (point >> 10), 0xdc00 | (point & 0x3ff))
    } else {
      units.push(point)
    }
    if (units.length > 8192) out += String.fromCharCode(...units.splice(0))
  }
  return out + String.fromCharCode(...units)
}

function bytesOf(input: unknown): Uint8Array {
  if (input === undefined) return new Uint8Array(0)
  if (input instanceof Uint8Array) return input
  if (input instanceof ArrayBuffer) return new Uint8Array(input)
  if (ArrayBuffer.isView(input)) return new Uint8Array(input.buffer, input.byteOffset, input.byteLength)
  throw new TypeError('TextDecoder.decode takes an ArrayBuffer or a view of one')
}

class Utf8Decoder {
  readonly encoding = 'utf-8'
  readonly fatal = false
  readonly ignoreBOM = false

  constructor(label = 'utf-8') {
    if (!/^\s*(utf-?8|unicode-1-1-utf-8)\s*$/i.test(label)) throw new RangeError(`unsupported encoding ${label}`)
  }

  decode(input?: unknown): string {
    return decodeUtf8(bytesOf(input))
  }
}

if (typeof globalThis.TextDecoder === 'undefined') {
  Reflect.set(globalThis, 'TextDecoder', Utf8Decoder)
}
