// 安装本项目已验证的 NRO 和静态素材；不读写游戏存档 INI。
const fs=require('fs'),path=require('path'),crypto=require('crypto'),assert=require('assert/strict');
const root=path.resolve(__dirname,'..');
const id=fs.readFileSync(path.join(root,'Cargo.toml'),'utf8').match(/^name = "([^"]+)"/m)[1].replaceAll('-','_').replace(/^mhgu_/,'');
const hash=b=>crypto.createHash('sha256').update(b).digest('hex');
const assets=id==='target_health_display'?{
 'romfs/nativeNX/forge/fonts/NotoSansSC-Medium.otf':'assets/romfs/nativeNX/forge/fonts/NotoSansSC-Medium.otf',
 'licenses/NotoSansSC-LICENSE.txt':'assets/licenses/NotoSansSC-LICENSE.txt',
 'docs/target-health-font-source.json':'assets/font-source.json',
}:{};
function install(plugins) {
 const destination=path.resolve(plugins),modRoot=path.resolve(destination,'../../..');
 const bytes=fs.readFileSync(path.join(root,'dist',id+'.nro'));
 const verified=JSON.parse(fs.readFileSync(path.join(root,'target/build-verification.json'),'utf8'));
 assert.equal(hash(bytes),verified.sha256,'NRO changed since verification; run bash test.sh');
 const plan=[{bytes,target:path.join(destination,id+'.nro')},...Object.entries(assets).map(([target,source])=>({bytes:fs.readFileSync(path.join(root,source)),target:path.join(modRoot,target)}))];
 const backup=path.join(root,'target/install-backup');
 for(const item of plan) {
   if(fs.existsSync(item.target)) {
     const old=fs.readFileSync(item.target);
     fs.mkdirSync(backup,{recursive:true});
     fs.writeFileSync(path.join(backup,hash(old)+'-'+path.basename(item.target)),old);
   }
   fs.mkdirSync(path.dirname(item.target),{recursive:true});
   fs.writeFileSync(item.target,item.bytes);
   assert.deepEqual(fs.readFileSync(item.target),item.bytes);
 }
 const report={id,installed:plan.map(item=>({path:item.target,sha256:hash(item.bytes)})),backup,config_files_written:0,config_location:'MHGU current user save root',game_tested:false};
 fs.writeFileSync(path.join(root,'target/install-verification.json'),JSON.stringify(report,null,2)+'\n');
 return report;
}
if(require.main===module) {
 assert.equal(process.argv.length,3,'用法：node tools/install.cjs "Mod目录/romfs/nativeNX/plugins"');
 console.log(JSON.stringify(install(process.argv[2]),null,2));
}
module.exports={install,id,assets};
