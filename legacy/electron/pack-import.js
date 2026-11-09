const fs=require('fs'),path=require('path'),crypto=require('crypto'),AdmZip=require('adm-zip');
const MAX_FILE=512*1024*1024,MAX_TOTAL=2*1024*1024*1024;
function inside(root,name){
  if(typeof name!=='string'||!name||/[\\:\x00]/.test(name)||name.startsWith('/')||name.split('/').some(x=>x==='..'||x==='.'||!x))throw Error('Небезопасный путь в сборке: '+name);
  const result=path.resolve(root,name);if(!result.startsWith(path.resolve(root)+path.sep))throw Error('Путь выходит за папку сборки');return result;
}
function allowed(url){const u=new URL(url);return u.protocol==='https:'&&!u.username&&!u.password&&['cdn.modrinth.com','github.com','raw.githubusercontent.com','gitlab.com','release-assets.githubusercontent.com','objects.githubusercontent.com'].includes(u.hostname);}
async function download(url,request,headers,limit){
  for(let redirects=0;redirects<=5;redirects++){
    if(!allowed(url))throw Error('Источник файла не поддерживается: '+url);
    const r=await request(url,{headers,redirect:'manual',signal:AbortSignal.timeout(120000)});
    if(r.status>=300&&r.status<400){url=new URL(r.headers.get('location'),url).href;continue;}
    if(!r.ok)throw Error('Ошибка загрузки: HTTP '+r.status);
    if(Number(r.headers?.get('content-length'))>limit)throw Error('Файл сборки слишком большой');
    if(!r.body){const b=Buffer.from(await r.arrayBuffer());if(b.length>limit)throw Error('Файл сборки слишком большой');return b;}
    let size=0;const chunks=[];for await(const chunk of r.body){size+=chunk.length;if(size>limit)throw Error('Файл сборки слишком большой');chunks.push(Buffer.from(chunk));}return Buffer.concat(chunks);
  }
  throw Error('Слишком много перенаправлений при загрузке');
}
async function importPack(file,{instancesDir,request=global.fetch,headers={},progress=()=>{}}){
  const stat=fs.statSync(file);if(!stat.isFile()||!file.toLowerCase().endsWith('.mrpack')||stat.size>MAX_FILE)throw Error('Выберите файл .mrpack размером до 512 МБ');
  let zip,idx;try{zip=new AdmZip(file);const entry=zip.getEntry('modrinth.index.json');if(!entry||entry.header.size>8*1024*1024)throw Error();idx=JSON.parse(entry.getData().toString('utf8'));}catch{throw Error('Некорректная сборка: не найден корректный modrinth.index.json');}
  if(idx.formatVersion!==1||idx.game!=='minecraft'||typeof idx.name!=='string'||!idx.dependencies?.minecraft||!Array.isArray(idx.files))throw Error('Неподдерживаемый формат сборки Modrinth');
  const deps=idx.dependencies,loaders={'fabric-loader':'Fabric','quilt-loader':'Quilt',forge:'Forge',neoforge:'NeoForge'};
  for(const key of Object.keys(deps))if(key!=='minecraft'&&!loaders[key])throw Error('Неподдерживаемая зависимость сборки: '+key);
  const found=Object.keys(loaders).filter(k=>deps[k]);if(found.length>1)throw Error('В сборке указано несколько загрузчиков');
  if(Object.values(deps).some(v=>typeof v!=='string'||!/^[\w.+-]+$/.test(v)))throw Error('Некорректная версия зависимости');
  fs.mkdirSync(instancesDir,{recursive:true});
  const stage=fs.mkdtempSync(path.join(instancesDir,'.import-')),id=crypto.randomUUID(),root=path.join(instancesDir,id);
  let published=false;
  try{
    const files=idx.files.filter(f=>f.env?.client!=='unsupported'),targets=new Set();let total=0;
    for(const f of files){inside(stage,f.path);if(targets.has(f.path.toLowerCase()))throw Error('Повтор файла в сборке: '+f.path);targets.add(f.path.toLowerCase());
      if(!Number.isSafeInteger(f.fileSize)||f.fileSize<0||f.fileSize>MAX_FILE||!f.downloads?.length||!f.downloads.every(allowed)||!/^([a-f0-9]{128})$/i.test(f.hashes?.sha512||''))throw Error('Некорректное описание файла: '+f.path);
      total+=f.fileSize;
    }
    const overrides=[];
    for(const prefix of ['overrides/','client-overrides/'])for(const entry of zip.getEntries()){
      if(!entry.entryName.startsWith(prefix)||entry.isDirectory)continue;
      const name=entry.entryName.slice(prefix.length);inside(stage,name);
      if(((entry.attr>>>16)&0xf000)===0xa000)throw Error('Символьные ссылки в сборке не поддерживаются');
      if(entry.header.size>MAX_FILE)throw Error('Файл в архиве слишком большой');total+=entry.header.size;overrides.push({entry,name});
    }
    if(total>MAX_TOTAL||files.length+overrides.length>20000)throw Error('Сборка слишком большая');
    const count=files.length+overrides.length;let done=0;
    progress({message:'Проверка сборки «'+idx.name+'»',done,total:count});
    for(const f of files){
      let data,error;
      for(const url of f.downloads){try{data=await download(url,request,headers,Math.max(f.fileSize,1));if(data.length!==f.fileSize||crypto.createHash('sha512').update(data).digest('hex').toLowerCase()!==f.hashes.sha512.toLowerCase())throw Error('Не совпала контрольная сумма '+f.path);error=null;break;}catch(e){error=e;}}
      if(error)throw error;
      const target=inside(stage,f.path);fs.mkdirSync(path.dirname(target),{recursive:true});fs.writeFileSync(target,data);
      progress({message:f.path,done:++done,total:count});
    }
    for(const {entry,name}of overrides){const target=inside(stage,name);fs.mkdirSync(path.dirname(target),{recursive:true});fs.writeFileSync(target,entry.getData());progress({message:name,done:++done,total:count});}
    const key=found[0],inst={id,name:idx.name.slice(0,100),version:deps.minecraft,loader:key?loaders[key]:'Vanilla',...(key?{loaderVersion:deps[key]}:{})};
    fs.writeFileSync(path.join(stage,'nova-pack.json'),JSON.stringify({name:inst.name,version:idx.versionId,dependencies:deps},null,2));
    fs.renameSync(stage,root);published=true;return inst;
  }finally{if(!published)fs.rmSync(stage,{recursive:true,force:true});}
}
module.exports={importPack,inside};
