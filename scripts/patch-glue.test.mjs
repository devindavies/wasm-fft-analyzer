// Run: node --test scripts/patch-glue.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import vm from 'node:vm';
import { patchSource, patchDir, POLYFILL, MARKER } from './patch-glue.mjs';

// A context shaped like AudioWorkletGlobalScope: no text codecs.
const bareContext = () => vm.createContext({ Uint8Array, ArrayBuffer, String, TypeError, globalThis: undefined });
const run = (code, ctx = bareContext()) => (ctx.globalThis = ctx, vm.runInContext(code, ctx), ctx);

const GLUE = `let d = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
d.decode();
const e = new TextEncoder();
if (!('encodeInto' in e)) throw new Error('no encodeInto');
`;

test('unpatched glue throws without codecs (the bug)', () => {
  assert.throws(() => run(GLUE), /TextDecoder is not defined/);
});

test('patched glue loads without codecs', () => {
  run(patchSource(GLUE));
});

test('guard leaves native codecs alone', () => {
  const ctx = vm.createContext({ TextDecoder, TextEncoder, Uint8Array, ArrayBuffer, String, TypeError });
  ctx.globalThis = ctx;
  run(POLYFILL, ctx);
  assert.equal(ctx.TextDecoder, TextDecoder);
  assert.equal(ctx.TextEncoder, TextEncoder);
});

test('patching is idempotent', () => {
  const once = patchSource(GLUE);
  assert.equal(patchSource(once), once);
  assert.ok(once.startsWith(MARKER));
});

test('files without codec use are skipped; empty result throws', () => {
  assert.equal(patchSource('export const x = 1;'), null);
  const dir = mkdtempSync(join(tmpdir(), 'patch-glue-'));
  writeFileSync(join(dir, 'a.js'), 'export const x = 1;');
  assert.throws(() => patchDir(dir), /no TextDecoder\/TextEncoder/);
  writeFileSync(join(dir, 'b.js'), GLUE);
  assert.deepEqual(patchDir(dir), ['b.js']);
  assert.ok(readFileSync(join(dir, 'b.js'), 'utf8').includes(MARKER));
});

const SAMPLES = ['', 'ascii', 'é ñ ü', '€ 漢字', '😀 emoji 🎵', 'a\u{10FFFF}b', 'lone \ud800 surrogate'];

test('polyfill matches native encode/decode/encodeInto', () => {
  const ctx = run(POLYFILL);
  const pd = new ctx.TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
  const pe = new ctx.TextEncoder();
  for (const s of SAMPLES) {
    const native = new TextEncoder().encode(s);
    assert.deepEqual([...pe.encode(s)], [...native], `encode ${JSON.stringify(s)}`);
    assert.equal(pd.decode(native), new TextDecoder().decode(native));
    for (const size of [0, 1, 2, 3, 5, native.length + 4]) {
      const a = new Uint8Array(size), b = new Uint8Array(size);
      assert.deepEqual({ ...pe.encodeInto(s, a) }, { ...new TextEncoder().encodeInto(s, b) }, `encodeInto ${JSON.stringify(s)} size ${size}`);
      assert.deepEqual([...a], [...b]);
    }
  }
});

test('polyfill decode handles subarray views, BOM, long input and bad bytes', () => {
  const ctx = run(POLYFILL);
  const buf = new TextEncoder().encode('xx€yy');
  assert.equal(new ctx.TextDecoder().decode(buf.subarray(2, 5)), '€');
  const bom = new Uint8Array([0xef, 0xbb, 0xbf, 0x41]);
  assert.equal(new ctx.TextDecoder().decode(bom), new TextDecoder().decode(bom));
  assert.equal(new ctx.TextDecoder('utf-8', { ignoreBOM: true }).decode(bom), new TextDecoder('utf-8', { ignoreBOM: true }).decode(bom));
  const long = 'é'.repeat(20000);
  assert.equal(new ctx.TextDecoder().decode(new TextEncoder().encode(long)), long);
  for (const bytes of [[0xff], [0xc3], [0xe2, 0x82], [0xc0, 0x80], [0xed, 0xa0, 0x80], [0x41, 0x80, 0x42]]) {
    const u = new Uint8Array(bytes);
    assert.throws(() => new ctx.TextDecoder('utf-8', { fatal: true }).decode(u), TypeError);
    assert.equal(new ctx.TextDecoder().decode(u).includes('�'), true);
  }
});
