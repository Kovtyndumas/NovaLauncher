const fs=require('fs'),path=require('path'),crypto=require('crypto');
const clone=x=>JSON.parse(JSON.stringify(x));
const defaults=()=>({instances:[],servers:[],settings:{name:'Steve',ram:4,snow:false,theme:'black'}});
function normalize(value){
  if(!value||typeof value!=='object'||!Array.isArray(value.instances)||!value.settings||typeof value.settings!=='object')throw Error('Некорректный файл настроек');
  const d={...value,settings:{...defaults().settings,...value.settings},servers:Array.isArray(value.servers)?value.servers:[]};
  if(!d.instances.every(i=>i&&typeof i.id==='string'&&/^[\w-]+$/.test(i.id)&&typeof i.name==='string'&&typeof i.version==='string'&&/^[\w.+-]+$/.test(i.version)&&['Vanilla','Fabric','Quilt','Forge','NeoForge','Rust'].includes(i.loader)&&(i.loader!=='Rust'||i.version==='26.3')))throw Error('Некорректные данные инстанса');
  if(new Set(d.instances.map(x=>x.id)).size!==d.instances.length)throw Error('Повторяющиеся идентификаторы инстансов');
  d.settings.ram=Math.max(1,Math.min(64,Number(d.settings.ram)||4));d.settings.name=String(d.settings.name||'Steve');return d;
}
const equal=(a,b)=>JSON.stringify(a)===JSON.stringify(b);
function merge(current,incoming,base){
  normalize(incoming);if(!base){const result=normalize(incoming);result.settings.msRefresh=current.settings.msRefresh;return result;}
  const result=clone(current);
  for(const [key,value]of Object.entries(incoming.settings))if(key!=='msRefresh'&&!equal(base.settings?.[key],value))result.settings[key]=value;
  const old=new Map((base.instances||[]).map(i=>[i.id,i])),next=new Map(incoming.instances.map(i=>[i.id,i]));
  result.instances=result.instances.filter(i=>!old.has(i.id)||next.has(i.id));
  for(const [id,item]of next){const index=result.instances.findIndex(i=>i.id===id);if(index<0){if(!old.has(id))result.instances.push(clone(item));}else if(!old.has(id)||!equal(old.get(id),item)){
    const previous=result.instances[index];result.instances[index]={...previous,...clone(item)};
    if(previous.version===item.version&&previous.loader===item.loader&&previous.loaderVersion&&!item.loaderVersion)result.instances[index].loaderVersion=previous.loaderVersion;
  }}
  // Server edits are independent from backend-managed instances and accounts.
  if(!equal(base.servers,incoming.servers))result.servers=clone(incoming.servers||[]);
  return normalize(result);
}
function create(file){
 return {
  load(){if(!fs.existsSync(file))return defaults();try{return normalize(JSON.parse(fs.readFileSync(file,'utf8')));}catch(e){throw Error('Не удалось прочитать настройки. Сохранённый файл не изменён: '+e.message);}},
  save(data){const d=normalize(data);fs.mkdirSync(path.dirname(file),{recursive:true});const temp=file+'.'+crypto.randomBytes(4).toString('hex')+'.tmp';try{fs.writeFileSync(temp,JSON.stringify(d,null,2));fs.renameSync(temp,file);}finally{if(fs.existsSync(temp))fs.unlinkSync(temp);}},
 };
}
function publicDb(data){const result=clone(data);delete result.settings.msRefresh;return result;}
module.exports={create,merge,normalize,publicDb};
