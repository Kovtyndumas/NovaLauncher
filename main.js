const { app, BrowserWindow, ipcMain, shell, dialog } = require('electron');
const path = require('path'), fs = require('fs');
const { Client, Authenticator } = require('minecraft-launcher-core');
const { Auth } = require('msmc');
const AdmZip = require('adm-zip');

const UA = { 'User-Agent': 'NovaLauncher/1.0 (desktop Minecraft launcher)' };
const ud = () => app.getPath('userData');
const dbFile = () => path.join(ud(), 'db.json');
const iDir = id => path.join(ud(), 'instances', id);
const stateStore=require('./state-store');
const instanceTools=require('./instance-tools'),contentManager=require('./content-manager'),horizon=require('./horizon');
const history=()=>require('./play-history')(path.join(ud(),'play-history.json'));
const launcherUpdates=require('./launcher-update')({app,getFeed:()=>load().settings.updateFeed||'https://github.com/Kovtyndumas/NovaLauncher/releases/latest/download',notify:state=>{if(win&&!win.isDestroyed())win.webContents.send('launcher:update',state);}});
const {removeInstance}=require('./instance-delete');
const {Presence}=require('./discord-presence');
const presence=new Presence();let presenceContext={phase:'launcher'};
function refreshPresence(){const s=load().settings;presence.update(presenceContext);presence.configure(s.discordPresence!==false,s.discordAppId||'1568956638310699119');}
const {splitJvmArgs,quickPlay,neoPrefix,compareVersions}=require('./launch-options');
const load=()=>stateStore.create(dbFile()).load();
const save=d=>stateStore.create(dbFile()).save(d);
let win, msToken = null, activeGame = false, modBusy = false;
let smokeRoot;
if(process.env.NOVA_SMOKE_TEST==='1'){
 smokeRoot=fs.mkdtempSync(path.join(require('os').tmpdir(),'nova-packaged-smoke-'));app.setPath('userData',smokeRoot);
 fs.writeFileSync(path.join(smokeRoot,'db.json'),JSON.stringify({instances:[{id:'smoke',name:'Packaged startup test',version:'26.3',loader:'Vanilla'}],servers:[],settings:{name:'Test',ram:4,onboarded:true,discordPresence:false,autoUpdate:false}}));
}
async function packagedSmoke(){
 if(!smokeRoot)return;
 try{for(let n=0;n<150;n++){const state=await win.webContents.executeJavaScript("typeof S!=='undefined'&&!!S.db&&!!document.querySelector('[data-act=play]')").catch(()=>false);if(state){console.log('NOVA_PACKAGED_SMOKE_OK '+JSON.stringify({version:app.getVersion(),arch:process.arch}));fs.rmSync(smokeRoot,{recursive:true,force:true});app.exit(0);return;}await new Promise(r=>setTimeout(r,100));}throw Error('Packaged renderer did not become ready');}
 catch(e){console.error(e);fs.rmSync(smokeRoot,{recursive:true,force:true});app.exit(1);}
}

const analyzeCrash = require('./crash-analysis');
const {patchLaunchCache}=require('./launch-cache');
const modFiles=require('./mod-files');
const { patchClient } = require('./launch-args');
const compatibility = require('./mod-compatibility');
const compatibilityCache=require('./compatibility-cache');
const rollback = require('./mod-rollback');
const { exportPack, loaderVersion } = require('./pack-export');
const { importPack } = require('./pack-import');
const contentService = require('./content-service')({headers:UA});
const modUpdates = require('./mod-updates')({headers:UA});
const instanceById = id => {const inst=load().instances.find(x=>x.id===id);if(!inst)throw Error('Инстанс не найден');return inst;};

