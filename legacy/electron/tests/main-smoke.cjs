const {app,BrowserWindow}=require('electron'),fs=require('fs'),path=require('path'),os=require('os'),assert=require('node:assert/strict');
const root=fs.mkdtempSync(path.join(os.tmpdir(),'nova-real-main-'));app.setPath('userData',root);
fs.writeFileSync(path.join(root,'db.json'),JSON.stringify({instances:[{id:'fixture',name:'Test instance',version:'1.20.1',loader:'Vanilla'}],servers:[],settings:{onboarded:true,name:'Test',ram:4,discordPresence:false,autoUpdate:false}}));
fs.mkdirSync(path.join(root,'instances','fixture','saves','World'),{recursive:true});fs.writeFileSync(path.join(root,'instances','fixture','saves','World','level.dat'),'test world');
require('../main');
const pause=ms=>new Promise(resolve=>setTimeout(resolve,ms));
app.whenReady().then(async()=>{try{let win;for(let n=0;n<120;n++){win=BrowserWindow.getAllWindows()[0];if(win&&await win.webContents.executeJavaScript("typeof S!=='undefined'&&!!S.db").catch(()=>false))break;await pause(100);}assert(win);const result=await win.webContents.executeJavaScript(`(async()=>{
 const versions=await api.versions();if(!versions.versions.includes('1.0'))throw Error('Missing1.0');
 const copy=await api.duplicateInstance('fixture');const backup=await api.backupWorlds('fixture');await api.restoreWorlds('fixture',backup.id);
 const history=await api.history(),horizon=await api.horizonStatus(),update=await api.launcherCheck();
 return {versions:versions.versions.length,copy:copy.instance.id,backup:backup.id,history:history.length,horizonPath:typeof horizon.path,update:update.phase};
})()`);assert(result.versions>40);assert.equal(result.history,0);assert.equal(result.update,'error');assert.equal(fs.readFileSync(path.join(root,'instances','fixture','saves','World','level.dat'),'utf8'),'test world');assert.equal(fs.readFileSync(path.join(root,'instances',result.copy,'saves','World','level.dat'),'utf8'),'test world');console.log(JSON.stringify({passed:true,...result}));fs.rmSync(root,{recursive:true,force:true});app.exit(0);}catch(e){console.error(e);fs.rmSync(root,{recursive:true,force:true});app.exit(1);}finally{fs.rmSync(root,{recursive:true,force:true});}});
