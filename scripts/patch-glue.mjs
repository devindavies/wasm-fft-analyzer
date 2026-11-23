#!/usr/bin/env node
// Post-build step: make the wasm-bindgen glue loadable where TextDecoder /
// TextEncoder do not exist (AudioWorkletGlobalScope).
//
// wasm-bindgen emits `new TextDecoder(...)` and `.decode()` at module top
// level, so the glue throws on import in a worklet. This prepends a guard that
// installs a minimal UTF-8 polyfill ONLY when the globals are missing; where
// they exist (browsers, Node, workers) the guard does nothing.
//
// Usage: node scripts/patch-glue.mjs [pkgDir]   (default: ./pkg)
//
// The generated glue is never hand-edited in the repo; this runs on build
// output. It fails if it finds nothing to patch, so a change in the generated
// glue cannot silently ship an unguarded package.
import { readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';

export const MARKER = '/* wasm-fft-analyzer: text codec guard */';

// Lossy only on malformed input (each bad byte becomes U+FFFD, or throws when
// `fatal`). Valid UTF-8 round-trips exactly. Only the surface wasm-bindgen's
// glue uses is implemented: decode(view?), encode(str), encodeInto(str, view).
export const POLYFILL = `${MARKER}
(function (g) {
  if (typeof g.TextDecoder === 'undefined') {
    g.TextDecoder = class TextDecoder {
      constructor(label, options) {
        this.fatal = !!(options && options.fatal);
        this.ignoreBOM = !!(options && options.ignoreBOM);
      }
      get encoding() { return 'utf-8'; }
      decode(input) {
        if (input === undefined) return '';
        const b = ArrayBuffer.isView(input)
          ? new Uint8Array(input.buffer, input.byteOffset, input.byteLength)
          : new Uint8Array(input);
        const units = [];
        let out = '';
        let i = 0;
        if (!this.ignoreBOM && b[0] === 0xef && b[1] === 0xbb && b[2] === 0xbf) i = 3;
        const bad = () => {
          if (this.fatal) throw new TypeError('The encoded data was not valid for encoding utf-8');
          units.push(0xfffd);
        };
        while (i < b.length) {
          const lead = b[i];
          let n = 0, cp = lead, min = 0;
          if (lead >= 0xc2 && lead <= 0xdf) { n = 1; cp = lead & 0x1f; min = 0x80; }
          else if (lead >= 0xe0 && lead <= 0xef) { n = 2; cp = lead & 0x0f; min = 0x800; }
          else if (lead >= 0xf0 && lead <= 0xf4) { n = 3; cp = lead & 0x07; min = 0x10000; }
          else if (lead >= 0x80) { bad(); i++; continue; }
          let ok = i + n < b.length + (n === 0 ? 1 : 0);
          for (let k = 1; ok && k <= n; k++) {
            const c = b[i + k];
            if ((c & 0xc0) !== 0x80) ok = false;
            else cp = (cp << 6) | (c & 0x3f);
          }
          if (!ok || cp < min || cp > 0x10ffff || (cp >= 0xd800 && cp <= 0xdfff)) { bad(); i++; continue; }
          i += n + 1;
          if (cp >= 0x10000) { cp -= 0x10000; units.push(0xd800 + (cp >> 10), 0xdc00 + (cp & 0x3ff)); }
          else units.push(cp);
          if (units.length >= 4096) { out += String.fromCharCode.apply(null, units); units.length = 0; }
        }
        return out + String.fromCharCode.apply(null, units);
      }
    };
  }
  if (typeof g.TextEncoder === 'undefined') {
    const utf8 = (str, view) => {
      // Encodes into \`view\` when given (stopping before a char that won't
      // fit), else into a growable array. Returns [read, written, bytes].
      const bytes = view ? null : [];
      let read = 0, written = 0;
      const put = (...v) => {
        if (view) {
          if (written + v.length > view.length) return false;
          for (let k = 0; k < v.length; k++) view[written + k] = v[k];
        } else bytes.push(...v);
        written += v.length;
        return true;
      };
      while (read < str.length) {
        let cp = str.charCodeAt(read);
        let units = 1;
        if (cp >= 0xd800 && cp <= 0xdbff && read + 1 < str.length) {
          const lo = str.charCodeAt(read + 1);
          if (lo >= 0xdc00 && lo <= 0xdfff) { cp = 0x10000 + ((cp - 0xd800) << 10) + (lo - 0xdc00); units = 2; }
        }
        if (cp >= 0xd800 && cp <= 0xdfff) cp = 0xfffd;
        const ok = cp < 0x80 ? put(cp)
          : cp < 0x800 ? put(0xc0 | (cp >> 6), 0x80 | (cp & 0x3f))
          : cp < 0x10000 ? put(0xe0 | (cp >> 12), 0x80 | ((cp >> 6) & 0x3f), 0x80 | (cp & 0x3f))
          : put(0xf0 | (cp >> 18), 0x80 | ((cp >> 12) & 0x3f), 0x80 | ((cp >> 6) & 0x3f), 0x80 | (cp & 0x3f));
        if (!ok) break;
        read += units;
      }
      return [read, written, bytes];
    };
    g.TextEncoder = class TextEncoder {
      get encoding() { return 'utf-8'; }
      encode(str = '') { return new Uint8Array(utf8(String(str), null)[2]); }
      encodeInto(str, view) {
        const [read, written] = utf8(String(str), view);
        return { read, written };
      }
    };
  }
})(globalThis);
`;

/** Returns the patched source, or null if it has no top-level codec use. */
export function patchSource(src) {
  if (src.includes(MARKER)) return src; // idempotent
  if (!/new Text(De|En)coder\b/.test(src)) return null;
  return `${POLYFILL}${src}`;
}

/** Patches every glue .js in `dir`; throws if none needed patching. */
export function patchDir(dir) {
  const patched = [];
  for (const name of readdirSync(dir)) {
    if (!name.endsWith('.js')) continue;
    const file = join(dir, name);
    const src = readFileSync(file, 'utf8');
    const next = patchSource(src);
    if (next === null) continue;
    if (next !== src) writeFileSync(file, next);
    patched.push(name);
  }
  if (patched.length === 0) {
    throw new Error(`patch-glue: no TextDecoder/TextEncoder use found in ${dir}; the generated glue changed, review this script`);
  }
  return patched;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const dir = process.argv[2] ?? 'pkg';
  console.log(`patched: ${patchDir(dir).join(', ')}`);
}
