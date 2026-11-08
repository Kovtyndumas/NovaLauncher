const fs=require('fs'),path=require('path'),crypto=require('crypto');
function remove(root,name){
 if(typeof name!=='string'||path.basename(name)!==name||/[\\/\x00]/.test(name)||!/^.+\.jar(?:\.disabled)?$/.test(name))throw Error('Некорректное имя мода');
 const source=path.join(root,'mods',name);if(!fs.statSync(source).isFile()||fs.lstatSync(source).isSymbolicLink())throw Error('Не найден обычный файл мода');
 const id=Date.now()+'-'+crypto.randomBytes(4).toString('hex'),dir=path.join(root,'backups','removed-mods',id);fs.mkdirSync(dir,{recursive:true});
 const target=path.join(dir,name);fs.renameSync(source,target);try{fs.writeFileSync(path.join(dir,'manifest.json'),JSON.stringify({name,at:new Date().toISOString()}));}catch(e){fs.renameSync(target,source);throw e;}
 return {name,backup:id};
}
function list(root){const dir=path.join(root,'backups','removed-mods');if(!fs.existsSync(dir))return [];return fs.readdirSync(dir).flatMap(id=>{try{const j=JSON.parse(fs.readFileSync(path.join(dir,id,'manifest.json'),'utf8'));return [{id,...j,restored:fs.existsSync(path.join(dir,id,'restored.json'))}];}catch{return [];}}).sort((a,b)=>b.at.localeCompare(a.at));}
function restore(root,id){
 if(typeof id!=='string'||!/^[\w-]+$/.test(id))throw Error('Некорректная копия');
 const dir=path.join(root,'backups','removed-mods',id),info=JSON.parse(fs.readFileSync(path.join(dir,'manifest.json'),'utf8')),name=info.name;
 if(typeof name!=='string'||path.basename(name)!==name||/[\\/\x00]/.test(name)||!/^.+\.jar(?:\.disabled)?$/.test(name))throw Error('Некорректная копия');
 const mods=path.join(root,'mods'),target=path.join(mods,name),alternate=target.endsWith('.disabled')?target.slice(0,-9):target+'.disabled';
 if(fs.existsSync(target)||fs.existsSync(alternate))throw Error('Мод уже существует. Сначала удалите или переименуйте текущий файл.');
 const source=path.join(dir,name);if(fs.lstatSync(source).isSymbolicLink())throw Error('Недопустимая ссылка');
 fs.mkdirSync(mods,{recursive:true});fs.renameSync(source,target);try{fs.writeFileSync(path.join(dir,'restored.json'),'{}');}catch(e){fs.renameSync(target,source);throw e;}return {name};
}
module.exports={remove,list,restore};
