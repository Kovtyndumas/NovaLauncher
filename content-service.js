const fs=require('fs'),path=require('path'),crypto=require('crypto');
const AdmZip=require('adm-zip');
const folders={shader:'shaderpacks',resourcepack:'resourcepacks'};
const sha=b=>crypto.createHash('sha512').update(b).digest('hex');
function searchURL({q='',type='mod',loader,ver}){
  if(!['mod','modpack','shader','resourcepack'].includes(type))throw Error('Неизвестный тип контента');
  const facets=[[`project_type:${type}`]];
  if(ver)facets.push([`versions:${ver}`]);
  if(type==='mod'){if(!['Fabric','Quilt','Forge','NeoForge'].includes(loader))return null;facets.push([`categories:${loader.toLowerCase()}`],['client_side:required','client_side:optional']);}
  return 'https://api.modrinth.com/v2/search?'+new URLSearchParams({query:q,limit:'24',index:q?'relevance':'downloads',facets:JSON.stringify(facets)});
}
module.exports=function({request=global.fetch,headers={}}={}){
  const json=async(url,options={})=>{const r=await request(url,{...options,headers:{...headers,...options.headers},signal:AbortSignal.timeout(30000)});if(!r.ok)throw Error('Modrinth: HTTP '+r.status);return r.json();};
  async function search(options){const url=searchURL(options);return url?(await json(url)).hits||[]:[];}
  async function installMod(project,inst,root,exact=null,prepare=false){
    if(inst.loader==='Vanilla')throw Error('Для модов создайте инстанс Fabric, Quilt, Forge или NeoForge');
    const dir=path.join(root,'mods'),local=fs.existsSync(dir)?fs.readdirSync(dir).filter(x=>/\.jar(\.disabled)?$/.test(x)).map(name=>({name,hash:sha(fs.readFileSync(path.join(dir,name)))})):[];
    const known=local.length?await json('https://api.modrinth.com/v2/version_files',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({hashes:local.map(x=>x.hash),algorithm:'sha512'})}):{};
    const planned=new Map();
    async function resolve(projectId,exact=null,depth=0){
      if(depth>20)throw Error('Слишком длинная цепочка зависимостей');
      let v;
      if(exact)v=await json('https://api.modrinth.com/v2/version/'+encodeURIComponent(exact));
      else{
        const params=new URLSearchParams({game_versions:JSON.stringify([inst.version]),loaders:JSON.stringify([inst.loader.toLowerCase()])});
        const versions=await json('https://api.modrinth.com/v2/project/'+encodeURIComponent(projectId)+'/version?'+params);
        v=versions.find(x=>x.version_type==='release');
      }
      if(!v||!v.game_versions.includes(inst.version)||!v.loaders.includes(inst.loader.toLowerCase()))throw Error('Нет совместимой стабильной версии '+projectId+' для '+inst.loader+' '+inst.version);
      const previous=planned.get(v.project_id);if(previous){if(exact&&previous.id!==exact)throw Error('Зависимости требуют разные версии одного мода');return previous.filename;}
      const installed=local.find(x=>known[x.hash]?.project_id===v.project_id);
      if(installed){if(!known[installed.hash].game_versions.includes(inst.version)||!known[installed.hash].loaders.includes(inst.loader.toLowerCase()))throw Error('Уже установлен несовместимый мод '+installed.name+'. Сначала проверьте совместимость сборки');if(installed.name.endsWith('.disabled'))throw Error('Включите установленный мод '+installed.name+' и повторите установку');if(exact&&known[installed.hash].id!==exact)throw Error('Обновите зависимость '+installed.name+' через «Обновление модов»');return installed.name;}
      const file=v.files.find(f=>f.primary)||v.files[0];if(!file||path.basename(file.filename)!==file.filename||/[\\/\x00]/.test(file.filename)||!file.filename.endsWith('.jar')||!file.hashes?.sha512)throw Error('Некорректный файл мода');
      if(new URL(file.url).protocol!=='https:')throw Error('Небезопасная ссылка мода');
      planned.set(v.project_id,{...v,filename:file.filename,file});
      for(const dep of v.dependencies||[]){if(dep.dependency_type==='required'){if(!dep.project_id&&!dep.version_id)throw Error('Мод требует ручную установку зависимости: '+(dep.file_name||'неизвестная'));await resolve(dep.project_id,dep.version_id,depth+1);}}
      return file.filename;
    }
    const filename=await resolve(project,exact),targets=new Set(),buffers=[];
    const present=[...planned.values(),...local.filter(x=>!x.name.endsWith('.disabled')).map(x=>known[x.hash]).filter(Boolean)];
    for(const item of planned.values())for(const d of item.dependencies||[])if(d.dependency_type==='incompatible'&&present.some(v=>(!d.project_id||v.project_id===d.project_id)&&(!d.version_id||v.id===d.version_id)))throw Error('Конфликт обязательных модов: '+item.filename);
    for(const item of planned.values()){
      if(targets.has(item.filename)||fs.existsSync(path.join(dir,item.filename))||fs.existsSync(path.join(dir,item.filename+'.disabled')))throw Error('Файл уже существует: '+item.filename);targets.add(item.filename);
      const r=await request(item.file.url,{headers,signal:AbortSignal.timeout(120000)});if(!r.ok)throw Error('Не удалось скачать '+item.filename);const b=Buffer.from(await r.arrayBuffer());if(sha(b)!==item.file.hashes.sha512)throw Error('Повреждён файл '+item.filename);buffers.push([item.filename,b]);
    }
    if(prepare)return {filename,versions:[...planned.values()],files:buffers.map(([name,bytes])=>({target:path.join(dir,name),bytes}))};
    fs.mkdirSync(dir,{recursive:true});const written=[];
    try{for(const [name,b]of buffers){fs.writeFileSync(path.join(dir,name),b,{flag:'wx'});written.push(name);}}catch(e){for(const name of written)fs.unlinkSync(path.join(dir,name));throw e;}
    return filename;
  }

  const projectInfo=id=>json('https://api.modrinth.com/v2/project/'+encodeURIComponent(id));
  function archiveFile(v,extension){
    const files=(v.files||[]).filter(f=>typeof f.filename==='string'&&f.filename.toLowerCase().endsWith(extension));
    const file=files.find(f=>f.primary)||files[0];
    if(!file||path.basename(file.filename)!==file.filename||/[\\/:\x00]/.test(file.filename)||!/^([a-f0-9]{128})$/i.test(file.hashes?.sha512||''))throw Error('У версии нет корректного файла '+extension);
    let url;try{url=new URL(file.url);}catch{throw Error('Некорректная ссылка контента');}
    if(url.protocol!=='https:'||url.username||url.password)throw Error('Небезопасная ссылка контента');
    return file;
  }
  async function selectVersion(project,{version,type,exact}={}){
    const versions=exact?[await json('https://api.modrinth.com/v2/version/'+encodeURIComponent(exact))]:await json('https://api.modrinth.com/v2/project/'+encodeURIComponent(project)+'/version?'+new URLSearchParams(version?{game_versions:JSON.stringify([version])}:{}));
    if(!Array.isArray(versions))throw Error('Modrinth вернул некорректный список версий');
    const extension=type==='modpack'?'.mrpack':'.zip',matches=versions.filter(v=>(!version||v.game_versions?.includes(version))&&(!v.project_id||v.project_id===project)&&v.files?.some(f=>f.filename?.toLowerCase().endsWith(extension)));
    const v=matches.find(v=>v.version_type==='release')||matches.find(v=>v.version_type==='beta')||matches[0];
    if(!v)throw Error('Нет файла '+extension+(version?' для Minecraft '+version:''));
    return {version:v,file:archiveFile(v,extension)};
  }
  async function downloadFile(file){
    const r=await request(file.url,{headers,signal:AbortSignal.timeout(180000)});
    if(!r.ok)throw Error('Загрузка '+file.filename+': HTTP '+r.status);
    const bytes=Buffer.from(await r.arrayBuffer());
    if(bytes.length>512*1024*1024||(Number.isSafeInteger(file.size)&&bytes.length!==file.size)||sha(bytes)!==file.hashes.sha512.toLowerCase())throw Error('Не совпала контрольная сумма '+file.filename+'. Повторите установку');
    try{if(!new AdmZip(bytes).getEntries().length)throw Error();}catch{throw Error('Повреждён архив '+file.filename);}
    return bytes;
  }
  function listContent(root,type){
    const folder=folders[type];if(!folder)throw Error('Неизвестный тип контента');
    let records=[];try{const r=JSON.parse(fs.readFileSync(path.join(root,'nova-content.json'),'utf8'));if(Array.isArray(r))records=r;}catch{}
    try{return fs.readdirSync(path.join(root,folder),{withFileTypes:true}).filter(e=>e.isFile()&&/\.zip$/i.test(e.name)).map(e=>({name:e.name,project:records.find(r=>r.type===type&&r.filename===e.name)?.project||null}));}catch(e){if(e.code==='ENOENT')return [];throw e;}
  }
  async function installContent(project,inst,root,type){
    if(!folders[type])throw Error('Неизвестный тип контента');
    const planned=new Map(),mods=new Map();
    async function resolve(id,exact,expected,depth=0){
      if(depth>20)throw Error('Слишком длинная цепочка зависимостей');
      if(!id&&exact)id=(await json('https://api.modrinth.com/v2/version/'+encodeURIComponent(exact))).project_id;
      if(!id)throw Error('Не удалось определить зависимость контента');
      const p=await projectInfo(id);
      if(expected&&p.project_type!==expected)throw Error('Тип проекта на Modrinth изменился. Обновите каталог');
      if(p.project_type==='mod'){const previous=mods.get(p.id);if(previous&&exact&&previous.exact&&previous.exact!==exact)throw Error('Зависимости требуют разные версии мода');mods.set(p.id,{exact:exact||previous?.exact});return;}
      if(!folders[p.project_type])throw Error('Неподдерживаемая зависимость контента: '+p.title);
      const previous=planned.get(p.id);if(previous){if(exact&&previous.version.id!==exact)throw Error('Зависимости требуют разные версии контента');return previous.file.filename;}
      const item=await selectVersion(p.id,{version:inst.version,type:p.project_type,exact});
      planned.set(p.id,{...item,project:p.id,type:p.project_type});
      for(const d of item.version.dependencies||[])if(d.dependency_type==='required'){
        if(!d.project_id&&!d.version_id)throw Error('Требуется ручная установка зависимости: '+(d.file_name||p.title));
        await resolve(d.project_id,d.version_id,null,depth+1);
      }
      return item.file.filename;
    }
    const filename=await resolve(project,null,type),targets=new Set(),pending=[];
    for(const item of planned.values()){
      const key=folders[item.type]+'/'+item.file.filename.toLowerCase();if(targets.has(key))throw Error('Повтор файла контента');targets.add(key);
      const target=path.join(root,folders[item.type],item.file.filename);
      if(fs.existsSync(target)){if(sha(fs.readFileSync(target))===item.file.hashes.sha512.toLowerCase())continue;throw Error('Файл '+item.file.filename+' уже существует. Переименуйте его в папке контента');}
      pending.push({target,bytes:await downloadFile(item.file)});
    }
    const modVersions=new Map(),modTargets=new Map();
    for(const [id,{exact}]of mods){const plan=await installMod(id,inst,root,exact,true);
      for(const v of plan.versions){const previous=modVersions.get(v.project_id);if(previous&&previous.id!==v.id)throw Error('Зависимости требуют разные версии одного мода');modVersions.set(v.project_id,v);}
      for(const item of plan.files){const previous=modTargets.get(item.target);if(previous&&sha(previous.bytes)!==sha(item.bytes))throw Error('Конфликт файлов зависимостей');modTargets.set(item.target,item);}
    }
    for(const v of modVersions.values())for(const d of v.dependencies||[])if(d.dependency_type==='incompatible'&&[...modVersions.values()].some(x=>(!d.project_id||x.project_id===d.project_id)&&(!d.version_id||x.id===d.version_id)))throw Error('Конфликт обязательных модов: '+v.filename);
    pending.push(...modTargets.values());
    const registry=path.join(root,'nova-content.json'),temp=registry+'.'+crypto.randomUUID()+'.tmp',written=[];
    try{
      for(const item of pending){fs.mkdirSync(path.dirname(item.target),{recursive:true});fs.writeFileSync(item.target,item.bytes,{flag:'wx'});written.push(item.target);}
      let records=[];try{const data=JSON.parse(fs.readFileSync(registry,'utf8'));if(Array.isArray(data))records=data;}catch{}
      for(const item of planned.values()){records=records.filter(r=>!(r.project===item.project&&r.type===item.type));records.push({project:item.project,type:item.type,version:item.version.id,filename:item.file.filename});}
      fs.mkdirSync(root,{recursive:true});fs.writeFileSync(temp,JSON.stringify(records,null,2),{flag:'wx'});fs.renameSync(temp,registry);
    }catch(e){for(const file of written)fs.rmSync(file,{force:true});throw e;}finally{fs.rmSync(temp,{force:true});}
    return filename;
  }
  async function downloadPack(project,version,target){
    const p=await projectInfo(project);if(p.project_type!=='modpack')throw Error('Это не сборка Modrinth');
    const selected=await selectVersion(p.id,{version,type:'modpack'}),bytes=await downloadFile(selected.file);
    fs.mkdirSync(path.dirname(target),{recursive:true});fs.writeFileSync(target,bytes,{flag:'wx'});return selected.version;
  }
  return {search,installMod,installContent,listContent,downloadPack};
};
module.exports.searchURL=searchURL;
