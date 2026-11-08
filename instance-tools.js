const fs=require('fs/promises'),path=require('path'),crypto=require('crypto');
const safe=id=>{if(typeof id!=='string'||!/^[-\w]+$/.test(id))throw Error('Некорректный идентификатор');return id;};
async function copyTree(source,target){
 const stat=await fs.lstat(source);if(stat.isSymbolicLink())throw Error('Символические ссылки не копируются: '+path.basename(source));
 if(stat.isDirectory()){await fs.mkdir(target,{recursive:true});for(const name of await fs.readdir(source))await copyTree(path.join(source,name),path.join(target,name));}
 else if(stat.isFile()){await fs.mkdir(path.dirname(target),{recursive:true});await fs.copyFile(source,target,require('fs').constants.COPYFILE_EXCL);}
 else throw Error('Неподдерживаемый файл: '+path.basename(source));
}
async function exists(file){try{await fs.lstat(file);return true;}catch(e){if(e.code==='ENOENT')return false;throw e;}}
async function duplicate(inst,{instancesDir,load,save}){
 safe(inst.id);const id=crypto.randomUUID(),root=path.join(instancesDir,id),source=path.join(instancesDir,inst.id);
 try{
  await fs.mkdir(root,{recursive:true});if(await exists(source))for(const name of await fs.readdir(source))if(!['backups','logs','crash-reports','nova-last-crash.json'].includes(name))await copyTree(path.join(source,name),path.join(root,name));
  const item={...inst,id,name:inst.name+' — копия'};const db=load();db.instances.push(item);save(db);return item;
 }catch(e){await fs.rm(root,{recursive:true,force:true});throw e;}
}
const backupDir=root=>path.join(root,'backups','worlds');
async function snapshot(root,reason='Вручную'){
 const saves=path.join(root,'saves');if(!await exists(saves))return null;
 const names=await fs.readdir(saves);if(!names.length)return null;
 const id=Date.now()+'-'+crypto.randomBytes(4).toString('hex'),dir=path.join(backupDir(root),id),staging=dir+'.tmp';
 try{await copyTree(saves,path.join(staging,'saves'));const record={id,at:new Date().toISOString(),reason,worlds:names};await fs.writeFile(path.join(staging,'manifest.json'),JSON.stringify(record));await fs.rename(staging,dir);return record;}
 catch(e){await fs.rm(staging,{recursive:true,force:true});throw e;}
}
async function list(root){let names;try{names=await fs.readdir(backupDir(root));}catch(e){if(e.code==='ENOENT')return [];throw e;}const records=[];for(const id of names)if(/^\d+-[a-f0-9]+$/.test(id))try{const r=JSON.parse(await fs.readFile(path.join(backupDir(root),id,'manifest.json'),'utf8'));records.push({...r,id});}catch{}return records.sort((a,b)=>b.at.localeCompare(a.at));}
async function restore(root,id){
 safe(id);if(!(await list(root)).some(x=>x.id===id))throw Error('Копия не найдена');
 const source=path.join(backupDir(root),id,'saves'),staging=path.join(root,'saves.restore-'+crypto.randomUUID()),saves=path.join(root,'saves'),previous=staging+'.previous';
 await snapshot(root,'Перед восстановлением');
 try{await copyTree(source,staging);const had=await exists(saves);if(had)await fs.rename(saves,previous);try{await fs.rename(staging,saves);}catch(e){if(had)await fs.rename(previous,saves);throw e;}await fs.rm(previous,{recursive:true,force:true});}
 finally{await fs.rm(staging,{recursive:true,force:true});}
 return {id};
}
module.exports={duplicate,snapshot,list,restore,copyTree};
