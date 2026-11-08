const fs=require('fs'),path=require('path'),crypto=require('crypto');
const folders={resourcepack:'resourcepacks',shader:'shaderpacks'};
function file(root,type,name){if(!folders[type]||typeof name!=='string'||path.basename(name)!==name||/[\\/\x00\r\n]/.test(name)||!name.toLowerCase().endsWith('.zip'))throw Error('Некорректный файл контента');const folder=path.join(root,folders[type]);if(fs.lstatSync(folder).isSymbolicLink())throw Error('Папка контента не должна быть ссылкой');const f=path.join(folder,name);if(!fs.lstatSync(f).isFile())throw Error('Нужен обычный ZIP-файл');return f;}
function read(f){try{return fs.readFileSync(f,'utf8');}catch(e){if(e.code==='ENOENT')return '';throw e;}}
function options(text,key,separator,value){const lines=text.split(/\r?\n/),prefix=key+separator,index=lines.findIndex(l=>l.startsWith(prefix));if(index>=0)lines[index]=prefix+value;else lines.push(prefix+value);return lines.join('\n');}
const property=(text,key)=>{const line=text.split(/\r?\n/).find(l=>l.startsWith(key+'='));return line?.slice(key.length+1).replace(/\\u([0-9a-f]{4})/gi,(_,x)=>String.fromCharCode(parseInt(x,16))).replace(/\\(.)/g,'$1')||'';};
const encodeProperty=value=>String(value).replace(/\\/g,'\\\\').replace(/[^\x20-\x7e]/g,c=>'\\u'+c.charCodeAt(0).toString(16).padStart(4,'0')).replace(/^ /,'\\ ');
function active(root,type,name){if(type==='resourcepack'){try{const line=read(path.join(root,'options.txt')).split(/\r?\n/).find(l=>l.startsWith('resourcePacks:'));return JSON.parse(line?.slice(14)||'[]').some(x=>x==='file/'+name||x===name);}catch{return false;}}
 return ['config/iris.properties','optionsshaders.txt'].some(f=>{const text=read(path.join(root,f));return property(text,'shaderPack')===name&&(f!=='config/iris.properties'||property(text,'enableShaders')!=='false');});}
function configure(root,type,name,enabled){
 file(root,type,name);
 const writes=[];
 if(type==='resourcepack'){const f=path.join(root,'options.txt'),text=read(f),line=text.split(/\r?\n/).find(l=>l.startsWith('resourcePacks:'));let packs;try{packs=JSON.parse(line?.slice(14)||'[]');if(!Array.isArray(packs))throw Error();}catch{throw Error('Не удалось прочитать список ресурспаков в options.txt');}
 packs=packs.filter(x=>x!==name&&x!=='file/'+name);if(enabled)packs.push('file/'+name);writes.push([f,options(text,'resourcePacks',':',JSON.stringify(packs))]);
 }else{
  // Configure the supported shader engines; leave all unrelated properties intact.
  for(const [rel,iris]of [['config/iris.properties',true],['optionsshaders.txt',false]]){const f=path.join(root,rel);let text=read(f);if(!enabled&&property(text,'shaderPack')!==name)continue;text=options(text,'shaderPack','=',enabled?encodeProperty(name):'');if(iris)text=options(text,'enableShaders','=',String(enabled));writes.push([f,text]);}
 }
 const previous=writes.map(([f])=>[f,fs.existsSync(f)?fs.readFileSync(f):null]);
 try{for(const [f,text]of writes){fs.mkdirSync(path.dirname(f),{recursive:true});fs.writeFileSync(f,text);}}
 catch(e){for(const [f,b]of previous)if(b)fs.writeFileSync(f,b);else fs.rmSync(f,{force:true});throw e;}
}
function remove(root,type,name){const source=file(root,type,name),dir=path.join(root,'backups','content',Date.now()+'-'+crypto.randomBytes(4).toString('hex'));fs.mkdirSync(dir,{recursive:true});fs.copyFileSync(source,path.join(dir,name));configure(root,type,name,false);fs.unlinkSync(source);return {backup:dir};}
async function update(root,type,name,inst,service){
 const source=file(root,type,name),entry=service.listContent(root,type).find(f=>f.name===name);if(!entry?.project)throw Error('Файл не связан с Modrinth. Установите его из каталога');
 const tools=require('./instance-tools'),dir=path.join(root,'backups','content',Date.now()+'-'+crypto.randomBytes(4).toString('hex'));
 const targets=['mods','resourcepacks','shaderpacks','nova-content.json','options.txt','config/iris.properties','optionsshaders.txt'];const existed=new Set();
 for(const rel of targets)if(fs.existsSync(path.join(root,rel))){existed.add(rel);await tools.copyTree(path.join(root,rel),path.join(dir,rel));}
 const enabled=active(root,type,name);
 try{const next=await service.installContent(entry.project,inst,root,type);if(next!==name){if(enabled)configure(root,type,next,true);if(type==='resourcepack'&&enabled)configure(root,type,name,false);fs.unlinkSync(source);}return {filename:next,changed:next!==name};}
 catch(e){for(const rel of targets){const dest=path.join(root,rel);await fs.promises.rm(dest,{recursive:true,force:true});if(existed.has(rel))await tools.copyTree(path.join(dir,rel),dest);}throw e;}
}
module.exports={active,configure,remove,file,update};
