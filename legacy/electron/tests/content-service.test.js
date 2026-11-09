const {test}=require('node:test'),assert=require('node:assert/strict'),fs=require('fs'),path=require('path'),os=require('os'),crypto=require('crypto'),create=require('../content-service');
const sha=b=>crypto.createHash('sha512').update(b).digest('hex'),inst={version:'1.20.1',loader:'Fabric'};
function setup(t,{missing=false,corrupt=false}={}){
 const root=fs.mkdtempSync(path.join(os.tmpdir(),'nova-content-'));t.after(()=>fs.rmSync(root,{recursive:true,force:true}));
 const data={a:Buffer.from('mod-a'),dep:Buffer.from('mod-dep')};
 const ver=(project,id,type='release')=>({project_id:project,id,version_type:type,game_versions:['1.20.1'],loaders:['fabric'],files:[{primary:true,filename:project+'.jar',url:'https://cdn.modrinth.com/'+project,hashes:{sha512:sha(data[project])}}],dependencies:project==='a'?[{dependency_type:'required',project_id:'dep',version_id:'dep-exact'}]:[]});
 const requests=[];
 const service=create({request:async(url,options)=>{requests.push(url);if(url.includes('/project/a/version'))return {ok:true,json:async()=>[ver('a','beta','beta'),ver('a','a-release')]};if(url.endsWith('/version/dep-exact'))return missing?{ok:false,status:404}:{ok:true,json:async()=>ver('dep','dep-exact')};const name=url.split('/').at(-1);return {ok:true,arrayBuffer:async()=>corrupt?Buffer.from('broken'):data[name]};}});
 return {root,service,data,requests};
}
test('automatic catalog search uses version, loader, client compatibility and popularity',()=>{
 const url=new URL(create.searchURL({type:'mod',loader:'Fabric',ver:'1.20.1'}));assert.equal(url.searchParams.get('index'),'downloads');const f=JSON.parse(url.searchParams.get('facets'));assert(f.flat().includes('versions:1.20.1'));assert(f.flat().includes('categories:fabric'));assert.equal(create.searchURL({type:'mod',loader:'Vanilla',ver:'1.20.1'}),null);
});
test('installs stable compatible version and exact required dependency',async t=>{
 const f=setup(t);assert.equal(await f.service.installMod('a',inst,f.root),'a.jar');assert.deepEqual(fs.readFileSync(path.join(f.root,'mods/a.jar')),f.data.a);assert.deepEqual(fs.readFileSync(path.join(f.root,'mods/dep.jar')),f.data.dep);assert(f.requests.some(x=>x.endsWith('/version/dep-exact')));
});
test('missing dependency fails explicitly before writing any mod',async t=>{
 const f=setup(t,{missing:true});await assert.rejects(f.service.installMod('a',inst,f.root),/HTTP 404/);assert(!fs.existsSync(path.join(f.root,'mods')));
});
test('bad checksum never installs a partial mod set',async t=>{
 const f=setup(t,{corrupt:true});await assert.rejects(f.service.installMod('a',inst,f.root),/Повреждён/);assert(!fs.existsSync(path.join(f.root,'mods')));
});
test('Vanilla cannot receive mod files',async t=>{const f=setup(t);await assert.rejects(f.service.installMod('a',{...inst,loader:'Vanilla'},f.root),/создайте инстанс/);});
