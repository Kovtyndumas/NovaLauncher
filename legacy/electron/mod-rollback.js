const fs=require('fs'),path=require('path'),crypto=require('crypto');
const hash=b=>crypto.createHash('sha512').update(b).digest('hex');
const safe=n=>typeof n==='string'&&path.basename(n)===n&&!/[\\/\x00]/.test(n)&&/\.jar(\.disabled)?$/.test(n);
function read(root,id){
  if(typeof id!=='string'||!/^[\w-]+$/.test(id))throw Error('Некорректная резервная копия');
  const dir=path.join(root,'backups','mods',id);if(fs.lstatSync(dir).isSymbolicLink()||fs.lstatSync(path.join(dir,'manifest.json')).isSymbolicLink())throw Error('Недопустимая ссылка в копии');const raw=JSON.parse(fs.readFileSync(path.join(dir,'manifest.json'),'utf8'));
  const items=Array.isArray(raw)?raw:raw.items;
  if(!Array.isArray(items)||!items.length||items.some(x=>!safe(x.name)||!safe(x.to))||new Set(items.map(x=>x.name)).size!==items.length||new Set(items.map(x=>x.to)).size!==items.length)throw Error('Повреждён список резервной копии');
  return {dir,items};
}
function list(root){
  const base=path.join(root,'backups','mods');if(!fs.existsSync(base))return [];
  return fs.readdirSync(base,{withFileTypes:true}).filter(x=>x.isDirectory()).flatMap(x=>{try{const {dir,items}=read(root,x.name);if(items.some(x=>x.afterHash)&&!fs.existsSync(path.join(dir,'completed.json')))return [];return [{id:x.name,at:fs.statSync(path.join(dir,'manifest.json')).mtime.toISOString(),count:items.length,files:items.map(x=>x.name),legacy:items.some(x=>!x.afterHash),restored:fs.existsSync(path.join(dir,'restored.json'))}];}catch{return [];}}).sort((a,b)=>b.at.localeCompare(a.at));
}
function restore(root,id){
  const {dir,items}=read(root,id),mods=path.join(root,'mods');
  if(fs.existsSync(path.join(dir,'restored.json')))throw Error('Эта копия уже восстановлена');
  const originals=[],current=new Map();
  for(const item of items){
    const file=path.join(dir,item.name);if(fs.lstatSync(file).isSymbolicLink())throw Error('Недопустимая ссылка в копии');const b=fs.readFileSync(file);if(item.beforeHash&&hash(b)!==item.beforeHash)throw Error('Повреждена копия '+item.name);originals.push(b);
    const target=path.join(mods,item.to),disabled=target.endsWith('.disabled')?target.slice(0,-9):target+'.disabled';
    if(!fs.existsSync(target)||fs.existsSync(disabled))throw Error('Файл '+item.to+' изменён или переключён. Откат отменён, чтобы сохранить ваши изменения.');
    if(fs.lstatSync(target).isSymbolicLink())throw Error('Символьные ссылки нельзя откатывать');
    const now=fs.readFileSync(target);if(item.afterHash&&hash(now)!==item.afterHash)throw Error('Мод '+item.to+' изменён после обновления. Выберите более свежую копию.');
    current.set(item.to,now);
  }
  for(const item of items)if(fs.existsSync(path.join(mods,item.name))&&!current.has(item.name))throw Error('Уже существует '+item.name+'. Откат не будет его перезаписывать.');
  const recovery=path.join(root,'backups','rollback-snapshots',Date.now()+'-'+crypto.randomBytes(3).toString('hex'));fs.mkdirSync(recovery,{recursive:true});
  for(const [name,b]of current)fs.writeFileSync(path.join(recovery,name),b);
  fs.writeFileSync(path.join(recovery,'manifest.json'),JSON.stringify({source:id,files:[...current.keys()]}));
  const written=[];
  try{
    for(const [name]of current)fs.unlinkSync(path.join(mods,name));
    for(let i=0;i<items.length;i++){fs.writeFileSync(path.join(mods,items[i].name),originals[i],{flag:'wx'});written.push(items[i].name);}
    fs.writeFileSync(path.join(dir,'restored.json'),JSON.stringify({at:new Date().toISOString(),recovery}));
  }catch(e){for(const name of written)fs.unlinkSync(path.join(mods,name));for(const [name,b]of current)fs.writeFileSync(path.join(mods,name),b);throw e;}
  return {count:items.length,recovery};
}
module.exports={list,restore};
