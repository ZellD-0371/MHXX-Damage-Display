// 检查独立安装、重复安装及更新；用户配置、前置和其它插件必须保留。
const fs=require('fs'),path=require('path'),assert=require('assert/strict');
const {install,id,assets}=require('./install.cjs');
const root=path.resolve(__dirname,'..'),out=path.join(root,'target/install-check',Date.now().toString());
for(const mode of ['fresh','existing']) {
 const base=path.join(out,mode),mod=path.join(base,'mod'),plugins=path.join(mod,'romfs/nativeNX/plugins'),save=path.join(base,'save');
 const fonts=path.join(mod,'romfs/nativeNX/forge/fonts');
 for(const dir of [plugins,save,fonts,path.join(mod,'exefs')]) fs.mkdirSync(dir,{recursive:true});
 fs.writeFileSync(path.join(plugins,'other.nro'),'other plugin');
 fs.writeFileSync(path.join(mod,'exefs/subsdk9'),'original Forge');
 fs.writeFileSync(path.join(fonts,'Roboto-Medium.ttf'),'original font');
 if(mode==='existing') {
   fs.writeFileSync(path.join(save,id+'.ini'),'preserved user configuration');
   fs.writeFileSync(path.join(plugins,id+'.nro'),'previous NRO');
 }
 const first=install(plugins),again=install(plugins);
 assert.equal(first.installed.length,1+Object.keys(assets).length);
 assert.equal(again.config_files_written,0);
 assert.deepEqual(fs.readFileSync(path.join(plugins,id+'.nro')),fs.readFileSync(path.join(root,'dist',id+'.nro')));
 assert.equal(fs.readFileSync(path.join(plugins,'other.nro'),'utf8'),'other plugin');
 assert.equal(fs.readFileSync(path.join(mod,'exefs/subsdk9'),'utf8'),'original Forge');
 assert.equal(fs.readFileSync(path.join(fonts,'Roboto-Medium.ttf'),'utf8'),'original font');
 for(const [target,source] of Object.entries(assets)) assert.deepEqual(fs.readFileSync(path.join(mod,target)),fs.readFileSync(path.join(root,source)));
 assert.deepEqual(fs.readdirSync(save),mode==='existing'?[id+'.ini']:[]);
 if(mode==='existing') assert.equal(fs.readFileSync(path.join(save,id+'.ini'),'utf8'),'preserved user configuration');
 assert.ok(!fs.readdirSync(plugins).some(file=>file.endsWith('.ini')));
 assert.ok(!fs.existsSync(path.join(base,'sdcard')));
}
console.log('PASS: '+id+' installer; fresh/update/repeated install, own assets, saved INI and unrelated files preserved');
