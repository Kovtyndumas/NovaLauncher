// Horizon's documented launcher CLI owns preparation, authentication and game updates.
// https://playhorizon.dev/docs/api/launcher
const fs=require('fs'),path=require('path'),os=require('os');
function executable(input){
 let p=String(input||'').trim();if(process.platform==='darwin'&&p.endsWith('.app')){const dir=path.join(p,'Contents','MacOS');const names=fs.readdirSync(dir);p=path.join(dir,names.find(n=>/^horizon-launcher$/i.test(n))||names.find(n=>/horizon/i.test(n))||'horizon-launcher');}
 if(!path.isAbsolute(p)||!fs.statSync(p).isFile())throw Error('Выберите установленный Horizon Launcher');return p;
}
function detect(){const paths=process.platform==='darwin'?['/Applications/Horizon.app',path.join(os.homedir(),'Applications/Horizon.app')]:process.platform==='win32'?[path.join(process.env.LOCALAPPDATA||os.homedir(),'Programs','Horizon','horizon-launcher.exe')]:['/usr/bin/horizon-launcher',path.join(os.homedir(),'.local/bin/horizon-launcher')];return paths.find(p=>fs.existsSync(p))||'';}
function command(action,{tag,server}={}){if(tag&&!/^v[0-9]+\.[0-9]+\.[0-9]+$/.test(tag))throw Error('Некорректная версия Horizon');const args={versions:['versions','--installed'],install:['install','latest'],prepare:['prepare','--download'],play:['play'],open:[]}[action];if(!Object.hasOwn({versions:1,install:1,prepare:1,play:1,open:1},action))throw Error('Неизвестная команда Horizon');if(tag){if(action==='install')args[1]=tag;if(action==='prepare'||action==='play')args.push('--version',tag);}if(server&&action==='play')args.push('--server',String(server));return args;}
function launch(input,action,dataRoot,{spawn=require('child_process').spawn,onLog=()=>{},onStart=()=>{},tag,server,gameRoot}={}){
 const exe=executable(input),args=command(action,{tag,server});fs.mkdirSync(dataRoot,{recursive:true});
 const env={...process.env,HORIZON_LAUNCHER_HOME:path.join(dataRoot,'launcher'),HORIZON_HOME:gameRoot||path.join(dataRoot,'game')};
 return new Promise((resolve,reject)=>{const child=spawn(exe,args,{cwd:dataRoot,env,shell:false,stdio:['ignore','pipe','pipe']});let settled=false,output='';
 child.once('spawn',()=>onStart(child));for(const stream of [child.stdout,child.stderr])stream?.on('data',b=>{const line=String(b).replace(/(access_token|refresh_token)(["\s:=]+)[^\s,"]+/gi,'$1$2[скрыто]');output=(output+line).slice(-4000);onLog(line);});
 child.once('error',e=>{settled=true;reject(e);});child.once('close',code=>{if(settled)return;if(code!==0)reject(Object.assign(Error('Horizon завершился с кодом '+code+'. '+output.trim().slice(-1200)),{code}));else resolve({code:0,output});});
 });
}
module.exports={executable,detect,command,launch};
