const fs=require('fs'),path=require('path'),crypto=require('crypto'),AdmZip=require('adm-zip');
function loaderVersion(root,inst){
  if(inst.loaderVersion)return inst.loaderVersion;
  try{const d=JSON.parse(fs.readFileSync(path.join(root,'nova-pack.json'),'utf8')).dependencies;const k={Fabric:'fabric-loader',Quilt:'quilt-loader',Forge:'forge',NeoForge:'neoforge'}[inst.loader];if(d[k])return d[k];}catch{}
  const dir=path.join(root,'versions');if(fs.existsSync(dir))for(const id of fs.readdirSync(dir)){
    try{const j=JSON.parse(fs.readFileSync(path.join(dir,id,id+'.json'),'utf8'));if(j.inheritsFrom!==inst.version&&!id.includes(inst.version))continue;
      for(const lib of j.libraries||[]){const name=lib.name||'';const prefix={Fabric:'net.fabricmc:fabric-loader:',Quilt:'org.quiltmc:quilt-loader:',Forge:'net.minecraftforge:forge:',NeoForge:'net.neoforged:neoforge:'}[inst.loader];if(prefix&&name.startsWith(prefix)){const v=name.slice(prefix.length).split(':')[0];return inst.loader==='Forge'?v.replace(inst.version+'-',''):v;}}
    }catch{}
  }
  return null;
}
function exportPack(root,inst,destination,{progress=()=>{}}={}){
  const dependencies={minecraft:inst.version},key={Fabric:'fabric-loader',Quilt:'quilt-loader',Forge:'forge',NeoForge:'neoforge'}[inst.loader];
  if(inst.loader!=='Vanilla'&&!key)throw Error('Неизвестный загрузчик');
  if(key){const v=loaderVersion(root,inst);if(!v)throw Error('Не удалось определить точную версию загрузчика. Запустите этот инстанс один раз и повторите экспорт.');dependencies[key]=v;}
  const entries=[],omitted=[];
  const dirs=['mods','resourcepacks','shaderpacks','config','defaultconfigs','kubejs','scripts','datapacks'];
  function walk(dir,rel){for(const name of fs.readdirSync(dir)){
    if(/[\\:\x00]/.test(name)||name==='.'||name==='..')throw Error('Недопустимое имя файла: '+name);
    const source=path.join(dir,name),nameInPack=rel+'/'+name,stat=fs.lstatSync(source);if(stat.isSymbolicLink()){omitted.push(nameInPack);continue;}
    if(/(?:^|[._-])(?:accounts?|tokens?|secrets?|sessions?|credentials?)(?:[._-]|$)/i.test(name)){omitted.push(nameInPack);continue;}
    if(stat.isDirectory())walk(source,nameInPack);else if(stat.isFile())entries.push({source,name:nameInPack,size:stat.size});
  }}
  for(const dir of dirs){const source=path.join(root,dir);if(!fs.existsSync(source))continue;if(fs.lstatSync(source).isSymbolicLink()){omitted.push(dir);continue;}if(fs.statSync(source).isDirectory())walk(source,dir);}
  if(entries.length>20000||entries.reduce((n,x)=>n+x.size,0)>2*1024*1024*1024)throw Error('Сборка слишком большая для экспорта (лимит 2 ГБ)');
  const zip=new AdmZip();zip.addFile('modrinth.index.json',Buffer.from(JSON.stringify({formatVersion:1,game:'minecraft',versionId:'nova-'+Date.now(),name:inst.name,summary:'Exported from Nova Launcher',files:[],dependencies},null,2)));
  let done=0;for(const f of entries){zip.addFile('overrides/'+f.name,fs.readFileSync(f.source));progress({message:f.name,done:++done,total:entries.length});}
  const temp=destination+'.'+crypto.randomBytes(5).toString('hex')+'.tmp';
  try{zip.writeZip(temp);fs.renameSync(temp,destination);}finally{if(fs.existsSync(temp))fs.unlinkSync(temp);}
  return {file:destination,count:entries.length,omitted};
}
module.exports={exportPack,loaderVersion};
