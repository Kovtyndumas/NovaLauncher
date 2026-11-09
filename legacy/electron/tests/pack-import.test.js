const {test}=require('node:test'),assert=require('node:assert/strict'),fs=require('fs'),path=require('path'),os=require('os'),crypto=require('crypto'),AdmZip=require('adm-zip');
const {importPack,inside}=require('../pack-import');
function fixture(t,mutate=()=>{}){
 const root=fs.mkdtempSync(path.join(os.tmpdir(),'nova-pack-test-'));t.after(()=>fs.rmSync(root,{recursive:true,force:true}));
 const data=Buffer.from('verified mod'),hash=crypto.createHash('sha512').update(data).digest('hex');
 const idx={formatVersion:1,game:'minecraft',name:'Test Pack',versionId:'1.0',dependencies:{minecraft:'1.20.1','fabric-loader':'0.15.11'},files:[{path:'mods/example.jar',fileSize:data.length,hashes:{sha512:hash},downloads:['https://cdn.modrinth.com/example.jar']},{path:'mods/server.jar',env:{client:'unsupported'}}]};mutate(idx);
 const zip=new AdmZip();zip.addFile('modrinth.index.json',Buffer.from(JSON.stringify(idx)));zip.addFile('overrides/config/test.json',Buffer.from('base'));zip.addFile('client-overrides/config/test.json',Buffer.from('client'));zip.addFile('server-overrides/config/server.json',Buffer.from('server'));
 const file=path.join(root,'test.mrpack');zip.writeZip(file);
 return {root,file,data,instancesDir:path.join(root,'instances'),request:async()=>({ok:true,status:200,headers:{get:()=>null},arrayBuffer:async()=>data})};
}
test('imports into a new instance with pinned loader, verified downloads and client overrides',async t=>{
 const f=fixture(t),progress=[];const inst=await importPack(f.file,{...f,progress:p=>progress.push(p)});
 assert.equal(inst.loader,'Fabric');assert.equal(inst.loaderVersion,'0.15.11');assert.equal(inst.version,'1.20.1');
 const root=path.join(f.instancesDir,inst.id);assert.deepEqual(fs.readFileSync(path.join(root,'mods/example.jar')),f.data);assert.equal(fs.readFileSync(path.join(root,'config/test.json'),'utf8'),'client');assert(!fs.existsSync(path.join(root,'mods/server.jar')));assert(!fs.existsSync(path.join(root,'config/server.json')));assert.equal(progress.at(-1).done,progress.at(-1).total);
});
test('failed checksum removes staging directory and does not publish instance',async t=>{
 const f=fixture(t);f.request=async()=>({ok:true,status:200,headers:{get:()=>null},arrayBuffer:async()=>Buffer.from('corrupt data')});await assert.rejects(importPack(f.file,f),/контрольная сумма/);assert.deepEqual(fs.readdirSync(f.instancesDir),[]);
});
test('manifest path traversal cannot escape the new instance',async t=>{
 const f=fixture(t,idx=>idx.files[0].path='../escape.jar');await assert.rejects(importPack(f.file,f),/Небезопасный путь/);assert.deepEqual(fs.readdirSync(f.instancesDir),[]);assert(!fs.existsSync(path.join(f.root,'escape.jar')));
});
test('unsupported archive reports a useful error',async t=>{
 const f=fixture(t,idx=>idx.formatVersion=2);await assert.rejects(importPack(f.file,f),/Неподдерживаемый формат/);
});
test('unsupported download source is rejected before network access',async t=>{
 const f=fixture(t,idx=>idx.files[0].downloads=['http://127.0.0.1/private']);await assert.rejects(importPack(f.file,f),/описание файла/);assert.deepEqual(fs.readdirSync(f.instancesDir),[]);
});
test('rejects Windows, absolute and sibling-prefix paths',()=>{
 const root=path.join(os.tmpdir(),'instances');for(const p of ['/tmp/evil','../instances-evil/file','C:/evil','mods\\evil','mods/../../evil','./mods/x'])assert.throws(()=>inside(root,p));assert.equal(inside(root,'config/test.json'),path.join(root,'config','test.json'));
});
