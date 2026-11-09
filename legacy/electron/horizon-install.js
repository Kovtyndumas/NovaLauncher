const fs=require('fs'),path=require('path'),crypto=require('crypto');
const LAUNCHER_TAG='launcher@0.2.54',GAME_TAG='v0.2.97',GAME_VERSION='26.3';
function platformAsset(platform=process.platform,arch=process.arch){
 if(platform==='darwin'&&arch==='arm64')return 'horizon-launcher-macos-arm64.zip';
 if(platform==='win32'&&['x64','arm64'].includes(arch))return 'horizon-launcher-windows-x64.zip';
 if(platform==='linux'&&arch==='x64')return 'horizon-launcher-linux-x86_64.tar.gz';
 throw Error('Эта сборка Horizon 26.3 доступна для macOS Apple Silicon, Windows x64 и Linux x64');
}
function findExecutable(dir,platform=process.platform){const name=platform==='win32'?'horizon-launcher.exe':'horizon-launcher';for(const e of fs.readdirSync(dir,{withFileTypes:true})){const p=path.join(dir,e.name);if(e.isSymbolicLink())continue;if(e.isFile()&&e.name===name)return p;if(e.isDirectory()){const found=findExecutable(p,platform);if(found)return found;}}return null;}
async function ensure(base,{fetch:request=global.fetch,log=()=>{},platform=process.platform,arch=process.arch}={}){
 const assetName=platformAsset(platform,arch),target=path.join(base,LAUNCHER_TAG.replace('@','-')+'-'+platform+'-'+arch);
 if(fs.existsSync(target)){const exe=findExecutable(target,platform);if(exe)return exe;throw Error('Установка Horizon повреждена: '+target);}
 log('Скачиваем официальный Horizon Launcher…');
 const headers={'User-Agent':'NovaLauncher/1.0','Accept':'application/vnd.github+json'},api='https://api.github.com/repos/horizon-rust/horizon/releases/tags/'+encodeURIComponent(LAUNCHER_TAG);
 const response=await request(api,{headers,signal:AbortSignal.timeout(30000)});if(!response.ok)throw Error('Horizon: GitHub HTTP '+response.status);const release=await response.json();const asset=release.assets?.find(a=>a.name===assetName);
 if(!asset||!/^sha256:[a-f0-9]{64}$/.test(asset.digest||'')||!Number.isSafeInteger(asset.size)||asset.size>200*1024*1024)throw Error('Нет проверенной сборки Horizon для этой платформы');
 if(!asset.browser_download_url.startsWith('https://github.com/horizon-rust/horizon/releases/download/'))throw Error('Некорректный источник Horizon');
 const download=await request(asset.browser_download_url,{signal:AbortSignal.timeout(180000)});if(!download.ok)throw Error('Скачивание Horizon: HTTP '+download.status);const bytes=Buffer.from(await download.arrayBuffer());if(bytes.length!==asset.size||'sha256:'+crypto.createHash('sha256').update(bytes).digest('hex')!==asset.digest)throw Error('Не совпала контрольная сумма Horizon');
 fs.mkdirSync(base,{recursive:true});const staging=target+'.'+crypto.randomUUID()+'.tmp',archive=staging+(assetName.endsWith('.zip')?'.zip':'.tar.gz');fs.mkdirSync(staging);
 try{
  fs.writeFileSync(archive,bytes,{flag:'wx'});
  if(assetName.endsWith('.zip')){
   const zip=new(require('adm-zip'))(bytes);for(const e of zip.getEntries()){const n=e.entryName.replace(/\\/g,'/');if(n.startsWith('/')||n.split('/').includes('..')||/^[A-Za-z]:/.test(n))throw Error('Некорректный путь в архиве Horizon');if(((e.header.attr>>>16)&0xf000)===0xa000)throw Error('Неподдерживаемая ссылка в архиве Horizon');}
   zip.extractAllTo(staging,false);
  }else{
   const {execFileSync}=require('child_process');const names=execFileSync('tar',['-tzf',archive],{encoding:'utf8',maxBuffer:8*1024*1024}).split('\n').filter(Boolean);if(names.some(n=>n.startsWith('/')||n.split('/').includes('..')))throw Error('Некорректный путь в архиве Horizon');execFileSync('tar',['-xzf',archive,'-C',staging],{timeout:120000});
  }
  const exe=findExecutable(staging,platform);if(!exe)throw Error('В архиве нет Horizon Launcher');if(platform!=='win32')fs.chmodSync(exe,0o755);fs.renameSync(staging,target);return path.join(target,path.relative(staging,exe));
 }finally{fs.rmSync(archive,{force:true});fs.rmSync(staging,{recursive:true,force:true});}
}
module.exports={ensure,platformAsset,findExecutable,LAUNCHER_TAG,GAME_TAG,GAME_VERSION};
