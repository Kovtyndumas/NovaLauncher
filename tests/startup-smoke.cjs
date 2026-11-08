const {app,BrowserWindow,session}=require('electron'),path=require('path'),fs=require('fs'),os=require('os'),assert=require('node:assert/strict');
const root=fs.mkdtempSync(path.join(os.tmpdir(),'nova-intro-test-'));app.setPath('userData',root);const output=path.join(os.tmpdir(),'nova-startup-review');fs.mkdirSync(output,{recursive:true});
const wait=ms=>new Promise(r=>setTimeout(r,ms));
app.whenReady().then(async()=>{
 // Intro tests use local fixtures; remote avatars/skins must not delay the load event.
 session.defaultSession.webRequest.onBeforeRequest({urls:['https://*/*']},(_,done)=>done({cancel:true}));
 const windows=[];try{
 for(const first of [true,false]){
  const w=new BrowserWindow({width:1100,height:700,show:false,webPreferences:{offscreen:true,backgroundThrottling:false,preload:path.join(__dirname,'mock-preload.cjs'),additionalArguments:first?['--nova-first-start']:[]}});windows.push(w);
  await w.loadURL('about:blank');await w.webContents.debugger.attach('1.3');await w.webContents.debugger.sendCommand('Emulation.setEmulatedMedia',{features:[{name:'prefers-reduced-motion',value:'no-preference'}]});
  await w.loadFile(path.join(__dirname,'..','index.html'));
  for(let n=0;n<60;n++){if(await w.webContents.executeJavaScript("typeof NovaStartup!=='undefined'&&NovaStartup.active"))break;await wait(20);}
  assert.equal(await w.webContents.executeJavaScript("document.querySelector('#startupOverlay').dataset.mode"),first?'first':'returning');
  assert(await w.webContents.executeJavaScript("document.querySelector('main').inert&&document.activeElement.classList.contains('startup-skip')"));
  await wait(first?1050:250);fs.writeFileSync(path.join(output,first?'first-opening.png':'normal-opening.png'),(await w.webContents.capturePage()).toPNG());
  if(first){await w.webContents.executeJavaScript("document.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',bubbles:true}))");}
  for(let n=0;n<100;n++){if(await w.webContents.executeJavaScript('!NovaStartup.active'))break;await wait(30);}
  assert(await w.webContents.executeJavaScript("document.querySelector('#startupOverlay').hidden&&!document.querySelector('main').inert&&!document.documentElement.classList.contains('startup-lock')&&S.db.settings.introSeen===true"));
  assert.equal(await w.webContents.executeJavaScript("document.querySelector('#onb').classList.contains('onb')"),first);
  if(first)await w.webContents.executeJavaScript("document.querySelector('#o3').click()");
  await wait(650);await w.webContents.executeJavaScript("document.querySelector('[data-tab=settings]').click()");await wait(100);
  await w.webContents.executeJavaScript("document.querySelector('[data-act=intropreview]').click()");assert(await w.webContents.executeJavaScript("NovaStartup.active&&document.querySelector('#startupOverlay').dataset.mode==='first'"));
  await w.webContents.executeJavaScript('NovaStartup.skip()');await wait(800);assert(await w.webContents.executeJavaScript("!NovaStartup.active&&S.tab==='settings'&&S.db.settings.onboarded===true"));
  await w.webContents.debugger.sendCommand('Emulation.setEmulatedMedia',{features:[{name:'prefers-reduced-motion',value:'reduce'}]});
  const start=Date.now();await w.webContents.executeJavaScript('NovaStartup.play({first:true})');assert(Date.now()-start<900);assert(await w.webContents.executeJavaScript("!NovaStartup.active&&document.querySelector('#startupOverlay').hidden"));w.webContents.debugger.detach();
 }
 console.log(JSON.stringify({passed:true,firstOpening:true,returningOpening:true,skip:true,settingsPreview:true,reducedMotion:true,output}));
 for(const w of windows)w.destroy();fs.rmSync(root,{recursive:true,force:true});app.exit(0);
 }catch(e){console.error(e);for(const w of windows)w.destroy();app.exit(1);}});