app.whenReady().then(() => {
  win = new BrowserWindow({
    show:!smokeRoot,width: 1100, height: 700, minWidth: 900, minHeight: 600,
    icon:path.join(__dirname,'assets','nova-icon.png'),
    titleBarStyle: 'hidden', trafficLightPosition: { x: 18, y: 16 },
    titleBarOverlay: { color: '#00000000', symbolColor: '#8e8e93', height: 44 },
    vibrancy: 'sidebar', webPreferences: { preload: path.join(__dirname, 'preload.js') },
  });
  win.webContents.once?.('did-finish-load',packagedSmoke);
  win.loadFile(path.join(__dirname,'index.html'));
  refreshPresence();
  if(app.isPackaged){setTimeout(()=>{if(load().settings.autoUpdate!==false)launcherUpdates.check();},7000).unref();setInterval(()=>{if(load().settings.autoUpdate!==false)launcherUpdates.check();},4*60*60*1000).unref();}
});
app.on('before-quit',()=>presence.stop());
app.on('window-all-closed', () => app.quit());

ipcMain.handle('db:get',()=>stateStore.publicDb(load()));
ipcMain.handle('versions:list',async()=>{
 const file=path.join(ud(),'versions-cache.json');
 try{const response=await fetch('https://piston-meta.mojang.com/mc/game/version_manifest_v2.json',{signal:AbortSignal.timeout(15000)});if(!response.ok)throw Error('Каталог версий: HTTP '+response.status);const versions=require('./game-versions').releases(await response.json());if(!versions.includes('1.0'))throw Error('Неполный каталог Minecraft');fs.mkdirSync(ud(),{recursive:true});fs.writeFileSync(file,JSON.stringify(versions));return {versions,cached:false};}
 catch(e){try{const versions=JSON.parse(fs.readFileSync(file,'utf8'));if(Array.isArray(versions)&&versions.includes('1.0'))return {versions,cached:true};}catch{}throw e;}
});
ipcMain.handle('history:list',()=>history().list());
const rustInstall=require('./horizon-install');
const rustData=id=>path.join(ud(),'horizon-instances',id);
async function rustTask(inst,action,{log=()=>{},onStart=()=>{},server}={}){
 if(inst.loader!=='Rust'||inst.version!==rustInstall.GAME_VERSION)throw Error('Выберите инстанс 26.3 RUST');
 const exe=await rustInstall.ensure(path.join(ud(),'runtimes','horizon-launcher'),{log});
 const options={tag:rustInstall.GAME_TAG,gameRoot:iDir(inst.id),onLog:log};
 if(action==='open')return horizon.launch(exe,'open',rustData(inst.id),options);
 if(action==='install'||action==='play'){
  log('Устанавливаем Minecraft 26.3 RUST · Horizon '+rustInstall.GAME_TAG+'…');
  const installed=await horizon.launch(exe,'versions',rustData(inst.id),{...options,onLog:()=>{}});
  if(!installed.output?.split(/\r?\n/).some(line=>line.trim().replace(/^\*\s*/,'').split(/\s+/)[0]===rustInstall.GAME_TAG))await horizon.launch(exe,'install',rustData(inst.id),options);
  else log('Сборка '+rustInstall.GAME_TAG+' уже установлена');
  if(action==='install')return {installed:true,tag:rustInstall.GAME_TAG};
 }
 log('Подготовка ресурсов Minecraft 26.3…');
 await horizon.launch(exe,'prepare',rustData(inst.id),options);
 log('Запускаем Minecraft 26.3 RUST…');
 return horizon.launch(exe,'play',rustData(inst.id),{...options,server,onStart});
}
ipcMain.handle('rust:setup',(_,id,action)=>idleOperation(async()=>{
 if(!['open','install'].includes(action))throw Error('Неизвестное действие');const inst=instanceById(id);
 return rustTask(inst,action,{log:line=>{if(win&&!win.isDestroyed())win.webContents.send('game:log',line);}});
}));
async function launchRust(inst,server){
 activeGame=true;presenceContext={phase:'launching',instance:inst};refreshPresence();
 const send=(channel,...args)=>{if(win&&!win.isDestroyed())win.webContents.send(channel,...args);},log=line=>send('game:log',line);let session,exitCode=-1,report=null;
 try{await rustTask(inst,'play',{server,log,onStart:()=>{try{session=history().start(inst,'horizon');}catch(e){log(e.message);}presenceContext={phase:'playing',instance:inst,started:Math.floor(Date.now()/1000)};refreshPresence();send('game:started',inst.id);}});exitCode=0;return true;}
 catch(e){log(e.message);const auth=/sign.?in|account|login|войд|аккаунт/i.test(e.message);report={instanceId:inst.id,code:e.code??-1,title:auth?'Войдите в Horizon':'Не удалось запустить 26.3 RUST',advice:auth?'Нажмите «Войти в Horizon» на экране игры, войдите в Microsoft с Java Edition и закройте окно Horizon. Затем нажмите «Играть».':e.message,evidence:[],at:new Date().toISOString()};return false;}
 finally{if(session)try{history().finish(session,exitCode);}catch(e){log(e.message);}activeGame=false;presenceContext={phase:'launcher',instance:inst};refreshPresence();send('game:closed',exitCode,report);}
}

