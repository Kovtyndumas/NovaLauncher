const {test}=require('node:test'),assert=require('node:assert/strict'),fs=require('fs'),path=require('path'),os=require('os'),crypto=require('crypto'),AdmZip=require('adm-zip'),create=require('../content-service');
const sha=b=>crypto.createHash('sha512').update(b).digest('hex');
function fixture(t){
 const root=fs.mkdtempSync(path.join(os.tmpdir(),'nova-archive-'));t.after(()=>fs.rmSync(root,{recursive:true,force:true}));
 const zip=new AdmZip();zip.addFile('pack.mcmeta',Buffer.from('{"pack":{"pack_format":15,"description":"Test"}}'));const bytes=zip.toBuffer(),projects={},versions={},data={},calls=[];
 function project(id,type,files=[id+'.zip'],deps=[]){projects[id]={id,title:id,project_type:type};const v={id:id+'-v',project_id:id,version_type:'release',game_versions:['1.20.1'],loaders:type==='resourcepack'?['minecraft']:['iris'],dependencies:deps,files:files.map((name,i)=>{data['https://cdn.modrinth.com/'+name]=bytes;return {filename:name,primary:i===0,url:'https://cdn.modrinth.com/'+name,hashes:{sha512:sha(bytes)},size:bytes.length}})};versions[id]=[v];return v;}
 const service=create({request:async(url,options)=>{calls.push({url,options});const u=new URL(url),parts=u.pathname.split('/');if(parts[2]==='project'){const id=decodeURIComponent(parts[3]);return {ok:true,json:async()=>parts[4]==='version'?versions[id]:projects[id]};}if(parts[2]==='version'){return {ok:true,json:async()=>Object.values(versions).flat().find(v=>v.id===parts[3])};}if(u.hostname==='cdn.modrinth.com')return {ok:true,arrayBuffer:async()=>data[url]};throw Error('Unexpected request '+url);}});
 return {root,service,project,versions,data,bytes,calls,inst:{version:'1.20.1',loader:'Fabric'}};
}
test('all content categories search the selected game version; ZIP types do not use the mod loader',()=>{
 for(const type of ['modpack','shader','resourcepack']){const facets=JSON.parse(new URL(create.searchURL({type,ver:'26.2',loader:'Fabric'})).searchParams.get('facets')).flat();assert(facets.includes('versions:26.2'));assert(!facets.includes('categories:fabric'));}
});
test('shader installs the ZIP rather than a primary unrelated file, persists status, and is idempotent',async t=>{
 const f=fixture(t);f.project('shader','shader',['source.jar','shader.zip']);assert.equal(await f.service.installContent('shader',f.inst,f.root,'shader'),'shader.zip');assert(fs.existsSync(path.join(f.root,'shaderpacks/shader.zip')));assert(!fs.existsSync(path.join(f.root,'mods')));assert.deepEqual(f.service.listContent(f.root,'shader'),[{name:'shader.zip',project:'shader'}]);const before=f.calls.filter(x=>x.url==='https://cdn.modrinth.com/shader.zip').length;await f.service.installContent('shader',f.inst,f.root,'shader');assert.equal(f.calls.filter(x=>x.url==='https://cdn.modrinth.com/shader.zip').length,before);
});
test('resource pack dependencies keep their type and exact version',async t=>{
 const f=fixture(t);f.project('parent','resourcepack',['parent.zip'],[{dependency_type:'required',project_id:'base',version_id:'base-v'}]);f.project('base','resourcepack',['base.zip']);await f.service.installContent('parent',f.inst,f.root,'resourcepack');assert.deepEqual(fs.readdirSync(path.join(f.root,'resourcepacks')).sort(),['base.zip','parent.zip']);assert(f.calls.some(x=>x.url.endsWith('/version/base-v')));assert(!fs.existsSync(path.join(f.root,'mods')));
});
test('corrupt download writes no content or installation registry',async t=>{
 const f=fixture(t);f.project('pack','resourcepack');f.data['https://cdn.modrinth.com/pack.zip']=Buffer.from('corrupt');await assert.rejects(f.service.installContent('pack',f.inst,f.root,'resourcepack'),/контрольная сумма/);assert.deepEqual(fs.readdirSync(f.root),[]);
});
test('wrong game version and wrong project type cannot install content',async t=>{
 const f=fixture(t),v=f.project('pack','resourcepack');v.game_versions=['1.21'];await assert.rejects(f.service.installContent('pack',f.inst,f.root,'resourcepack'),/Minecraft 1.20.1/);await assert.rejects(f.service.installContent('pack',f.inst,f.root,'shader'),/Тип проекта/);assert.deepEqual(fs.readdirSync(f.root),[]);
});
test('existing user files are preserved on filename collision',async t=>{
 const f=fixture(t);f.project('pack','resourcepack');fs.mkdirSync(path.join(f.root,'resourcepacks'));fs.writeFileSync(path.join(f.root,'resourcepacks/pack.zip'),'user file');await assert.rejects(f.service.installContent('pack',f.inst,f.root,'resourcepack'),/уже существует/);assert.equal(fs.readFileSync(path.join(f.root,'resourcepacks/pack.zip'),'utf8'),'user file');
});
test('registry write failure rolls back every newly installed archive',async t=>{
 const f=fixture(t);f.project('parent','resourcepack',['parent.zip'],[{dependency_type:'required',project_id:'base'}]);f.project('base','resourcepack',['base.zip']);const write=fs.writeFileSync;fs.writeFileSync=function(file,...args){if(String(file).includes('nova-content.json.'))throw Error('disk full');return write.call(this,file,...args);};try{await assert.rejects(f.service.installContent('parent',f.inst,f.root,'resourcepack'),/disk full/);}finally{fs.writeFileSync=write;}assert.deepEqual(fs.readdirSync(path.join(f.root,'resourcepacks')),[]);assert(!fs.existsSync(path.join(f.root,'nova-content.json')));
});
test('archive dependency on an unavailable mod fails before writing the archive',async t=>{
 const f=fixture(t);f.project('parent','resourcepack',['parent.zip'],[{dependency_type:'required',project_id:'mod'}]);f.project('mod','mod',['mod.jar']);await assert.rejects(f.service.installContent('parent',{...f.inst,loader:'Vanilla'},f.root,'resourcepack'),/Для модов/);assert.deepEqual(fs.readdirSync(f.root),[]);
});
test('modpack selection respects the requested game version and chooses the mrpack file',async t=>{
 const f=fixture(t),v=f.project('pack','modpack',['server.zip','client.mrpack']);f.versions.pack.unshift({...v,id:'newer',game_versions:['26.3']});const target=path.join(f.root,'download.mrpack');const selected=await f.service.downloadPack('pack','1.20.1',target);assert.equal(selected.id,'pack-v');assert.deepEqual(fs.readFileSync(target),f.bytes);assert(f.calls.some(x=>x.url.includes('game_versions=')&&decodeURIComponent(x.url).includes('1.20.1')));
});

test('ZIP and required mods are committed together and rolled back on failure',async t=>{
 const f=fixture(t);f.project('parent','resourcepack',['parent.zip'],[{dependency_type:'required',project_id:'mod'}]);const v=f.project('mod','mod',['mod.jar']);v.loaders=['fabric'];
 const write=fs.writeFileSync;fs.writeFileSync=function(file,...args){if(String(file).includes('nova-content.json.'))throw Error('disk full');return write.call(this,file,...args);};
 try{await assert.rejects(f.service.installContent('parent',f.inst,f.root,'resourcepack'),/disk full/);}finally{fs.writeFileSync=write;}
 assert.deepEqual(fs.readdirSync(path.join(f.root,'mods')),[]);assert.deepEqual(fs.readdirSync(path.join(f.root,'resourcepacks')),[]);
 await f.service.installContent('parent',f.inst,f.root,'resourcepack');assert(fs.existsSync(path.join(f.root,'mods/mod.jar')));assert(fs.existsSync(path.join(f.root,'resourcepacks/parent.zip')));
});
