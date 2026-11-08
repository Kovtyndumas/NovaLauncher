const fs=require('fs'),path=require('path');
function patchLaunchCache(client){
 if(typeof client.createGameDirectory!=='function')return;
 const create=client.createGameDirectory.bind(client);
 client.createGameDirectory=function(...args){const result=create(...args);patchHandler(this.handler,this.options);return result;};
}
function patchHandler(handler,options){
 if(!handler||handler.novaCachePatched)return;handler.novaCachePatched=true;
 const file=path.join(options.root,'cache','nova-checksums.json');let records={};try{if(fs.statSync(file).size<24*1024*1024)records=JSON.parse(fs.readFileSync(file,'utf8'));}catch{}
 let dirty=false;
 const persist=()=>{if(!dirty)return;try{fs.mkdirSync(path.dirname(file),{recursive:true});const temp=file+'.tmp';fs.writeFileSync(temp,JSON.stringify(records));fs.renameSync(temp,file);dirty=false;}catch{}};
 if(typeof handler.checkSum==='function'){
  const checksum=handler.checkSum.bind(handler);
  handler.checkSum=async(sum,name)=>{
   let stat;try{stat=fs.statSync(name);if(!stat.isFile())return false;}catch{return false;}
   const fingerprint=[stat.size,stat.mtimeMs,stat.ctimeMs,stat.ino].join(':'),old=records[name];
   if(old?.sum===sum&&old.fingerprint===fingerprint)return true;
   const valid=await checksum(sum,name);
   if(valid){records[name]={sum,fingerprint};dirty=true;}else if(old){delete records[name];dirty=true;}
   return valid;
  };
 }
 if(typeof handler.getVersion==='function'){
  const version=handler.getVersion.bind(handler);
  handler.getVersion=async()=>{
   const id=options.version.number;
   for(const name of [options.overrides?.versionJson,path.join(options.directory||options.root,id+'.json'),path.join(options.cache||path.join(options.root,'cache'),'json',id+'.json')].filter(Boolean)){
    try{const j=JSON.parse(fs.readFileSync(name,'utf8'));if(j.id===id&&j.downloads?.client&&Array.isArray(j.libraries)){handler.version=j;handler.client?.emit('debug','[Nova]: Метаданные версии загружены из локального кэша');return j;}}catch{}
   }
   return version();
  };
 }
 if(typeof handler.getAssets==='function'){
  const assets=handler.getAssets.bind(handler);handler.getAssets=async(...args)=>{try{return await assets(...args);}finally{persist();}};
 }
}
module.exports={patchLaunchCache,patchHandler};
