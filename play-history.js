const fs=require('fs'),path=require('path'),crypto=require('crypto');
module.exports=file=>{
 function read(){try{const x=JSON.parse(fs.readFileSync(file,'utf8'));return Array.isArray(x)?x:[];}catch(e){if(e.code==='ENOENT')return [];throw e;}}
 function write(records){fs.mkdirSync(path.dirname(file),{recursive:true});const tmp=file+'.'+crypto.randomUUID()+'.tmp';try{fs.writeFileSync(tmp,JSON.stringify(records.slice(-300),null,2));fs.renameSync(tmp,file);}finally{fs.rmSync(tmp,{force:true});}}
 return {list:()=>read().slice().reverse(),start(instance,engine='java'){const r={id:crypto.randomUUID(),instanceId:instance.id,name:instance.name,version:instance.version,engine,at:new Date().toISOString(),seconds:0,closed:false};const a=read();a.push(r);write(a);return r.id;},finish(id,code){const a=read(),r=a.find(x=>x.id===id);if(!r||r.closed)return;r.seconds=Math.max(0,Math.round((Date.now()-Date.parse(r.at))/1000));r.closed=true;r.code=code;write(a);}};
};
