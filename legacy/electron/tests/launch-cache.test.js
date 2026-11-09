const {test}=require('node:test'),assert=require('node:assert/strict'),fs=require('fs'),path=require('path'),os=require('os'),crypto=require('crypto');
const {patchHandler}=require('../launch-cache'),modFiles=require('../mod-files');
function temp(t){const root=fs.mkdtempSync(path.join(os.tmpdir(),'nova-cache-test-'));t.after(()=>fs.rmSync(root,{recursive:true,force:true}));return root;}
test('unchanged verified resources reuse checksum cache; edited files are rehashed',async t=>{
 const root=temp(t),name=path.join(root,'asset');fs.writeFileSync(name,'correct');let hashes=0;const sum=crypto.createHash('sha1').update('correct').digest('hex');
 const handler={async checkSum(expected,file){hashes++;return crypto.createHash('sha1').update(fs.readFileSync(file)).digest('hex')===expected;},async getAssets(){assert(await this.checkSum(sum,name));}};
 patchHandler(handler,{root,version:{number:'26.2'}});await handler.getAssets();assert.equal(hashes,1);await handler.getAssets();assert.equal(hashes,1);fs.writeFileSync(name,'changed');assert.equal(await handler.checkSum(sum,name),false);assert.equal(hashes,2);
});
test('checksum cache persists between launcher sessions',async t=>{
 const root=temp(t),name=path.join(root,'asset');fs.writeFileSync(name,'data');const handler={checkSum:async()=>true,getAssets:async function(){await this.checkSum('hash',name);}};patchHandler(handler,{root,version:{number:'26.2'}});await handler.getAssets();
 let reads=0;const second={checkSum:async()=>{reads++;return true;}};patchHandler(second,{root,version:{number:'26.2'}});assert(await second.checkSum('hash',name));assert.equal(reads,0);
});
test('cached vanilla metadata is reused for Fabric without a manifest request',async t=>{
 const root=temp(t),dir=path.join(root,'cache/json');fs.mkdirSync(dir,{recursive:true});const metadata={id:'26.2',downloads:{client:{url:'https://example.org/game.jar'}},libraries:[]};fs.writeFileSync(path.join(dir,'26.2.json'),JSON.stringify(metadata));const handler={getVersion:async()=>{throw Error('unexpected network')}};patchHandler(handler,{root,directory:path.join(root,'versions/fabric'),version:{number:'26.2'}});assert.deepEqual(await handler.getVersion(),metadata);
});
test('removal is reversible and restoration refuses to overwrite another mod',t=>{
 const root=temp(t);fs.mkdirSync(path.join(root,'mods'));fs.writeFileSync(path.join(root,'mods/a.jar'),'original');const removed=modFiles.remove(root,'a.jar');assert(!fs.existsSync(path.join(root,'mods/a.jar')));assert.equal(modFiles.list(root).length,1);fs.writeFileSync(path.join(root,'mods/a.jar'),'different');assert.throws(()=>modFiles.restore(root,removed.backup),/уже существует/);fs.unlinkSync(path.join(root,'mods/a.jar'));modFiles.restore(root,removed.backup);assert.equal(fs.readFileSync(path.join(root,'mods/a.jar'),'utf8'),'original');assert(modFiles.list(root)[0].restored);assert.throws(()=>modFiles.remove(root,'../a.jar'),/Некорректное/);
});
test('unchanged mod set skips repeated startup ZIP scans; removals invalidate compatibility cache',async t=>{
 const root=temp(t),cache=require('../compatibility-cache');fs.mkdirSync(path.join(root,'mods'));fs.writeFileSync(path.join(root,'mods','a.jar'),'mod');let scans=0;const inspect=async()=>{scans++;return {issues:[],errors:0,warnings:0,checked:1,online:false};};const inst={version:'26.2',loader:'Fabric'};
 await cache.check(root,inst,{},inspect);assert.equal(scans,1);const warm=await cache.check(root,inst,{},inspect);assert.equal(scans,1);assert(warm.cached);fs.renameSync(path.join(root,'mods','a.jar'),path.join(root,'mods','a.jar.disabled'));await cache.check(root,inst,{},inspect);assert.equal(scans,2);await cache.check(root,{...inst,version:'26.3'},{},inspect);assert.equal(scans,3);
});