async function idleOperation(fn){if(activeGame||modBusy)throw Error('Закройте игру и дождитесь завершения операций');modBusy=true;try{return await fn();}finally{modBusy=false;}}
ipcMain.handle('instance:duplicate',(_,id)=>idleOperation(async()=>{const item=await instanceTools.duplicate(instanceById(id),{instancesDir:path.join(ud(),'instances'),load,save});return {instance:item,db:stateStore.publicDb(load())};}));
ipcMain.handle('worlds:list',(_,id)=>{instanceById(id);return instanceTools.list(iDir(id));});
ipcMain.handle('worlds:backup',(_,id)=>idleOperation(()=>{instanceById(id);return instanceTools.snapshot(iDir(id));}));
ipcMain.handle('worlds:restore',(_,id,backup)=>idleOperation(()=>{instanceById(id);return instanceTools.restore(iDir(id),backup);}));
ipcMain.handle('content:manage',(_,id,type,name,action)=>idleOperation(async()=>{const inst=instanceById(id),root=iDir(id);if(inst.loader==='Rust')throw Error('Управляйте контентом Horizon в Horizon');if(action==='remove')return contentManager.remove(root,type,name);if(action==='toggle')return contentManager.configure(root,type,name,!contentManager.active(root,type,name));if(action==='update'){await instanceTools.snapshot(root,'Перед обновлением контента');return contentManager.update(root,type,name,inst,contentService);}throw Error('Неизвестное действие');}));
ipcMain.handle('launcher:status',()=>launcherUpdates.status());
ipcMain.handle('launcher:check',()=>launcherUpdates.check());
ipcMain.handle('launcher:download',()=>idleOperation(()=>launcherUpdates.download()));
ipcMain.handle('launcher:install',()=>{if(activeGame||modBusy)throw Error('Закройте игру и дождитесь завершения операций');launcherUpdates.install();});
ipcMain.handle('horizon:status',()=>({path:load().settings.horizonPath||horizon.detect()}));
ipcMain.handle('horizon:choose',async()=>{const selected=await dialog.showOpenDialog(win,{title:'Выберите Horizon Launcher',properties:['openFile'],...(process.platform==='darwin'?{filters:[{name:'Horizon',extensions:['app']}]}:{})});if(selected.canceled)return null;horizon.executable(selected.filePaths[0]);const db=load();db.settings.horizonPath=selected.filePaths[0];save(db);return {path:db.settings.horizonPath,db:stateStore.publicDb(db)};});
ipcMain.handle('horizon:download',()=>shell.openExternal('https://playhorizon.dev/docs/install'));
ipcMain.handle('horizon:run',(_,action)=>idleOperation(async()=>{
 const file=load().settings.horizonPath||horizon.detect();if(!file)throw Error('Установите Horizon и выберите приложение');
 const inst={id:'horizon',name:'Horizon',version:'26.3',loader:'Rust'};let session,exitCode=-1;
 if(action==='play'){activeGame=true;presenceContext={phase:'launching',instance:inst};refreshPresence();}
 try{const result=await horizon.launch(file,action,path.join(ud(),'horizon'),{onLog:line=>{if(win&&!win.isDestroyed())win.webContents.send('horizon:log',line);},onStart:()=>{if(action==='play'){try{session=history().start(inst,'horizon');}catch(e){if(win&&!win.isDestroyed())win.webContents.send('horizon:log','История: '+e.message);}presenceContext={phase:'playing',instance:inst,started:Math.floor(Date.now()/1000)};refreshPresence();}}});exitCode=0;return result;}
 finally{if(session)try{history().finish(session,exitCode);}catch(e){if(win&&!win.isDestroyed())win.webContents.send('horizon:log','История: '+e.message);}if(action==='play'){activeGame=false;presenceContext={phase:'launcher'};refreshPresence();}}
}));

