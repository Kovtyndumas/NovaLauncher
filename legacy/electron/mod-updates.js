const fs=require('fs'),path=require('path'),crypto=require('crypto');
const hash=b=>crypto.createHash('sha512').update(b).digest('hex');
const safe=name=>typeof name==='string'&&path.basename(name)===name&&!/[\\/\x00]/.test(name)&&/\.jar$/.test(name);
module.exports=function({fetch:request=global.fetch,headers={}}={}){
  async function post(route,body){const r=await request('https://api.modrinth.com/v2/'+route,{method:'POST',headers:{...headers,'Content-Type':'application/json'},body:JSON.stringify(body),signal:AbortSignal.timeout(30000)});if(!r.ok)throw Error('Modrinth: HTTP '+r.status);return r.json();}
  async function scan(root,inst){
    const dir=path.join(root,'mods');
    const files=fs.existsSync(dir)?fs.readdirSync(dir).filter(n=>/\.jar(\.disabled)?$/.test(n)).map(name=>({name,hash:hash(fs.readFileSync(path.join(dir,name)))})):[];
    if(!files.length)return {updates:[],unknown:[],known:{}};
    const hashes=files.map(f=>f.hash),body={hashes,algorithm:'sha512'};
    const known=await post('version_files',body);
    const latest=await post('version_files/update',{...body,loaders:[inst.loader.toLowerCase()],game_versions:[inst.version],version_types:['release']});
    const updates=[];
    for(const f of files){const old=known[f.hash],v=latest[f.hash];if(!old||!v||old.id===v.id||v.project_id!==old.project_id)continue;
      if(!v.game_versions.includes(inst.version)||!v.loaders.includes(inst.loader.toLowerCase()))continue;
      const file=v.files.find(x=>x.primary)||v.files[0];if(!file||!safe(file.filename)||!file.hashes?.sha512||file.hashes.sha512===f.hash)continue;
      if(old.date_published&&v.date_published&&v.date_published<=old.date_published)continue;
      updates.push({...f,to:file.filename+(f.name.endsWith('.disabled')?'.disabled':''),version:v.version_number,fromVersion:old.version_number,project:v.project_id,id:v.id,file,dependencies:v.dependencies||[]});
    }
    return {updates,unknown:files.filter(f=>!known[f.hash]).map(f=>f.name),known,files};
  }
  async function apply(root,inst,names){
    const plan=await scan(root,inst),items=plan.updates.filter(x=>names.includes(x.name));
    if(items.length!==names.length)throw Error('Список модов изменился. Проверьте обновления ещё раз.');
    if(!items.length)return {count:0};
    const known=Object.values(plan.known),selected=new Map(items.map(x=>[x.project,x]));
    for(const item of items)for(const dep of item.dependencies){
      if(dep.dependency_type!=='required'&&dep.dependency_type!=='incompatible')continue;
      const identified=!!(dep.project_id||dep.version_id);
      const exists=identified?known.some(v=>(!dep.project_id||v.project_id===dep.project_id)&&(!dep.version_id||(selected.get(v.project_id)?.id||v.id)===dep.version_id)&&plan.files.some(f=>!f.name.endsWith('.disabled')&&plan.known[f.hash]?.id===v.id)):!!dep.file_name&&plan.files.some(f=>f.name===dep.file_name&&!f.name.endsWith('.disabled'));
      if((dep.dependency_type==='required'&&!exists)||(dep.dependency_type==='incompatible'&&exists))throw Error('Зависимости для '+item.name+' не подходят. Установите нужную зависимость через каталог и повторите проверку.');
    }
    const dir=path.join(root,'mods'),backup=path.join(root,'backups','mods',Date.now()+'-'+crypto.randomBytes(3).toString('hex'));
    const targets=new Set();
    for(const item of items){if(targets.has(item.to)||(fs.existsSync(path.join(dir,item.to))&&!items.some(x=>x.name===item.to)))throw Error('Конфликт имени: '+item.to);targets.add(item.to);}
    // Download and verify the entire batch before touching installed files.
    const buffers=[];
    for(const item of items){if(new URL(item.file.url).protocol!=='https:')throw Error('Недопустимая ссылка загрузки');const r=await request(item.file.url,{headers,signal:AbortSignal.timeout(120000)});if(!r.ok)throw Error('Не удалось скачать '+item.to);const b=Buffer.from(await r.arrayBuffer());if(hash(b)!==item.file.hashes.sha512)throw Error('Повреждён файл '+item.to);buffers.push(b);}
    fs.mkdirSync(backup,{recursive:true});
    for(const item of items){if(hash(fs.readFileSync(path.join(dir,item.name)))!==item.hash)throw Error('Мод изменился: '+item.name);fs.copyFileSync(path.join(dir,item.name),path.join(backup,item.name));}
    fs.writeFileSync(path.join(backup,'manifest.json'),JSON.stringify(items.map(({name,to,hash:beforeHash,file})=>({name,to,beforeHash,afterHash:file.hashes.sha512})),null,2));
    const written=[];
    try{
      for(const item of items)fs.unlinkSync(path.join(dir,item.name));
      for(let i=0;i<items.length;i++){fs.writeFileSync(path.join(dir,items[i].to),buffers[i],{flag:'wx'});written.push(items[i].to);}
      fs.writeFileSync(path.join(backup,'completed.json'),JSON.stringify({at:new Date().toISOString()}));
    }catch(e){for(const name of written)fs.unlinkSync(path.join(dir,name));for(const item of items)fs.copyFileSync(path.join(backup,item.name),path.join(dir,item.name));throw e;}
    return {count:items.length,backup};
  }
  return {scan,apply};
};
