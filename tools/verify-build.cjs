// 校验本项目 ELF/NRO；提供 Forge NSO 和 GU ExeFS 时再核对真实导出与原版指令。
const fs = require('fs'), path = require('path'), crypto = require('crypto'), assert = require('assert/strict');
const root = path.resolve(__dirname, '..');
const stem = fs.readFileSync(path.join(root, 'Cargo.toml'), 'utf8').match(/^name = "([^"]+)"/m)[1].replaceAll('-', '_');
const short = stem.replace(/^mhgu_/, '');
const health = short === 'target_health_display';
const elfPath = path.join(root, 'target/armv7a-none-eabi/release/lib' + stem + '.so');
const nroPath = path.join(root, 'dist', short + '.nro');
const elf = fs.readFileSync(elfPath), nro = fs.readFileSync(nroPath);
const u32 = (b, o) => b.readUInt32LE(o);
const string = (b, o) => b.subarray(o, b.indexOf(0, o)).toString('utf8');
const hash = b => crypto.createHash('sha256').update(b).digest('hex');
assert.ok(process.argv.length === 2 || process.argv.length === 4, '用法：node tools/verify-build.cjs ["Forge NSO" "MHGU ExeFS目录"]');
assert.equal(elf.subarray(0, 4).toString('hex'), '7f454c46');
assert.equal(elf[4], 1);
assert.equal(elf[5], 1);
assert.equal(elf.readUInt16LE(18), 40);
assert.equal(u32(elf, 36) & 0x600, 0x200, 'ARM32 soft-float ABI');
assert.equal(nro.subarray(0x10, 0x14).toString(), 'NRO0');
assert.equal(u32(nro, 0x18), nro.length);
const loads = [];
for (let i = 0; i < elf.readUInt16LE(44); i++) {
  const o = u32(elf, 28) + i * elf.readUInt16LE(42);
  if (u32(elf, o) === 1) loads.push({file:u32(elf,o+4),addr:u32(elf,o+8),size:u32(elf,o+16),memory:u32(elf,o+20),flags:u32(elf,o+24)});
}
assert.equal(loads.length, 3);
const page = n => Math.ceil(n / 0x1000) * 0x1000;
for (let i = 0; i < 3; i++) {
  const load = loads[i], skip = i === 0 ? 0x80 : 0;
  assert.equal(u32(nro, 0x20 + i * 8), load.addr);
  assert.equal(u32(nro, 0x24 + i * 8), page(load.size));
  assert.equal(load.addr % 0x1000, 0);
  assert.deepEqual(nro.subarray(load.addr+skip,load.addr+load.size),elf.subarray(load.file+skip,load.file+load.size));
}
assert.equal(u32(nro, 0x38), page(Math.max(0, loads[2].memory-page(loads[2].size))));
const mod = u32(nro, 4);
assert.equal(nro.subarray(mod, mod+4).toString(), 'MOD0');
const dynamic = mod+nro.readInt32LE(mod+4);
assert.ok(dynamic >= loads[2].addr && dynamic < nro.length);
const bssStart = mod+nro.readInt32LE(mod+8), bssEnd = mod+nro.readInt32LE(mod+12);
assert.ok(bssEnd >= bssStart && bssEnd <= nro.length+u32(nro,0x38));
for (let o=dynamic; u32(nro,o); o+=8) {
  assert.notEqual(u32(nro,o),22,'DT_TEXTREL forbidden');
  if (u32(nro,o)===30) assert.equal(u32(nro,o+4)&4,0,'DF_TEXTREL forbidden');
}
const sections = Array.from({length:elf.readUInt16LE(48)},(_,i)=>{
  const o=u32(elf,32)+i*elf.readUInt16LE(46);
  return {type:u32(elf,o+4),file:u32(elf,o+16),size:u32(elf,o+20),link:u32(elf,o+24),stride:u32(elf,o+36)};
});
let relocations=0;
for (const section of sections.filter(s=>s.type===9)) {
  for(let o=section.file;o<section.file+section.size;o+=section.stride||8) {
    if((u32(elf,o+4)&255)===0) continue;
    const target=u32(elf,o);
    assert.ok(loads.some(s=>(s.flags&2)&&target>=s.addr&&target+4<=s.addr+s.memory),'relocation outside writable segment');
    relocations++;
  }
}
const dynsym=sections.find(s=>s.type===11),dynstr=sections[dynsym.link];
const names=elf.subarray(dynstr.file,dynstr.file+dynstr.size),symbols=[];
for(let i=0;i<dynsym.size;i+=dynsym.stride) {
  const o=dynsym.file+i;
  symbols.push({name:string(names,u32(elf,o)),value:u32(elf,o+4),bind:elf[o+12]>>4,defined:elf.readUInt16LE(o+14)!==0});
}
const required=fs.readFileSync(path.join(root,'exports.txt'),'utf8').split(/\r?\n/).filter(s=>s&&!s.startsWith('#'));
for(const name of required) assert.ok(symbols.some(s=>s.defined&&s.name===name),'missing callback: '+name);
const imports=symbols.filter(s=>!s.defined&&s.bind===1).map(s=>s.name);
const logic=fs.readFileSync(path.join(root,'src/lib.rs'),'utf8');
const data=fs.readFileSync(path.join(root,'src/data.rs'),'utf8');
assert.ok(nro.includes(Buffer.from('savedata:/'+short+'.ini')),'save-root configuration path missing');
assert.ok(nro.includes(fs.readFileSync(path.join(root,'conf',short+'.ini'))),'embedded default INI differs from conf template');
for(const prefix of ['app:/nativeNX/plugins/','healthcfg:/forge/plugins/','sdmc:/forge/plugins/']) {
  assert.ok(!nro.includes(Buffer.from(prefix+short+'.ini')),'obsolete configuration path');
}
const checks=['ARM32 soft-float','NRO segments match ELF','MOD0/dynamic/BSS','writable relocations; no TEXTREL','callbacks exported','embedded conf template; save-root INI'];
if(health) {
  assert.ok(!imports.includes('forge_hook_create'),'health display must only read game state');
  const selection=logic.slice(logic.indexOf('unsafe fn selected_health('),logic.indexOf('unsafe fn loading_visible('));
  assert.ok(selection.indexOf('slot == 0')<selection.indexOf('loading_visible('));
  assert.match(selection,/!show_other_areas && unsafe \{ usable\(manager\) \}/);
  const draw=logic.slice(logic.indexOf('fn draw_health()'),logic.indexOf('fn draw_settings()'));
  assert.ok(draw.indexOf('loading_visible(')<draw.indexOf('DrawList::foreground()'));
  assert.ok(!/monster|\*const|\*mut/.test(data.slice(data.indexOf('pub struct Snapshot'),data.indexOf('pub struct State'))));
  assert.ok(!/\*const|\*mut|\bu32\b/.test(data.slice(data.indexOf('pub struct MonsterInfo'),data.indexOf('impl MonsterInfo'))));
  assert.match(logic,/read_u32\(work \+ MONSTER_SCALE\)/);
  assert.match(logic,/size_and_crown\([\s\S]*?scale_percent\(scale\)/);
  checks.push('no game hooks; owned render snapshots; selected target/loading/metadata access');
}
const result={nro:nroPath,bytes:nro.length,sha256:hash(nro),callbacks:required,strong_imports:imports,writable_relocations:relocations,checks};
if(process.argv[2]) {
  const nso=require('./nso.cjs');
  function map(file) {
    const bytes=fs.readFileSync(file),h=nso.nsoHeader(bytes);
    assert.equal(bytes.subarray(0,4).toString(),'NSO0');
    const memory=Buffer.alloc(h.dataMemOff+h.dataSize+h.bssSize);
    for(const [i,part] of ['text','ro','data'].entries()) {
      const off=h[part+'FileOff'],length=h[part+'FileSize'];
      const segment=h.flags&(1<<i)?nso.decompressSegment(bytes,off,length,h[part+'Size']):bytes.subarray(off,off+length);
      segment.copy(memory,h[part+'MemOff']);
    }
    const mod=u32(memory,4),tags={};
    assert.equal(memory.subarray(mod,mod+4).toString(),'MOD0');
    for(let o=mod+memory.readInt32LE(mod+4);u32(memory,o);o+=8) tags[u32(memory,o)]=u32(memory,o+4);
    const count=tags[4]?u32(memory,tags[4]+4):(tags[5]-tags[6])/16,symbols=[];
    for(let i=0;i<count;i++) {
      const o=tags[6]+16*i;
      if(memory.readUInt16LE(o+14)!==0) symbols.push({name:string(memory,tags[5]+u32(memory,o)),value:u32(memory,o+4)});
    }
    return {memory,symbols};
  }
  const runtimePath=path.resolve(process.argv[2]),runtime=map(runtimePath),providers=new Map(runtime.symbols.map(s=>[s.name,'Forge']));
  for(const name of ['main','sdk','rtld','subsdk0']) {
    const module=map(path.join(process.argv[3],name));
    for(const symbol of module.symbols) if(!providers.has(symbol.name)) providers.set(symbol.name,'GU '+name);
    if(name==='main') {
      let signatures;
      if(health) {
        const constants=Object.fromEntries([...data.matchAll(/pub const (\w+): u32 = (0x[0-9a-fA-F]+|\d+);/g)].map(m=>[m[1],Number(m[2])]));
        signatures=[...data.matchAll(/\((\w+)(?: \+ (\d+))?, (0x[0-9a-fA-F]+)\)/g)].map(m=>[constants[m[1]]+Number(m[2]||0),Number(m[3])]);
      } else signatures=[...logic.slice(logic.indexOf('fn compatible('),logic.indexOf('// 当前加载器')).matchAll(/\((0x[0-9A-F]+), (0x[0-9A-F]+)\)/g)].map(m=>[Number(m[1]),Number(m[2])]);
      assert.equal(signatures.length,health?31:12);
      for(const [offset,expected] of signatures) assert.equal(u32(module.memory,offset),expected,'main word at 0x'+offset.toString(16));
      result.main_signatures=signatures.length;
    }
  }
  for(const name of imports) assert.ok(providers.has(name),'unresolved strong import: '+name);
  result.import_providers=Object.fromEntries(imports.map(name=>[name,providers.get(name)]));
  result.runtime=runtimePath;result.runtime_sha256=hash(fs.readFileSync(runtimePath));
  checks.push('strong imports provided by Forge + GU','original main instructions');
  if(!health) {
    const offset=runtime.symbols.find(s=>s.name==='forge_hook_create').value,memory=runtime.memory;
    const hookBytes=u32(memory,offset+0x24)===0xE3A0204C?76:[0xE1C200F0,0xE1C200F8,0xE1C201F0].every((word,i)=>u32(memory,offset+0xA4+i*8)===word)?24:0;
    assert.ok(hookBytes,'unrecognized Forge Hook return implementation');
    result.hook_return_bytes=hookBytes;result.hook_storage_bytes=76;
    checks.push('known 24/76-byte Forge Hook ABI fits explicit storage');
  }
}
fs.writeFileSync(path.join(root,'target/build-verification.json'),JSON.stringify(result,null,2)+'\n');
console.log('PASS: '+short+': '+nro.length+' bytes; '+checks.join('; '));
