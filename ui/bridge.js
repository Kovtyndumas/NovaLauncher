(()=>{
 const call=(method,...args)=>window.__TAURI__.core.invoke('dispatch',{method,args}).catch(e=>{throw Error(typeof e==='string'?e:JSON.stringify(e));});
 const names=['aiKeyHelp','aiStatus','aiConfigure','aiPlan','aiInstall','compatibility','backups','rollback','exportPack','importPack','versions','history','duplicateInstance','worldBackups','backupWorlds','restoreWorlds','manageContent','launcherStatus','launcherCheck','launcherDownload','launcherInstall','rustSetup','horizonStatus','horizonChoose','horizonDownload','horizonRun','getDb','setDb','contentFiles','openContentDir','search','install','checkUpdates','updateMods','lastCrash','removeMod','removedMods','restoreRemoved','mods','toggle','launch','pack','ping','login','restore','logout','discordStatus','discordSelect','deleteInstance','openDir','importLegacy'];
 window.api=Object.fromEntries(names.map(name=>[name,(...args)=>call(name,...args)]));
 window.api.filePath=()=>null;
 const listeners=[];window.api.on=(channel,fn)=>{listeners.push(window.__TAURI__.event.listen(channel,e=>fn(...(Array.isArray(e.payload)?e.payload:[e.payload]))));};
 window.addEventListener('DOMContentLoaded',()=>call('ready'));
 // Tauri supplies native file paths through its drag/drop event, not File objects.
 window.__TAURI__.event.listen('tauri://drag-drop',e=>{const files=e.payload.paths||[];if(files.length===1&&files[0].endsWith('.mrpack'))window.importLocalPack?.(files[0]);});
})();
