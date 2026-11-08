const {test}=require('node:test'),assert=require('node:assert/strict');
const fs=require('fs'),os=require('os'),path=require('path'),crypto=require('crypto');
const createUpdater=require('../mod-updates'),analyze=require('../crash-analysis');
const hash=b=>crypto.createHash('sha512').update(b).digest('hex');
function fixture(t,options={}){
  const root=fs.mkdtempSync(path.join(os.tmpdir(),'nova-update-test-'));t.after(()=>fs.rmSync(root,{recursive:true,force:true}));fs.mkdirSync(path.join(root,'mods'));
  const old=Buffer.from('old mod'),newFile=Buffer.from('new mod');
  const name=options.disabled?'old.jar.disabled':'old.jar';fs.writeFileSync(path.join(root,'mods',name),old);fs.writeFileSync(path.join(root,'mods','local.jar'),'unlisted mod');
  const original={id:'old',project_id:'project',version_number:'1.0',date_published:'2025-01-01'};
  const version={id:'new',project_id:'project',version_number:'2.0',date_published:'2026-01-01',game_versions:['1.20.1'],loaders:['fabric'],dependencies:options.dependencies||[],files:[{primary:true,filename:options.filename||'new.jar',url:'https://cdn.modrinth.com/new.jar',hashes:{sha512:hash(newFile)}}]};
  const requests=[];
  const updater=createUpdater({fetch:async(url,o)=>{requests.push({url,o});if(url.includes('/version_files'))return {ok:true,json:async()=>({[hash(old)]:url.endsWith('/update')?version:original})};return {ok:true,arrayBuffer:async()=>options.corrupt?Buffer.from('corrupt'):newFile};}});
  return {root,updater,requests,name,old,newFile};
}
test('detects compatible updates and unknown mods, sends compatibility filters',async t=>{
 const f=fixture(t),p=await f.updater.scan(f.root,{version:'1.20.1',loader:'Fabric'});assert.equal(p.updates.length,1);assert.deepEqual(p.unknown,['local.jar']);const body=JSON.parse(f.requests[1].o.body);assert.deepEqual(body.loaders,['fabric']);assert.deepEqual(body.game_versions,['1.20.1']);assert.deepEqual(body.version_types,['release']);
});
test('updates disabled mod without enabling it and backs up old bytes',async t=>{
 const f=fixture(t,{disabled:true}),r=await f.updater.apply(f.root,{version:'1.20.1',loader:'Fabric'},[f.name]);assert.equal(r.count,1);assert.deepEqual(fs.readFileSync(path.join(f.root,'mods','new.jar.disabled')),f.newFile);assert.deepEqual(fs.readFileSync(path.join(r.backup,f.name)),f.old);assert(!fs.existsSync(path.join(f.root,'mods',f.name)));assert(fs.existsSync(path.join(f.root,'mods','local.jar')));
});
test('bad checksum leaves original mod intact',async t=>{
 const f=fixture(t,{corrupt:true});await assert.rejects(f.updater.apply(f.root,{version:'1.20.1',loader:'Fabric'},[f.name]),/Повреждён/);assert.deepEqual(fs.readFileSync(path.join(f.root,'mods',f.name)),f.old);
});
test('missing required dependency prevents replacement',async t=>{
 const f=fixture(t,{dependencies:[{dependency_type:'required',project_id:'missing'}]});await assert.rejects(f.updater.apply(f.root,{version:'1.20.1',loader:'Fabric'},[f.name]),/Зависимости/);assert.deepEqual(fs.readFileSync(path.join(f.root,'mods',f.name)),f.old);
});
test('rejects remote path traversal and stale selection',async t=>{
 const f=fixture(t,{filename:'../escape.jar'});const p=await f.updater.scan(f.root,{version:'1.20.1',loader:'Fabric'});assert.equal(p.updates.length,0);await assert.rejects(f.updater.apply(f.root,{version:'1.20.1',loader:'Fabric'},[f.name]),/изменился/);
});
test('write failure rolls back originals',async t=>{
 const f=fixture(t),write=fs.writeFileSync;fs.writeFileSync=function(file,...args){if(file===path.join(f.root,'mods','new.jar'))throw Error('disk full');return write.call(this,file,...args);};
 try{await assert.rejects(f.updater.apply(f.root,{version:'1.20.1',loader:'Fabric'},[f.name]),/disk full/);}finally{fs.writeFileSync=write;}
 assert.deepEqual(fs.readFileSync(path.join(f.root,'mods',f.name)),f.old);assert(!fs.existsSync(path.join(f.root,'mods','new.jar')));
});
test('crash analysis distinguishes Java, memory, mods, graphics and unknown failures',()=>{
 assert.match(analyze('UnsupportedClassVersionError: class file version 65',1).title,/Java/);
 assert.match(analyze('java.lang.OutOfMemoryError: Java heap space',1).title,/памяти/);
 assert.match(analyze('Incompatible mod set!',1).title,/моды/);
 assert.match(analyze('MixinApplyError',1).title,/Конфликт/);
 assert.match(analyze('GLFW error 65542',1).title,/графики/);
 assert.equal(analyze('Game closed',0),null);assert.match(analyze('unknown error',9).advice,/не удалось/);
});
function mainFixture(t){
 const vm=require('vm'),{EventEmitter}=require('events');
 const root=fs.mkdtempSync(path.join(os.tmpdir(),'nova-main-test-'));t.after(()=>fs.rmSync(root,{recursive:true,force:true}));
 const inst={id:'fixture',name:'Test',version:'1.20.1',loader:'Vanilla'};
 fs.writeFileSync(path.join(root,'db.json'),JSON.stringify({instances:[inst],settings:{autoJava:false,name:'Test',ram:4}}));
 const handlers={},sent=[];let client,fail=false,deleteResponse=0;const trashed=[];
 class Client extends EventEmitter{constructor(){super();client=this;}async launch(options){this.options=options;if(fail)throw Error('fetch failed');return new EventEmitter();}}
 class Window{constructor(){this.webContents={send:(...args)=>sent.push(args)};}loadFile(){}isDestroyed(){return false;}}
 const fakeRequire=name=>{
  if(name==='./discord-presence')return {Presence:class{configure(){}update(){}stop(){}}};
  if(name==='electron')return {app:{getPath:()=>root,whenReady:()=>({then:fn=>fn()}),on(){}},BrowserWindow:Window,ipcMain:{handle:(name,fn)=>handlers[name]=fn},shell:{openPath(){},trashItem:async file=>{assert(file.startsWith(root+path.sep));trashed.push(file);fs.rmSync(file,{recursive:true,force:true});}},dialog:{showMessageBox:async()=>({response:deleteResponse}),showOpenDialog:async()=>({canceled:true,filePaths:[]})}};
  if(name==='minecraft-launcher-core')return {Client,Authenticator:{getAuth:()=>({})}};
  if(name==='msmc')return {Auth:class{}};
  if(name==='adm-zip')return class{};
  if(name.startsWith('./'))return require(path.join(__dirname,'..',name));return require(name);
 };
 vm.runInNewContext(fs.readFileSync(path.join(__dirname,'..','main.js'),'utf8'),{__dirname:path.join(__dirname,'..'),require:fakeRequire,process,Buffer,fetch:async()=>{throw Error('Unexpected network request')},URLSearchParams,AbortSignal,console});
 return {root,handlers,sent,inst,client:()=>client,setFail:x=>fail=x,confirmDelete:x=>deleteResponse=x,trashed};
}
test('clean game close is not reported as a crash; running game blocks updates',async t=>{
 const f=mainFixture(t);assert.equal(await f.handlers['game:launch'](null,f.inst),true);
 await assert.rejects(f.handlers['mods:update'](null,f.inst.id,[]),/Закройте игру/);
 f.client().emit('close',0);const last=f.sent.at(-1);assert.equal(last[0],'game:closed');assert.equal(last[1],0);assert.equal(last[2],null);
 assert.equal(await f.handlers['game:launch'](null,f.inst),true);f.client().emit('close',0);
});
test('nonzero game exit produces a persisted diagnosis with evidence',async t=>{
 const f=mainFixture(t);await f.handlers['game:launch'](null,f.inst);f.client().emit('data','UnsupportedClassVersionError: class file version 65');f.client().emit('close',1);
 const report=f.sent.at(-1)[2];assert.match(report.title,/Java/);assert.equal(report.instanceId,f.inst.id);assert.equal(report.evidence.length,1);assert.equal(f.handlers['game:lastCrash'](null,f.inst.id).title,report.title);
});
test('launch failure releases the game lock for a subsequent launch',async t=>{
 const f=mainFixture(t);f.setFail(true);await assert.rejects(f.handlers['game:launch'](null,f.inst),/fetch failed/);assert.match(f.sent.at(-1)[2].title,/сети/);
 f.setFail(false);assert.equal(await f.handlers['game:launch'](null,f.inst),true);f.client().emit('close',0);
});
test('canceling native pack picker releases mutation lock',async t=>{
 const f=mainFixture(t);assert.equal(await f.handlers['pack:import'](null),null);assert.equal(await f.handlers['game:launch'](null,f.inst),true);f.client().emit('close',0);
});
test('local pack IPC publishes a separate saved instance and progress',async t=>{
 const f=mainFixture(t),AdmZip=require('adm-zip'),zip=new AdmZip();zip.addFile('modrinth.index.json',Buffer.from(JSON.stringify({formatVersion:1,game:'minecraft',name:'IPC Test',versionId:'v1',dependencies:{minecraft:'1.20.1','fabric-loader':'0.15.11'},files:[]})));zip.addFile('overrides/options.txt',Buffer.from('fov:0.0'));
 const file=path.join(f.root,'input.mrpack');zip.writeZip(file);
 const inst=await f.handlers['pack:import'](null,file),db=JSON.parse(fs.readFileSync(path.join(f.root,'db.json')));
 assert.equal(db.instances.length,2);assert.equal(inst.loaderVersion,'0.15.11');assert.equal(db.instances[1].id,inst.id);assert(f.sent.some(x=>x[0]==='pack:progress'));assert.equal(fs.readFileSync(path.join(f.root,'instances',inst.id,'options.txt'),'utf8'),'fov:0.0');assert(fs.existsSync(file));
});
test('cached Fabric profile launches without a loader metadata request and uses modern quick play',async t=>{
 const f=mainFixture(t),file=path.join(f.root,'db.json'),db=JSON.parse(fs.readFileSync(file));db.instances[0].loader='Fabric';db.instances[0].loaderVersion='0.15.11';fs.writeFileSync(file,JSON.stringify(db));
 const root=path.join(f.root,'instances',f.inst.id),profile='fabric-loader-0.15.11-1.20.1';fs.mkdirSync(path.join(root,'versions',profile),{recursive:true});fs.writeFileSync(path.join(root,'versions',profile,profile+'.json'),'{}');fs.writeFileSync(path.join(root,'nova-loader.json'),JSON.stringify({id:profile,loader:'Fabric',version:'1.20.1',loaderVersion:'0.15.11'}));
 assert.equal(await f.handlers['game:launch'](null,db.instances[0],'example.org:25565'),true);assert.equal(f.client().options.version.custom,profile);assert.equal(f.client().options.quickPlay.type,'multiplayer');assert.equal(f.client().options.quickPlay.identifier,'example.org:25565');f.client().emit('close',0);
});
test('logs hide access tokens before reaching renderer or crash report',async t=>{
 const f=mainFixture(t);await f.handlers['game:launch'](null,f.inst);f.client().emit('debug','[MCLC]: Launching with arguments --accessToken private-token --uuid private-uuid');const log=f.sent.find(x=>x[0]==='game:log');assert(!log[1].includes('private-token'));assert(!log[1].includes('private-uuid'));f.client().emit('close',0);
});

