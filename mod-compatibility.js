const fs=require('fs'),path=require('path'),crypto=require('crypto'),AdmZip=require('adm-zip');
const digest=b=>crypto.createHash('sha512').update(b).digest('hex');
// Unknown predicates stay unknown rather than being treated as compatible.
function matches(version,range){
  if(Array.isArray(range)){const values=range.map(r=>matches(version,r));return values.includes(true)?true:values.includes(null)?null:false;}
  if(typeof range!=='string')return null;
  if(range.includes('||'))return matches(version,range.split('||').map(x=>x.trim()));
  if(range==='*'||range==='')return true;
  const parse=v=>/^\d+(\.\d+){0,3}$/.test(v)?v.split('.').map(Number):null;
  const a=parse(version);if(!a)return null;
  const compare=b=>{for(let i=0;i<Math.max(a.length,b.length);i++){const d=(a[i]||0)-(b[i]||0);if(d)return Math.sign(d);}return 0;};
  const parts=range.trim().replace(/(>=|<=|>|<|=|~|\^)\s+/g,'$1').split(/\s+/);let unknown=false;
  for(const part of parts){const m=part.match(/^(>=|<=|>|<|=|~|\^)?(\d+(?:\.(?:\d+|[xX*]))*)$/);if(!m){unknown=true;continue;}
    const op=m[1]||'=',raw=m[2],wild=raw.search(/[xX*]/);
    if(wild>=0){if(op!=='='){unknown=true;continue;}const prefix=raw.split('.');if(!prefix.every((x,i)=>/[xX*]/.test(x)||(a[i]||0)===Number(x)))return false;continue;}
    const b=parse(raw),c=compare(b);let good;
    if(op==='~'){const upper=[...b];if(upper.length===1)upper[0]++;else upper[1]++;upper.length=Math.min(2,upper.length);good=c>=0&&compare(upper)<0;}
    else if(op==='^'){const upper=[...b];let i=upper.findIndex(x=>x!==0);if(i<0)i=upper.length-1;upper[i]++;upper.length=i+1;good=c>=0&&compare(upper)<0;}
    else good={'=':c===0,'>':c>0,'<':c<0,'>=':c>=0,'<=':c<=0}[op];
    if(!good)return false;
  }
  return unknown?null:true;
}
function inspectLocal(file,preferred){
  const zip=new AdmZip(file),mods=[];
  if(preferred==='Forge'&&zip.getEntry('META-INF/mods.toml'))return [{loader:'Forge'}];
  if(preferred==='NeoForge'&&zip.getEntry('META-INF/neoforge.mods.toml'))return [{loader:'NeoForge'}];
  function read(z,depth=0){
    const entry=z.getEntry('fabric.mod.json');
    if(entry){if(entry.header.size>1024*1024)throw Error('Слишком большие метаданные');const j=JSON.parse(entry.getData().toString('utf8'));mods.push({id:j.id,version:j.version,provides:j.provides||[],depends:j.depends||{},breaks:j.breaks||{},loader:'Fabric',server:j.environment==='server'});
      if(depth<1)for(const nested of (j.jars||[]).slice(0,64)){const e=z.getEntry(nested.file);if(e&&e.header.size<16*1024*1024)read(new AdmZip(e.getData()),depth+1);}
      return;
    }
    const quilt=z.getEntry('quilt.mod.json');if(quilt){const j=JSON.parse(quilt.getData().toString('utf8')).quilt_loader;mods.push({id:j.id,version:j.version,loader:'Quilt',depends:Object.fromEntries((j.depends||[]).filter(x=>x.id&&!x.optional).map(x=>[x.id,x.versions||'*'])),provides:(j.provides||[]).map(x=>typeof x==='string'?x:x.id)});return;}
    if(z.getEntry('META-INF/neoforge.mods.toml'))mods.push({loader:'NeoForge'});
    else if(z.getEntry('META-INF/mods.toml'))mods.push({loader:'Forge'});
  }
  read(zip);return mods;
}
async function check(root,inst,{request=global.fetch,headers={}}={}){
  const dir=path.join(root,'mods'),names=fs.existsSync(dir)?fs.readdirSync(dir).filter(n=>n.endsWith('.jar')):[],issues=[],items=[],ids=new Map();
  const add=(severity,file,message)=>{if(!issues.some(x=>x.file===file&&x.message===message))issues.push({severity,file,message});};
  for(const name of names){try{const bytes=fs.readFileSync(path.join(dir,name)),local=inspectLocal(path.join(dir,name),inst.loader);items.push({name,hash:digest(bytes),local});
    for(const mod of local){for(const id of [mod.id,...(mod.provides||[])].filter(Boolean)){if(ids.has(id)&&ids.get(id).file!==name)add('error',name,'Дубликат мода '+id+' в '+ids.get(id).file);else ids.set(id,{file:name,version:mod.version});}
      const accepted=mod.loader===inst.loader||(inst.loader==='Quilt'&&mod.loader==='Fabric');if(!accepted){if(mod.loader==='Forge'&&inst.loader==='NeoForge')add('warning',name,'Старый формат метаданных Forge/NeoForge — требуется проверка Modrinth');else add('error',name,'Мод для '+mod.loader+', выбран '+inst.loader);}
      if(mod.server)add('warning',name,'Мод только для сервера, а не для игрового клиента');
    }
  }catch(e){add('error',name,'Не удалось прочитать JAR: '+e.message);}}
  ids.set('minecraft',{version:inst.version});
  if(inst.loaderVersion)ids.set(({Fabric:'fabricloader',Quilt:'quilt_loader',Forge:'forge',NeoForge:'neoforge'})[inst.loader],{version:inst.loaderVersion});
  for(const item of items)for(const mod of item.local){
    for(const [id,range]of Object.entries(mod.depends||{})){
      if(id==='java')continue;
      if((id==='fabricloader'&&['Fabric','Quilt'].includes(inst.loader))||(id==='quilt_loader'&&inst.loader==='Quilt')){if(!ids.has(id))continue;}
      const dep=ids.get(id);if(!dep){add('error',item.name,'Не найдена обязательная зависимость: '+id);continue;}
      const ok=matches(String(dep.version),range);if(ok===false)add('error',item.name,'Не подходит '+id+' '+dep.version+'; требуется '+JSON.stringify(range));else if(ok===null)add('warning',item.name,'Не удалось полностью проверить диапазон '+id+': '+JSON.stringify(range));
    }
    for(const [id,range]of Object.entries(mod.breaks||{})){const dep=ids.get(id);if(dep&&matches(String(dep.version),range)===true)add('error',item.name,'Заявлен конфликт с '+id+' '+dep.version);}
  }
  let online=false,known={};
  if(items.length)try{const r=await request('https://api.modrinth.com/v2/version_files',{method:'POST',headers:{...headers,'Content-Type':'application/json'},body:JSON.stringify({hashes:items.map(x=>x.hash),algorithm:'sha512'}),signal:AbortSignal.timeout(20000)});if(!r.ok)throw Error();known=await r.json();online=true;}catch{online=false;}
  const versions=Object.values(known),projects=new Map();
  for(const item of items){const v=known[item.hash];if(!v){if(!item.local.length||item.local.every(x=>!x.id))add('warning',item.name,'Нет полных метаданных: совместимость не подтверждена');continue;}
    if(projects.has(v.project_id))add('error',item.name,'Повтор проекта Modrinth: '+projects.get(v.project_id));else projects.set(v.project_id,item.name);
    if(!v.game_versions.includes(inst.version))add('error',item.name,'На Modrinth эта версия мода не поддерживает Minecraft '+inst.version);
    if(!v.loaders.includes(inst.loader.toLowerCase())&&!(inst.loader==='Quilt'&&v.loaders.includes('fabric')))add('error',item.name,'На Modrinth указан другой загрузчик: '+v.loaders.join(', '));
    for(const d of v.dependencies||[]){if(!['required','incompatible'].includes(d.dependency_type))continue;
      const exists=!!(d.project_id||d.version_id)&&versions.some(x=>(!d.project_id||x.project_id===d.project_id)&&(!d.version_id||x.id===d.version_id));
      if(d.dependency_type==='required'&&!exists)add('warning',item.name,'Не подтверждена зависимость Modrinth '+(d.project_id||d.version_id||d.file_name||'без идентификатора')+'. Возможно, она установлена локально под другим хешем.');
      if(d.dependency_type==='incompatible'&&exists)add('error',item.name,'Заявлен конфликт с проектом Modrinth '+(d.project_id||d.version_id));
    }
  }
  return {checked:names.length,disabled:fs.existsSync(dir)?fs.readdirSync(dir).filter(x=>x.endsWith('.jar.disabled')).length:0,online,issues,errors:issues.filter(x=>x.severity==='error').length,warnings:issues.filter(x=>x.severity==='warning').length};
}
module.exports={check,matches};
