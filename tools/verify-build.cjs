// 对照最终 ELF / NRO 与用户当前 subsdk9 的真实导出，检查装载所需入口及 ABI。
const fs = require('fs');
const path = require('path');
const vm = require('vm');
const crypto = require('crypto');
const assert = require('assert/strict');
const root = path.resolve(__dirname, '..');
const repo = path.resolve(root, '..');
const elfPath = path.join(root, 'target/armv7a-none-eabi/release/libmhgu_attack_damage_display.so');
const nroPath = path.join(root, 'attack_damage_display.nro');
const runtimePath = process.argv[2] || 'C:/Users/Lawrence/AppData/Roaming/Ryujinx/mods/contents/0100770008dd8000/exefs/subsdk9';
const elf = fs.readFileSync(elfPath), nro = fs.readFileSync(nroPath);
const u32 = (b, o) => b.readUInt32LE(o);
const hash = b => crypto.createHash('sha256').update(b).digest('hex');
const string = (b, o) => b.subarray(o, b.indexOf(0, o)).toString('utf8');

assert.equal(elf.subarray(0, 4).toString('hex'), '7f454c46');
assert.equal(elf[4], 1); // ELF32
assert.equal(elf[5], 1); // little endian
assert.equal(elf.readUInt16LE(18), 40); // EM_ARM
assert.equal(u32(elf, 36) & 0x600, 0x200); // soft-float C ABI
assert.equal(nro.subarray(0x10, 0x14).toString(), 'NRO0');
assert.equal(u32(nro, 0x18), nro.length);

const phOff = u32(elf, 28), phSize = elf.readUInt16LE(42), phCount = elf.readUInt16LE(44);
const loads = [];
for (let i = 0; i < phCount; i++) {
  const o = phOff + i * phSize;
  if (u32(elf, o) === 1) loads.push({ file: u32(elf, o + 4), addr: u32(elf, o + 8), size: u32(elf, o + 16), memory: u32(elf, o + 20) });
}
assert.equal(loads.length, 3);
const page = n => Math.ceil(n / 0x1000) * 0x1000;
for (let i = 0; i < 3; i++) {
  const load = loads[i], segment = 0x20 + i * 8;
  assert.equal(u32(nro, segment), load.addr);
  assert.equal(u32(nro, segment + 4), page(load.size));
  assert.equal(load.addr % 0x1000, 0);
  const skip = i === 0 ? 0x80 : 0; // NRO 头覆盖的预留区域。
  assert.deepEqual(nro.subarray(load.addr + skip, load.addr + load.size), elf.subarray(load.file + skip, load.file + load.size));
}
assert.equal(u32(nro, 0x38), page(Math.max(0, loads[2].memory - page(loads[2].size))));
const mod = u32(nro, 4);
assert.equal(nro.subarray(mod, mod + 4).toString(), 'MOD0');
const dynamic = mod + nro.readInt32LE(mod + 4);
assert.ok(dynamic >= loads[2].addr && dynamic < nro.length);
const bssStart = mod + nro.readInt32LE(mod + 8), bssEnd = mod + nro.readInt32LE(mod + 12);
assert.ok(bssEnd >= bssStart && bssEnd <= nro.length + u32(nro, 0x38));

const sectionOff = u32(elf, 32), sectionSize = elf.readUInt16LE(46), sectionCount = elf.readUInt16LE(48);
const sections = Array.from({ length: sectionCount }, (_, i) => {
  const o = sectionOff + i * sectionSize;
  return { type: u32(elf, o + 4), file: u32(elf, o + 16), size: u32(elf, o + 20), link: u32(elf, o + 24), stride: u32(elf, o + 36) };
});
const dynsym = sections.find(s => s.type === 11), dynstr = sections[dynsym.link];
const names = elf.subarray(dynstr.file, dynstr.file + dynstr.size), symbols = [];
for (let i = 0; i < dynsym.size; i += dynsym.stride) {
  const o = dynsym.file + i;
  symbols.push({ name: string(names, u32(elf, o)), value: u32(elf, o + 4), bind: elf[o + 12] >> 4, defined: elf.readUInt16LE(o + 14) !== 0 });
}
const exportedNames = symbols.filter(s => s.defined && s.bind !== 0).map(s => s.name);
const required = fs.readFileSync(path.join(root, 'exports.txt'), 'utf8').split(/\r?\n/).filter(s => s && !s.startsWith('#'));
for (const name of required) assert.ok(exportedNames.includes(name), 'missing callback: ' + name);

// 使用现有的只读 NSO 解压器。此处只运行其纯函数部分。
const context = { require, Buffer };
vm.createContext(context);
vm.runInContext(fs.readFileSync(path.join(repo, '.codex-tmp/mh-sfx-analysis/nso_lz4.js'), 'utf8').split('const file =')[0], context);
const runtime = fs.readFileSync(runtimePath), header = context.nsoHeader(runtime);
const memory = Buffer.alloc(header.dataMemOff + header.dataSize + header.bssSize);
for (const [i, name] of ['text', 'ro', 'data'].entries()) {
  const off = header[name + 'FileOff'], size = header[name + 'FileSize'];
  const segment = header.flags & (1 << i) ? context.decompressSegment(runtime, off, size, header[name + 'Size']) : runtime.subarray(off, off + size);
  segment.copy(memory, header[name + 'MemOff']);
}
const runtimeMod = u32(memory, 4);
assert.equal(memory.subarray(runtimeMod, runtimeMod + 4).toString(), 'MOD0');
const tags = {};
for (let o = runtimeMod + memory.readInt32LE(runtimeMod + 4); u32(memory, o) !== 0; o += 8) tags[u32(memory, o)] = u32(memory, o + 4);
const runtimeNames = memory.subarray(tags[5], tags[5] + tags[10]);
const runtimeSymbols = [];
for (let i = 0; i < u32(memory, tags[4] + 4); i++) {
  const o = tags[6] + 16 * i;
  if (memory.readUInt16LE(o + 14) !== 0) runtimeSymbols.push({ name: string(runtimeNames, u32(memory, o)), value: u32(memory, o + 4) });
}
const imports = symbols.filter(s => !s.defined && s.bind === 1).map(s => s.name);
for (const name of imports) assert.ok(runtimeSymbols.some(s => s.name === name), 'unresolved strong import: ' + name);
const create = runtimeSymbols.find(s => s.name === 'forge_hook_create');
assert.equal(u32(memory, create.value + 0x24), 0xE3A0204C, 'current Hook return buffer must be 76 bytes');

const result = {
  nro: nroPath, bytes: nro.length, sha256: hash(nro),
  runtime: runtimePath, runtime_sha256: hash(runtime),
  callbacks: required, strong_imports: imports,
  checks: ['ARM32 soft-float', 'NRO segments match ELF', 'MOD0/dynamic/BSS', 'all callbacks exported', 'all strong imports in installed Forge', '76-byte Hook ABI'],
};
fs.writeFileSync(path.join(root, 'target/build-verification.json'), JSON.stringify(result, null, 2) + '\n');
console.log(JSON.stringify(result, null, 2));
