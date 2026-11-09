const {test}=require('node:test'),assert=require('node:assert/strict'),fs=require('fs'),path=require('path'),os=require('os'),crypto=require('crypto'),AdmZip=require('adm-zip');
const compatibility=require('../mod-compatibility'),rollback=require('../mod-rollback'),{exportPack,loaderVersion}=require('../pack-export'),{importPack}=require('../pack-import');
const offline=async()=>{throw Error('offline')},hash=b=>crypto.createHash('sha512').update(b).digest('hex');
function temp(t){const root=fs.mkdtempSync(path.join(os.tmpdir(),'nova-tools-test-'));t.after(()=>fs.rmSync(root,{recursive:true,force:true}));fs.mkdirSync(path.join(root,'mods'));return root;}
function jar(root,name,metadata,extra=[]){const z=new AdmZip();if(metadata)z.addFile('fabric.mod.json',Buffer.from(JSON.stringify(metadata)));for(const [n,data]of extra)z.addFile(n,Buffer.from(data));z.writeZip(path.join(root,'mods',name));}
const inst={id:'test',name:'Export Test',version:'1.20.1',loader:'Fabric',loaderVersion:'0.15.11'};
test('version ranges distinguish compatibility and unknown syntax',()=>{
 assert.equal(compatibility.matches('1.20.1','>=1.20 <1.21'),true);assert.equal(compatibility.matches('1.21','1.20.x'),false);assert.equal(compatibility.matches('1.20.4','~1.20.1'),true);assert.equal(compatibility.matches('1.21','~1.20.1'),false);assert.equal(compatibility.matches('1.20.1','>= 1.20'),true);assert.equal(compatibility.matches('1.20.1',['1.19.x','1.20.x']),true);assert.equal(compatibility.matches('1.20.1','[1.20,1.21)'),null);
});
test('local compatibility detects duplicates, missing deps, conflicts and wrong game version',async t=>{
 const root=temp(t);jar(root,'a.jar',{id:'a',version:'1.0.0',depends:{minecraft:'>=1.21',missing:'*'},breaks:{b:'*'}});jar(root,'b.jar',{id:'b',version:'1.0.0'});jar(root,'copy.jar',{id:'b',version:'1.0.0'});jar(root,'disabled.jar.disabled',{id:'ignored',depends:{absent:'*'}});
 const r=await compatibility.check(root,inst,{request:offline});assert.equal(r.checked,3);assert.equal(r.disabled,1);assert(!r.online);for(const text of ['Дубликат','missing','minecraft','конфликт'])assert(r.issues.some(x=>x.message.includes(text)));
});
test('bundled Fabric dependencies and alternate mod loaders are handled',async t=>{
 const root=temp(t),nested=new AdmZip();nested.addFile('fabric.mod.json',Buffer.from(JSON.stringify({id:'bundled',version:'1.0.0'})));
 jar(root,'a.jar',{id:'a',version:'1.0.0',depends:{bundled:'*',minecraft:'1.20.x',fabricloader:'>=0.15'},jars:[{file:'META-INF/jars/bundled.jar'}]},[['META-INF/jars/bundled.jar',nested.toBuffer()]]);
 const r=await compatibility.check(root,inst,{request:offline});assert.equal(r.errors,0);
 const forge=await compatibility.check(root,{...inst,loader:'Forge'},{request:offline});assert(forge.issues.some(x=>x.message.includes('Мод для Fabric')));
});
test('unrecognized metadata stays unverified rather than compatible',async t=>{
 const root=temp(t);jar(root,'legacy.jar',null);const r=await compatibility.check(root,inst,{request:offline});assert.equal(r.errors,0);assert.equal(r.warnings,1);
});
test('export/import roundtrip preserves mods and configs but excludes personal game data',async t=>{
 const root=temp(t);jar(root,'a.jar',{id:'a',version:'1.0.0'});jar(root,'off.jar.disabled',{id:'off',version:'1.0.0'});
 for(const dir of ['config','saves','logs','backups','versions'])fs.mkdirSync(path.join(root,dir));
 fs.writeFileSync(path.join(root,'config','example.json'),'config');fs.writeFileSync(path.join(root,'config','accounts.json'),'secret');fs.writeFileSync(path.join(root,'saves','world.dat'),'world');fs.writeFileSync(path.join(root,'servers.dat'),'personal servers');fs.writeFileSync(path.join(root,'logs','latest.log'),'sensitive log');
 const out=path.join(root,'..',path.basename(root)+'.mrpack');t.after(()=>fs.rmSync(out,{force:true}));const result=exportPack(root,inst,out);assert(result.omitted.includes('config/accounts.json'));
 const zip=new AdmZip(out),names=zip.getEntries().map(x=>x.entryName);assert(names.includes('overrides/mods/off.jar.disabled'));assert(!names.some(x=>/saves|logs|servers.dat|accounts.json/.test(x)));assert.equal(JSON.parse(zip.readAsText('modrinth.index.json')).dependencies['fabric-loader'],'0.15.11');
 const imported=await importPack(out,{instancesDir:path.join(root,'imported'),request:offline});const importedRoot=path.join(root,'imported',imported.id);assert.equal(fs.readFileSync(path.join(importedRoot,'config/example.json'),'utf8'),'config');assert(fs.existsSync(path.join(importedRoot,'mods/off.jar.disabled')));
});
test('unknown loader version blocks incomplete export and can be recovered from installed profile',t=>{
 const root=temp(t);assert.throws(()=>exportPack(root,{...inst,loaderVersion:null},path.join(root,'bad.mrpack')),/точную версию/);
 fs.mkdirSync(path.join(root,'versions','fabric-profile'),{recursive:true});fs.writeFileSync(path.join(root,'versions','fabric-profile','fabric-profile.json'),JSON.stringify({inheritsFrom:inst.version,libraries:[{name:'net.fabricmc:fabric-loader:0.16.10'}]}));assert.equal(loaderVersion(root,{...inst,loaderVersion:null}),'0.16.10');
});
function backup(root,{changed=false,disabled=false}={}){
 const name=disabled?'old.jar.disabled':'old.jar',to=disabled?'new.jar.disabled':'new.jar',old=Buffer.from('old'),current=Buffer.from('new'),dir=path.join(root,'backups/mods','test-backup');fs.mkdirSync(dir,{recursive:true});fs.writeFileSync(path.join(dir,name),old);fs.writeFileSync(path.join(root,'mods',to),changed?'changed':current);fs.writeFileSync(path.join(dir,'manifest.json'),JSON.stringify([{name,to,beforeHash:hash(old),afterHash:hash(current)}]));fs.writeFileSync(path.join(dir,'completed.json'),'{}');return {name,to,old,current,dir};
}
test('rollback restores old mod and preserves current version in a recovery copy',t=>{
 const root=temp(t),b=backup(root,{disabled:true});assert.equal(rollback.list(root).length,1);const result=rollback.restore(root,'test-backup');assert.deepEqual(fs.readFileSync(path.join(root,'mods',b.name)),b.old);assert(!fs.existsSync(path.join(root,'mods',b.to)));assert.deepEqual(fs.readFileSync(path.join(result.recovery,b.to)),b.current);assert.equal(rollback.list(root)[0].restored,true);assert.throws(()=>rollback.restore(root,'test-backup'),/уже восстановлена/);
});
test('rollback refuses modified files and invalid backup paths',t=>{
 const root=temp(t),b=backup(root,{changed:true});assert.throws(()=>rollback.restore(root,'test-backup'),/изменён после/);assert.equal(fs.readFileSync(path.join(root,'mods',b.to),'utf8'),'changed');assert.throws(()=>rollback.restore(root,'../test-backup'),/Некорректная/);
});
test('rollback failure recovers the current mod set',t=>{
 const root=temp(t),b=backup(root),write=fs.writeFileSync;fs.writeFileSync=function(file,...args){if(file===path.join(root,'mods',b.name))throw Error('disk full');return write.call(this,file,...args);};try{assert.throws(()=>rollback.restore(root,'test-backup'),/disk full/);}finally{fs.writeFileSync=write;}assert.deepEqual(fs.readFileSync(path.join(root,'mods',b.to)),b.current);assert(!fs.existsSync(path.join(root,'mods',b.name)));
});