test('instance deletion confirmation cancels without changes and releases the operation lock',async t=>{
 const f=mainFixture(t);const pending=f.handlers['instance:delete'](null,f.inst.id);await assert.rejects(f.handlers['game:launch'](null,f.inst),/уже запущена|обновление/);assert.equal(await pending,null);assert.equal(JSON.parse(fs.readFileSync(path.join(f.root,'db.json'))).instances.length,1);assert.equal(f.trashed.length,0);assert.equal(await f.handlers['game:launch'](null,f.inst),true);f.client().emit('close',0);
});
test('confirmed instance deletion removes its saved entry and trashes its files',async t=>{
 const f=mainFixture(t);f.confirmDelete(1);const dir=path.join(f.root,'instances',f.inst.id);fs.mkdirSync(dir,{recursive:true});fs.writeFileSync(path.join(dir,'options.txt'),'test');const r=await f.handlers['instance:delete'](null,f.inst.id);assert.equal(r.id,f.inst.id);assert.equal(r.db.instances.length,0);assert(!fs.existsSync(dir));assert.equal(f.trashed.length,1);assert(!r.db.settings.msRefresh);
});
test('running game prevents instance deletion',async t=>{const f=mainFixture(t);f.confirmDelete(1);await f.handlers['game:launch'](null,f.inst);await assert.rejects(f.handlers['instance:delete'](null,f.inst.id),/Закройте игру/);assert.equal(f.trashed.length,0);f.client().emit('close',0);});
test('instance tools and content mutations cannot run while the Java game is active',async t=>{const f=mainFixture(t);await f.handlers['game:launch'](null,f.inst);for(const [name,args]of [['instance:duplicate',[f.inst.id]],['worlds:backup',[f.inst.id]],['worlds:restore',[f.inst.id,'123-ab']],['content:manage',[f.inst.id,'shader','test.zip','remove']],['horizon:run',['play']]])await assert.rejects(f.handlers[name](null,...args),/Закройте игру/);f.client().emit('close',0);assert.equal(f.handlers['history:list']()[0].closed,true);});
test('offline version catalog uses its saved complete release list',async t=>{const f=mainFixture(t);fs.writeFileSync(path.join(f.root,'versions-cache.json'),JSON.stringify(['26.3','1.0']));assert.deepEqual(JSON.parse(JSON.stringify(await f.handlers['versions:list']())),{versions:['26.3','1.0'],cached:true});});