ipcMain.handle('db:set',(_,d,base)=>{save(stateStore.merge(load(),d,base));refreshPresence();});
ipcMain.handle('discord:status',()=>presence.state);
ipcMain.handle('discord:select',(_,id)=>{if(!activeGame){const inst=id?instanceById(id):undefined;presenceContext={phase:'launcher',instance:inst};refreshPresence();}});
ipcMain.handle('instance:delete',async(_,id)=>{
  if(activeGame||modBusy)throw Error('Закройте игру и дождитесь завершения установки');
  const inst=instanceById(id);modBusy=true;
  try{
    const answer=await dialog.showMessageBox(win,{type:'warning',title:'Удаление инстанса',message:'Удалить инстанс «'+inst.name+'»?',detail:'Папка с мирами, модами и настройками будет перемещена в Корзину. При необходимости файлы можно восстановить из Корзины.',buttons:['Отмена','Удалить инстанс'],defaultId:0,cancelId:0,noLink:true});
    if(answer.response!==1)return null;
    const result=await removeInstance(id,{instancesDir:path.join(ud(),'instances'),recoveryDir:path.join(ud(),'deleted-instances'),load,save,trash:file=>shell.trashItem(file)});
    if(result.retained)await dialog.showMessageBox(win,{type:'info',title:'Инстанс удалён',message:'Не удалось переместить файлы в Корзину. Они сохранены в резервной папке.',detail:result.recoveryPath,buttons:['OK']});
    return {...result,db:stateStore.publicDb(load())};
  }finally{modBusy=false;}
});
ipcMain.handle('dir:open', (_, id) => {instanceById(id);fs.mkdirSync(iDir(id), { recursive: true }); shell.openPath(iDir(id)); });

