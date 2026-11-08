const {test}=require('node:test'),assert=require('node:assert/strict'),fs=require('fs'),path=require('path'),os=require('os');
const store=require('../state-store'),{splitJvmArgs,quickPlay,neoPrefix,compareVersions}=require('../launch-options');
const instance=(id='one')=>({id,name:id,version:'1.20.1',loader:'Fabric'});
const db=()=>({instances:[instance()],servers:[],settings:{name:'Steve',ram:4,msRefresh:'private-refresh'}});
test('saving stale settings cannot delete an imported instance or lose backend loader metadata',()=>{
 const base=db(),current=structuredClone(base),incoming=structuredClone(base);current.instances.push(instance('imported'));current.instances[0].loaderVersion='0.16.1';incoming.settings.theme='light';const result=store.merge(current,incoming,base);assert.equal(result.instances.length,2);assert.equal(result.instances[0].loaderVersion,'0.16.1');assert.equal(result.settings.theme,'light');assert.equal(result.settings.msRefresh,'private-refresh');assert(!store.publicDb(result).settings.msRefresh);
});
test('explicit instance deletion is preserved while concurrent imports survive',()=>{
 const base=db(),current=structuredClone(base),incoming=structuredClone(base);current.instances.push(instance('imported'));incoming.instances=[];assert.deepEqual(store.merge(current,incoming,base).instances.map(x=>x.id),['imported']);
});
test('stale settings cannot resurrect a deleted instance',()=>{
 const base=db(),current=structuredClone(base),incoming=structuredClone(base);current.instances=[];incoming.settings.ram=6;assert.equal(store.merge(current,incoming,base).instances.length,0);
});
test('atomic save preserves old file on write failure; corrupted files are never silently reset',t=>{
 const dir=fs.mkdtempSync(path.join(os.tmpdir(),'nova-state-'));t.after(()=>fs.rmSync(dir,{recursive:true,force:true}));const file=path.join(dir,'db.json'),s=store.create(file);s.save(db());const before=fs.readFileSync(file,'utf8'),rename=fs.renameSync;fs.renameSync=()=>{throw Error('rename failed');};try{assert.throws(()=>s.save({...db(),servers:[{name:'new'}]}),/rename failed/);}finally{fs.renameSync=rename;}assert.equal(fs.readFileSync(file,'utf8'),before);fs.writeFileSync(file,'broken');assert.throws(()=>s.load(),/файл не изменён/);assert.equal(fs.readFileSync(file,'utf8'),'broken');
});
test('rejects path traversal in stored instance ids',()=>{const d=db();d.instances[0].id='../outside';assert.throws(()=>store.normalize(d),/Некорректные данные/);});
test('JVM arguments preserve quoted spaces and Windows paths',()=>{
 assert.deepEqual(splitJvmArgs('-Xmx4G -Dname="Hello World" "-Dpath=C:\\Program Files\\Java"'),['-Xmx4G','-Dname=Hello World','-Dpath=C:\\Program Files\\Java']);assert.throws(()=>splitJvmArgs('-Dname="unfinished'),/кавычка/);
});
test('server launch and NeoForge version selection support year-based releases',()=>{
 assert.equal(quickPlay('26.3','localhost:25565').type,'multiplayer');assert.equal(quickPlay('1.20','example.org').type,'multiplayer');assert.equal(quickPlay('1.19.4','example.org').type,'legacy');assert.equal(quickPlay('26.3','[::1]:25565').identifier,'[::1]:25565');assert.throws(()=>quickPlay('26.3','bad address'),/Некорректный/);
 assert.equal(neoPrefix('26.3'),'26.3.');assert.equal(neoPrefix('1.21.1'),'21.1.');assert.deepEqual(['21.1.9','21.1.100','21.1.20'].sort(compareVersions),['21.1.9','21.1.20','21.1.100']);
});
