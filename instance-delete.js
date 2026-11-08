const fs=require('fs'),path=require('path'),crypto=require('crypto');
async function removeInstance(id,{instancesDir,recoveryDir,load,save,trash}){
  if(typeof id!=='string'||!/^[\w-]+$/.test(id))throw Error('Некорректный инстанс');
  const db=load(),inst=db.instances.find(i=>i.id===id);if(!inst)throw Error('Инстанс не найден');
  const source=path.join(instancesDir,id);let staged=null;
  if(fs.existsSync(source)){
    fs.mkdirSync(recoveryDir,{recursive:true});staged=path.join(recoveryDir,id+'-'+crypto.randomUUID());fs.renameSync(source,staged);
  }
  try{save({...db,instances:db.instances.filter(i=>i.id!==id)});}
  catch(e){if(staged){try{fs.renameSync(staged,source);}catch{throw Error('Не удалось удалить инстанс. Его файлы сохранены: '+staged);}}throw e;}
  let retained=false;
  if(staged){try{await trash(staged);}catch{retained=true;}}
  return {id,name:inst.name,retained,...(retained?{recoveryPath:staged}:{})};
}
module.exports={removeInstance};