// ---------- Modrinth ----------
ipcMain.handle('mr:search',(_,options)=>contentService.search(options));
const dl = async (url, file) => { fs.mkdirSync(path.dirname(file), { recursive: true }); const r = await fetch(url, {headers:UA,signal:AbortSignal.timeout(180000)}); if (!r.ok) throw new Error('Не скачалось: ' + url); fs.writeFileSync(file, Buffer.from(await r.arrayBuffer())); };
ipcMain.handle('mr:install', async (_, { project, inst, type }) => {
  if(activeGame||modBusy)throw Error('Закройте игру и дождитесь завершения установки');
  modBusy=true;
  try{const saved=instanceById(inst.id);if(saved.loader==='Rust')throw Error('Java-контент не устанавливается в Horizon. Используйте форматы Horizon');return type==='mod'?await contentService.installMod(project,saved,iDir(saved.id)):await contentService.installContent(project,saved,iDir(saved.id),type);}finally{modBusy=false;}
});
ipcMain.handle('content:list',(_,id,type)=>{instanceById(id);return contentService.listContent(iDir(id),type).map(f=>({...f,active:contentManager.active(iDir(id),type,f.name)}));});
ipcMain.handle('content:open',async(_,id,type)=>{instanceById(id);const folder={shader:'shaderpacks',resourcepack:'resourcepacks'}[type];if(!folder)throw Error('Неизвестный тип контента');const dir=path.join(iDir(id),folder);fs.mkdirSync(dir,{recursive:true});const error=await shell.openPath(dir);if(error)throw Error(error);});
async function installPackFile(file){
  const inst=await importPack(file,{instancesDir:path.join(ud(),'instances'),headers:UA,progress:p=>win.webContents.send('pack:progress',p)});
  try{const d=load();d.instances.push(inst);save(d);return inst;}catch(e){fs.rmSync(iDir(inst.id),{recursive:true,force:true});throw e;}
}
ipcMain.handle('pack:import',async(_,file)=>{
  if(activeGame||modBusy)throw Error('Закройте игру и дождитесь завершения установки');
  modBusy=true;
  try{
    if(file==null){const result=await dialog.showOpenDialog(win,{title:'Импорт сборки Modrinth',filters:[{name:'Сборка Modrinth',extensions:['mrpack']}],properties:['openFile']});if(result.canceled)return null;file=result.filePaths[0];}
    if(typeof file!=='string')throw Error('Не выбран файл сборки');
    return await installPackFile(file);
  }finally{modBusy=false;}
});
ipcMain.handle('mr:pack', async (_, { project, version }) => {
  if(activeGame||modBusy)throw Error('Закройте игру и дождитесь завершения установки');
  modBusy=true;let tmp;
  try{
    tmp=path.join(ud(),'tmp',require('crypto').randomUUID()+'.mrpack');
    await contentService.downloadPack(project,version,tmp);
    return await installPackFile(tmp);
  }finally{if(tmp)fs.rmSync(tmp,{force:true});modBusy=false;}
});
ipcMain.handle('srv:ping', async (_, host) => {
  try { const j = await (await fetch(`https://api.mcsrvstat.us/3/${encodeURIComponent(host)}`, {headers:UA,signal:AbortSignal.timeout(10000)})).json();return {online: !!j.online, players: j.players, motd: j.motd && j.motd.clean && j.motd.clean[0], version: j.version }; } catch { return { online: false }; }
});
ipcMain.handle('mods:list', (_, id) => { try { return fs.readdirSync(path.join(iDir(id), 'mods')).filter(f => /\.jar(\.disabled)?$/.test(f)); } catch { return []; } });
ipcMain.handle('mods:toggle', (_, id, f) => {
  if(activeGame||modBusy)throw Error('Закройте игру перед изменением модов');
  instanceById(id);
  if(typeof f!=='string'||path.basename(f)!==f||/[\\/]/.test(f)||!/^.+\.jar(?:\.disabled)?$/.test(f))throw Error('Некорректное имя файла');
  const d = path.join(iDir(id), 'mods');
  const target=path.join(d,f.endsWith('.disabled')?f.slice(0,-9):f+'.disabled');if(fs.existsSync(target))throw Error('Файл с таким именем уже существует');
  fs.renameSync(path.join(d,f),target);
});

// Updates use the saved instance, never version/loader data supplied by the renderer.
ipcMain.handle('mods:remove',(_,id,name)=>{if(activeGame||modBusy)throw Error('Закройте игру и дождитесь завершения операций');instanceById(id);return modFiles.remove(iDir(id),name);});
ipcMain.handle('mods:removed',(_,id)=>{instanceById(id);return modFiles.list(iDir(id));});
ipcMain.handle('mods:restoreRemoved',(_,id,backup)=>{if(activeGame||modBusy)throw Error('Закройте игру и дождитесь завершения операций');instanceById(id);return modFiles.restore(iDir(id),backup);});
ipcMain.handle('mods:check', async (_, id) => {
  const inst=instanceById(id);if(inst.loader==='Rust')throw Error('Обновления Java-модов недоступны для Horizon');const {updates,unknown}=await modUpdates.scan(iDir(id),inst);
  return {updates:updates.map(({name,to,version,fromVersion})=>({name,to,version,fromVersion})),unknown};
});
ipcMain.handle('mods:update', async (_, id, names) => {
  if(activeGame||modBusy)throw Error('Закройте игру и дождитесь завершения операций с модами');
  if(!Array.isArray(names)||new Set(names).size!==names.length)throw Error('Некорректный список модов');
  const inst=instanceById(id);if(inst.loader==='Rust')throw Error('Обновления Java-модов недоступны для Horizon');modBusy=true;
  try{if(names.length)await instanceTools.snapshot(iDir(id),'Перед обновлением модов');return await modUpdates.apply(iDir(id),inst,names);}finally{modBusy=false;}
});
ipcMain.handle('mods:compatibility',async(_,id,online=true)=>{const inst=instanceById(id);if(inst.loader==='Rust')return {checked:0,disabled:0,errors:0,warnings:0,issues:[]};const checker=online?compatibility:compatibilityCache;return checker.check(iDir(id),inst,{headers:UA,...(!online?{request:async()=>{throw Error('offline preflight');}}:{})});});
ipcMain.handle('mods:backups',(_,id)=>{instanceById(id);return rollback.list(iDir(id));});
ipcMain.handle('mods:rollback',async(_,id,backup)=>{
  if(activeGame||modBusy)throw Error('Закройте игру и дождитесь завершения операций с модами');
  instanceById(id);modBusy=true;try{return rollback.restore(iDir(id),backup);}finally{modBusy=false;}
});
ipcMain.handle('pack:export',async(_,id)=>{
  if(activeGame||modBusy)throw Error('Закройте игру и дождитесь завершения операций с модами');
  const inst=instanceById(id);if(inst.loader==='Rust')throw Error('Экспорт .mrpack доступен для Java-инстансов');modBusy=true;
  try{
    const result=await dialog.showSaveDialog(win,{title:'Экспорт сборки',defaultPath:inst.name.replace(/[\\/:*?"<>|]/g,'_')+'.mrpack',filters:[{name:'Сборка Modrinth',extensions:['mrpack']}]});
    if(result.canceled||!result.filePath)return null;
    const file=result.filePath.toLowerCase().endsWith('.mrpack')?result.filePath:result.filePath+'.mrpack';
    // Keep exports outside the instance so they can never overwrite its mods/configs.
    const relative=path.relative(iDir(id),file);if(!relative.startsWith('..'+path.sep)&&relative!=='..'&&!path.isAbsolute(relative))throw Error('Сохраните сборку за пределами папки инстанса');
    return exportPack(iDir(id),inst,file);
  }finally{modBusy=false;}
});
ipcMain.handle('game:lastCrash', (_, id) => {instanceById(id);try{return JSON.parse(fs.readFileSync(path.join(iDir(id),'nova-last-crash.json'),'utf8'));}catch{return null;}});

// ---------- Auth ----------
const authMgr = () => new Auth('select_account');
async function fromX(x) {
  const mc = await x.getMinecraft(); msToken = mc.mclc();
  const d = load(); d.settings.msRefresh = x.save(); d.settings.name = mc.profile.name; d.settings.ms = true; save(d);
  return mc.profile.name;
}
ipcMain.handle('auth:ms', async () => fromX(await authMgr().launch('electron')));
ipcMain.handle('auth:restore', async () => { const r = load().settings.msRefresh; if (!r) return null; try { return await fromX(await authMgr().refresh(r)); } catch { return null; } });
ipcMain.handle('auth:logout', () => { msToken = null; const d = load(); delete d.settings.msRefresh; d.settings.ms = false; save(d); });

// ---------- Launch ----------
const javaMajor = require('./java-version');
function findJava(dir) {
  const exe = process.platform === 'win32' ? 'java.exe' : 'java', skip = ['lib', 'legal', 'man', 'include', 'jmods', 'conf'];
  const walk = d => { for (const e of fs.readdirSync(d, { withFileTypes: true })) { const p = path.join(d, e.name); if (e.isDirectory()) { if (skip.includes(e.name)) continue; const r = walk(p); if (r) return r; } else if (e.name === exe && path.basename(d) === 'bin') return p; } };
  try { return walk(dir); } catch {}
}
async function ensureJava(major, log) {
  const legacyMac=process.platform==='darwin'&&process.arch==='arm64'&&major<=16;
  const os = { darwin: 'mac', win32: 'windows', linux: 'linux' }[process.platform], arch = legacyMac?'x64':process.arch === 'arm64' ? 'aarch64' : 'x64';
  const dir=path.join(ud(),'runtimes',String(major)+(legacyMac?'-x64':''));let j=findJava(dir);if(j)return j;
  if(legacyMac)log('Для старых версий Minecraft используется Java x64. На Apple Silicon требуется установленная Rosetta 2.');
  log(`Скачиваю Java ${major}…`);
  const f = path.join(dir, 'jdk.' + (os === 'windows' ? 'zip' : 'tar.gz'));
  await dl(`https://api.adoptium.net/v3/binary/latest/${major}/ga/${os}/${arch}/jdk/hotspot/normal/eclipse`, f);
  if(os==='windows')new AdmZip(f).extractAllTo(dir,true);else require('child_process').execFileSync('tar',['-xf',f,'-C',dir],{timeout:120000});fs.unlinkSync(f);
  j = findJava(dir); if (!j) throw new Error('Не удалось установить Java'); if (os !== 'windows') fs.chmodSync(j, 0o755);
  return j;
}
async function ensureLoader(inst, root, log) {
  if (inst.loader === 'Vanilla') return {};
  if (inst.loader === 'Fabric' || inst.loader === 'Quilt') {
    const base = inst.loader === 'Fabric' ? 'https://meta.fabricmc.net/v2' : 'https://meta.quiltmc.org/v3';
    const cache=path.join(root,'nova-loader.json');
    try{const c=JSON.parse(fs.readFileSync(cache,'utf8'));if(c.version===inst.version&&c.loader===inst.loader&&(!inst.loaderVersion||c.loaderVersion===inst.loaderVersion)&&fs.existsSync(path.join(root,'versions',c.id,c.id+'.json')))return {custom:c.id};}catch{}
    let lv=inst.loaderVersion;
    if(!lv){const response=await fetch(`${base}/versions/loader/${inst.version}`,{signal:AbortSignal.timeout(30000)});if(!response.ok)throw Error('Не удалось получить '+inst.loader);const versions=await response.json();lv=(versions.find(x=>x.loader?.stable)||versions[0])?.loader?.version;if(!lv)throw Error('Нет '+inst.loader+' для Minecraft '+inst.version);}
    const response=await fetch(`${base}/versions/loader/${inst.version}/${lv}/profile/json`,{signal:AbortSignal.timeout(30000)});if(!response.ok)throw Error('Не удалось скачать профиль '+inst.loader);const j=await response.json();if(!j.id||!/^[\w.+-]+$/.test(j.id)||!j.mainClass)throw Error('Некорректный профиль загрузчика');
    const d = path.join(root, 'versions', j.id); fs.mkdirSync(d, { recursive: true });
    fs.writeFileSync(path.join(d,j.id+'.json'),JSON.stringify(j));fs.writeFileSync(cache,JSON.stringify({id:j.id,loader:inst.loader,version:inst.version,loaderVersion:lv}));return {custom:j.id};
  }
  const f = path.join(root, 'installers', `${inst.loader}-${inst.version}.jar`);
  if (!fs.existsSync(f)) {
    log('Скачиваю установщик ' + inst.loader + '…');
    if (inst.loader === 'Forge') {
      const pr = inst.loaderVersion ? {} : (await (await fetch('https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json',{signal:AbortSignal.timeout(30000)})).json()).promos;
      const v = inst.loaderVersion || pr[inst.version + '-recommended'] || pr[inst.version + '-latest']; if (!v) throw new Error('Нет Forge для ' + inst.version);
      await dl(`https://maven.minecraftforge.net/net/minecraftforge/forge/${inst.version}-${v}/forge-${inst.version}-${v}-installer.jar`, f);
      fs.writeFileSync(path.join(root,'nova-pack.json'),JSON.stringify({dependencies:{minecraft:inst.version,forge:v}}));
    } else {
      const pre=neoPrefix(inst.version);
      const vs = inst.loaderVersion ? [] : (await (await fetch('https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge',{signal:AbortSignal.timeout(30000)})).json()).versions.filter(x => x.startsWith(pre) && !/alpha|beta/.test(x));
      if (!inst.loaderVersion&&!vs.length) throw new Error('Нет NeoForge для ' + inst.version); const v = inst.loaderVersion||vs.sort(compareVersions).at(-1);
      await dl(`https://maven.neoforged.net/releases/net/neoforged/neoforge/${v}/neoforge-${v}-installer.jar`, f);
      fs.writeFileSync(path.join(root,'nova-pack.json'),JSON.stringify({dependencies:{minecraft:inst.version,neoforge:v}}));
    }
  }
  return { forge: f };
}
ipcMain.handle('game:launch', async (_, inst, server) => {
  if(activeGame||modBusy)throw Error('Игра уже запущена или идёт обновление модов');
  inst=instanceById(inst.id);if(inst.loader==='Rust')return launchRust(inst,server);activeGame=true;presenceContext={phase:'launching',instance:inst};refreshPresence();
  const s=load().settings,root=iDir(inst.id),started=Date.now(),send=(c,...a)=>{if(!win.isDestroyed())win.webContents.send(c,...a);};
  let transcript='',finished=false,playSession;
  const log=m=>{const line=String(m).replace(/(--accessToken|--clientId|--xuid|--uuid)(\s+)(\S+)/gi,'$1$2[скрыто]');transcript=(transcript+line+'\n').slice(-180000);send('game:log',line);};
  const finish=code=>{
    if(finished)return;finished=true;if(playSession){try{history().finish(playSession,code);}catch(e){log('История: '+e.message);}}activeGame=false;presenceContext={phase:'launcher',instance:inst};refreshPresence();
    let report=null;
    if(code!==0){
      let text=transcript;
      try{const dir=path.join(root,'crash-reports');const files=fs.readdirSync(dir).map(n=>({n,stat:fs.statSync(path.join(dir,n))})).filter(x=>x.stat.isFile()&&x.stat.mtimeMs>=started-1000).sort((a,b)=>b.stat.mtimeMs-a.stat.mtimeMs);if(files[0])text+='\n'+fs.readFileSync(path.join(dir,files[0].n),'utf8').slice(-180000);}catch{}
      report={...analyzeCrash(text,code),instanceId:inst.id,at:new Date().toISOString()};
      try{fs.writeFileSync(path.join(root,'nova-last-crash.json'),JSON.stringify(report,null,2));}catch{}
    }
    send('game:closed',code,report);
  };
  try {
  fs.mkdirSync(root, { recursive: true });
  if (s.msRefresh) {try{await fromX(await authMgr().refresh(s.msRefresh));}catch{msToken=null;throw Error('Сессия Microsoft истекла. Выйдите из аккаунта и войдите снова.');}}
  else if(s.ms&&!msToken)throw Error('Войдите в Microsoft заново или выберите офлайн-режим');
  const ld = await ensureLoader(inst, root, log);
  if(inst.loader!=='Vanilla'&&!inst.loaderVersion){const pinned=loaderVersion(root,inst);if(pinned){inst.loaderVersion=pinned;const db=load(),saved=db.instances.find(x=>x.id===inst.id);if(saved){saved.loaderVersion=pinned;save(db);}}}
  let javaPath = s.java || undefined;
  if (!javaPath && s.autoJava !== false) javaPath = await ensureJava(javaMajor(inst.version), log);
  const l = new Client();
  patchClient(l,inst.version);patchLaunchCache(l);
  l.on('progress', e => send('game:progress', e.task, e.total ? Math.max(0,Math.min(1,e.task/e.total)) : 0));
  l.on('debug', log); l.on('data', log); l.on('error', e=>{log(e.message||e);finish(-1);});l.on('close', finish);
  const serverLaunch=quickPlay(inst.version,server);
  const ok = await l.launch({
    authorization: msToken || Authenticator.getAuth(s.name || 'Steve'),
    root, version: { number: inst.version, type: 'release', ...(ld.custom && { custom: ld.custom }) }, forge: ld.forge,
    memory: { max: s.ram + 'G', min: '1G' }, javaPath,
    customArgs: splitJvmArgs(s.jvm),
    window: { ...(s.ww && s.wh && { width: s.ww, height: s.wh }), fullscreen: !!s.full },
    ...(serverLaunch?{quickPlay:serverLaunch}:{}),
  });
  if(!ok)finish(1);
  else if(typeof ok.on==='function')ok.on('error',e=>{log(e.message);finish(-1);});
  if(ok&&!finished){try{playSession=history().start(inst);}catch(e){log('История: '+e.message);}presenceContext={phase:'playing',instance:inst,started:Math.floor(Date.now()/1000)};refreshPresence();send('game:started',inst.id);}
  return !!ok;
  }catch(e){log(e.message);finish(-1);throw e;}
});
