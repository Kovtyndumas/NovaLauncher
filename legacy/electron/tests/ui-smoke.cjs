const {app,BrowserWindow,session}=require('electron'),fs=require('fs'),path=require('path'),os=require('os'),assert=require('assert');
const output=path.join(os.tmpdir(),'nova-ui-review');fs.mkdirSync(output,{recursive:true});
app.setPath('userData',path.join(output,'electron-test-data'));
const pause=ms=>new Promise(r=>setTimeout(r,ms));
app.whenReady().then(async()=>{
 const w=new BrowserWindow({width:1100,height:700,show:false,webPreferences:{preload:path.join(__dirname,'mock-preload.cjs'),offscreen:true}});
 const errors=[];w.webContents.on('console-message',(_,level,msg)=>{if(level>=3&&!/Content Security Policy|ERR_|Failed to load resource/.test(msg)){errors.push(msg);console.error('RENDERER:',msg);}});
 try{
  await w.loadFile(path.join(__dirname,'..','index.html'));
  for(let i=0;i<80;i++){if(await w.webContents.executeJavaScript("!!document.querySelector('[data-act=play]')"))break;await pause(250);}
  for(let n=0;n<80;n++){if(await w.webContents.executeJavaScript('!NovaStartup.active&&!document.documentElement.classList.contains("startup-lock")'))break;await pause(50);}
  await w.webContents.executeJavaScript('document.fonts.ready.then(()=>true)');
  assert(await w.webContents.executeJavaScript("document.fonts.check('12px Monocraft')"));
  async function snap(name){console.log('Screen:',name);await pause(600);fs.writeFileSync(path.join(output,name+'.png'),(await w.webContents.capturePage()).toPNG());}
  await snap('play');
  assert(await w.webContents.executeJavaScript("document.documentElement.dataset.theme==='black'&&getComputedStyle(document.body).backgroundColor==='rgb(8, 9, 11)'"));

  const play=await w.webContents.executeJavaScript("({font:getComputedStyle(document.body).fontFamily,radius:getComputedStyle(document.querySelector('.acts .play')).borderRadius,glow:getComputedStyle(document.querySelector('.glow')).display,characterFilter:getComputedStyle(document.querySelector('.sk')).filter,overflow:document.documentElement.scrollWidth>innerWidth,toasts:[...document.querySelectorAll('.toast')].map(x=>x.textContent)})");assert.equal(play.radius,'18px');assert.equal(play.glow,'none');assert.equal(play.characterFilter,'none');assert(!play.overflow);assert(!play.toasts.some(x=>x.startsWith('Ошибка:')));
  await w.webContents.executeJavaScript("document.querySelector('[data-act=managemods]').click()");await pause(250);assert(await w.webContents.executeJavaScript("document.querySelectorAll('.mod-row').length===2"));await snap('mod-manager');
  await w.webContents.executeJavaScript("document.querySelector('#modQuery').value='local';document.querySelector('#modQuery').dispatchEvent(new Event('input',{bubbles:true}))");assert(await w.webContents.executeJavaScript("document.querySelectorAll('.mod-row').length===1"));
  await w.webContents.executeJavaScript("document.querySelector('#modQuery').value='';document.querySelector('#modQuery').dispatchEvent(new Event('input',{bubbles:true}));window.confirm=()=>true;document.querySelector('[data-remove-mod]').click()");await pause(250);assert(await w.webContents.executeJavaScript("S.mods.length===1"));
  await w.webContents.executeJavaScript("document.querySelector('[data-act=removedmods]').click()");await pause(200);await snap('removed-mods');await w.webContents.executeJavaScript("document.querySelector('[data-restore-removed]').click()");await pause(200);assert(await w.webContents.executeJavaScript("S.mods.length===2&&document.querySelectorAll('.mod-row').length===2&&document.querySelector('[data-restore-removed]').disabled"));await w.webContents.executeJavaScript("document.querySelector('[data-act=closeremoved]').click()");await pause(250);
  await w.webContents.executeJavaScript("document.querySelector('[data-tab=play]').click()");await pause(700);
  const tiltBox=await w.webContents.executeJavaScript("(()=>{const r=document.querySelector('.st').getBoundingClientRect();return {x:r.left+r.width*.7,y:r.top+r.height*.3};})()");
  w.webContents.sendInputEvent({type:'mouseMove',x:Math.round(tiltBox.x),y:Math.round(tiltBox.y)});await pause(120);
  assert(await w.webContents.executeJavaScript("document.querySelector('.st').classList.contains('pointer-motion')&&!!document.querySelector('.st').style.getPropertyValue('--tilt-x')"));
  w.webContents.sendInputEvent({type:'mouseMove',x:300,y:80});await pause(100);assert(await w.webContents.executeJavaScript("!document.querySelector('.st').classList.contains('pointer-motion')"));
  await w.webContents.executeJavaScript("(()=>{const button=document.querySelector('.acts .play'),r=button.getBoundingClientRect();button.dispatchEvent(new PointerEvent('pointerdown',{bubbles:true,button:0,clientX:r.left+20,clientY:r.top+20}));})()");
  assert(await w.webContents.executeJavaScript("document.querySelector('.acts .play').getAnimations().length>0"));
  await w.webContents.executeJavaScript("document.querySelector('[data-tab=history]').click()");await pause(150);assert(await w.webContents.executeJavaScript("!!document.querySelector('[data-replay=test]')&&document.querySelector('#view').textContent.includes('10 мин')"));await snap('history');
  await w.webContents.executeJavaScript("document.querySelector('[data-tab=horizon]').click()");await pause(150);assert(await w.webContents.executeJavaScript("!!document.querySelector('[data-horizon=play]')&&!document.querySelector('[data-horizon=play]').disabled"));await snap('horizon');
  await w.webContents.executeJavaScript("document.querySelector('[data-horizon=versions]').click()");await pause(150);assert(await w.webContents.executeJavaScript("!S.horizonBusy&&!S.toolBusy"));
  await w.webContents.executeJavaScript("document.querySelector('[data-tab=play]').click()");await pause(150);await w.webContents.executeJavaScript("document.querySelector('[data-act=worlds]').click()");await pause(150);assert(await w.webContents.executeJavaScript("document.querySelector('#worldsDlg').open&&!!document.querySelector('[data-world-restore]')"));await w.webContents.executeJavaScript("document.querySelector('[data-act=closeworlds]').click()");await pause(350);
  assert(await w.webContents.executeJavaScript("Array.from(document.querySelector('#v').options).some(o=>o.value==='1.0')"));
  await w.webContents.executeJavaScript("document.querySelector('[data-act=duplicate]').click()");await pause(150);assert(await w.webContents.executeJavaScript("S.sel==='copy'&&cur().name.includes('копия')"));await w.webContents.executeJavaScript("(async()=>{const r=await api.deleteInstance('copy');acceptDb(r.db);S.sel='test';await loadMods();render();})()");await pause(150);

  await w.webContents.executeJavaScript("document.querySelector('[data-act=new]').click();document.querySelector('#n').value='Rust Test';document.querySelector('#v').value='26.3-rust';document.querySelector('#v').dispatchEvent(new Event('change',{bubbles:true}));");assert(await w.webContents.executeJavaScript("document.querySelector('#l').disabled&&document.querySelector('#l').value==='Rust'"));
  await w.webContents.executeJavaScript("document.querySelector('#v').value='1.20.1';document.querySelector('#v').dispatchEvent(new Event('change',{bubbles:true}));");assert(await w.webContents.executeJavaScript("!document.querySelector('#l').disabled&&document.querySelector('#l').value!=='Rust'"));
  await w.webContents.executeJavaScript("document.querySelector('#v').value='26.3-rust';document.querySelector('#v').dispatchEvent(new Event('change',{bubbles:true}));document.querySelector('#ok').click()");await pause(180);assert(await w.webContents.executeJavaScript("cur().version==='26.3'&&cur().loader==='Rust'&&!!document.querySelector('[data-rust-action=install]')"));await snap('rust-instance');
  await w.webContents.executeJavaScript("document.querySelector('[data-rust-action=install]').click()");await pause(120);assert(await w.webContents.executeJavaScript("!S.toolBusy"));await w.webContents.executeJavaScript("document.querySelector('[data-tab=content]').click()");await pause(100);assert(await w.webContents.executeJavaScript("!document.querySelector('[data-ins]')&&document.querySelector('#view').textContent.includes('Horizon')"));
  await w.webContents.executeJavaScript("(async()=>{const id=cur().id,r=await api.deleteInstance(id);acceptDb(r.db);S.sel='test';S.tab='play';await loadMods();render();})()");await pause(150);
  await w.webContents.executeJavaScript("document.querySelector('[data-tab=settings]').click()");await pause(300);assert(await w.webContents.executeJavaScript("!!document.querySelector('#characterSize')"));
  for(const theme of ['graphite','light','black']){await w.webContents.executeJavaScript("document.querySelector('[data-theme-choice="+theme+"]').click()");await pause(150);assert(await w.webContents.executeJavaScript("S.db.settings.theme==='"+theme+"'&&document.documentElement.dataset.theme==='"+theme+"'"));await pause(650);assert(await w.webContents.executeJavaScript("!document.documentElement.classList.contains('theme-changing')"));if(theme!=='black')await snap('theme-'+theme);}
  await w.webContents.executeJavaScript("document.querySelector('#characterSize').value='small';document.querySelector('#characterSize').dispatchEvent(new Event('change',{bubbles:true}));document.querySelector('[data-set=characterLook]').click()");
  assert(await w.webContents.executeJavaScript("S.db.settings.characterSize==='small'&&S.db.settings.characterLook===false"));
  await w.webContents.executeJavaScript("document.querySelector('#view').scrollTop=320");await snap('settings');
  await w.webContents.executeJavaScript("document.querySelector('[data-tab=servers]').click()");await pause(300);
  await w.webContents.executeJavaScript("document.querySelectorAll('[data-fav]')[1].click()");await pause(100);assert(await w.webContents.executeJavaScript('S.db.servers[1].favorite'));
  await w.webContents.executeJavaScript("document.querySelector('[data-act=favorites]').click()");await snap('servers');
  await w.webContents.executeJavaScript("document.querySelector('[data-tab=content]').click()");await pause(300);assert.equal((await w.webContents.executeJavaScript('api.testCounts()')).lastSearch.ver,'1.20.1');
  await w.webContents.executeJavaScript("document.querySelector('[data-act=catalogversion]').click()");await snap('catalog-version');
  await w.webContents.executeJavaScript("document.querySelector('#catalogVersion').value='1.21.1';document.querySelector('[data-act=catalogapply]').click()");await pause(250);
  assert(await w.webContents.executeJavaScript("document.querySelector('[data-ins]').disabled&&cur().version==='1.20.1'"));assert.equal((await w.webContents.executeJavaScript('api.testCounts()')).lastSearch.ver,'1.21.1');await snap('catalog-preview');
  await w.webContents.executeJavaScript("document.querySelector('[data-act=catalogreset]').click()");await pause(250);assert(await w.webContents.executeJavaScript("!document.querySelector('[data-ins]').disabled"));
  // Non-mod catalogs must install, retain their status, and expose downloaded files.
  await w.webContents.executeJavaScript("document.querySelector('#q').value='sodium';document.querySelector('#ty').value='shader';document.querySelector('#ty').dispatchEvent(new Event('change',{bubbles:true}))");await pause(150);
  assert.equal((await w.webContents.executeJavaScript('api.testCounts()')).lastSearch.q,'');
  await w.webContents.executeJavaScript("document.querySelector('[data-ins]').click()");await pause(20);assert(await w.webContents.executeJavaScript("document.querySelector('[data-ins]').disabled&&document.querySelector('[data-ins]').textContent.includes('Устанавливаем')"));await pause(180);
  assert(await w.webContents.executeJavaScript("document.querySelector('[data-ins]').textContent==='Установлено'&&S.contentFiles[0].name==='sample.zip'"));await snap('content-shader-installed');
  await w.webContents.executeJavaScript("document.querySelector('[data-content-action=toggle]').click()");await pause(120);assert(await w.webContents.executeJavaScript("S.contentFiles[0].active===true&&document.querySelector('[data-content-action=toggle]').textContent==='Выключить'"));
  await w.webContents.executeJavaScript("document.querySelector('[data-content-action=update]').click()");await pause(120);assert(await w.webContents.executeJavaScript("!S.toolBusy"));

  await w.webContents.executeJavaScript("document.querySelector('#ty').value='resourcepack';document.querySelector('#ty').dispatchEvent(new Event('change',{bubbles:true}))");await pause(100);
  await w.webContents.executeJavaScript("S.catalogLoader='Forge';render();api.testFailInstall().then(()=>document.querySelector('[data-ins]').click())");await pause(200);
  assert(await w.webContents.executeJavaScript("document.querySelector('[role=alert]').textContent==='Test download failed'&&document.querySelector('[data-ins]').textContent==='Повторить'"));
  await w.webContents.executeJavaScript("document.querySelector('[data-ins]').click()");await pause(200);assert(await w.webContents.executeJavaScript("document.querySelector('[data-ins]').textContent==='Установлено'&&S.contentFiles.length===1"));
  await w.webContents.executeJavaScript("document.querySelector('[data-act=contentfolder]').click()");await pause(50);assert.equal((await w.webContents.executeJavaScript('api.testCounts()')).openedContent,1);await snap('content-resourcepack-installed');
  await w.webContents.executeJavaScript("document.querySelector('#ty').value='modpack';document.querySelector('#ty').dispatchEvent(new Event('change',{bubbles:true}))");await pause(100);await w.webContents.executeJavaScript("document.querySelector('[data-ins]').click()");await pause(250);
  assert.equal((await w.webContents.executeJavaScript('api.testCounts()')).lastPack.version,'1.20.1');assert(await w.webContents.executeJavaScript("S.sel.startsWith('imported-')&&!S.importing&&!S.installing"));
  await w.webContents.executeJavaScript("S.sel='test';S.tab='content';S.type='shader';S.installs={};S.catalogLoader=null;S.catalogVersion=null;search()");await pause(150);assert(await w.webContents.executeJavaScript("document.querySelector('[data-ins]').textContent==='Установлено'"));
  await w.webContents.executeJavaScript("S.type='mod';search()");await pause(150);
  await w.webContents.executeJavaScript("document.querySelector('[data-act=checkupdates]').click()");await pause(300);
  assert(await w.webContents.executeJavaScript("document.querySelectorAll('[data-update]').length===1"));await snap('updates');
  await w.webContents.executeJavaScript("document.querySelector('[data-act=compatibility]').click()");await pause(250);assert(await w.webContents.executeJavaScript("document.querySelector('.compatibility').textContent.includes('fabric-api')"));await snap('compatibility');
  await w.webContents.executeJavaScript("document.querySelector('[data-act=exportpack]').click()");await pause(250);assert.equal((await w.webContents.executeJavaScript('api.testCounts()')).exports,1);
  await w.webContents.executeJavaScript("document.querySelector('[data-act=rollback]').click()");await pause(250);assert(await w.webContents.executeJavaScript("document.querySelector('#rollbackDlg').open"));await snap('rollback');
  await w.webContents.executeJavaScript("window.confirm=()=>true;document.querySelector('[data-restore]').click()");await pause(250);assert(await w.webContents.executeJavaScript("document.querySelector('[data-restore]').disabled"));assert((await w.webContents.executeJavaScript('api.testCounts()')).rolledBack);
  await w.webContents.executeJavaScript("document.querySelector('[data-act=closerollback]').click()");await pause(250);
  await w.webContents.executeJavaScript("document.querySelector('[data-tab=console]').click()");await pause(200);await w.webContents.executeJavaScript("document.querySelector('[data-act=lastcrash]').click()");await snap('crash');
  assert(await w.webContents.executeJavaScript("document.querySelector('.diagnosis').textContent.includes('MixinApplyError')"));
  w.setSize(900,600);await w.webContents.executeJavaScript("document.querySelector('[data-tab=play]').click()");await snap('play-small');
  assert(await w.webContents.executeJavaScript("document.querySelector('.acts .play').getBoundingClientRect().right<innerWidth"));
  const animation=await w.webContents.executeJavaScript(`(()=>{
    const c=S.character,v=S.v3,p=v.playerObject;v.renderPaused=true;
    c.cursor.active=false;c.settings.characterAuto=false;
    let max=0,maxJump=0,maxRunTilt=0,maxRunArm=0,last=p.skin.rightArm.quaternion.clone();
    const V=p.position.constructor;
    for(const mode of ['idle','wave','cool','look','run','dance','victory','nod','jump']){
      c.set(mode,Infinity);
      for(let frame=0;frame<180;frame++){
        c.frame(p,1/60);
        if(mode==='run'&&frame>40){maxRunTilt=Math.max(maxRunTilt,Math.abs(p.rotation.x));maxRunArm=Math.max(maxRunArm,Math.abs(p.skin.leftArm.rotation.x),Math.abs(p.skin.rightArm.rotation.x));}
        maxJump=Math.max(maxJump,last.angleTo(p.skin.rightArm.quaternion));last.copy(p.skin.rightArm.quaternion);
        if(frame%6)continue;
        p.updateWorldMatrix(true,true);v.camera.updateMatrixWorld(true);
        p.traverseVisible(mesh=>{if(!mesh.isMesh)return;mesh.geometry.computeBoundingBox();const b=mesh.geometry.boundingBox;
          for(const x of [b.min.x,b.max.x])for(const y of [b.min.y,b.max.y])for(const z of [b.min.z,b.max.z]){
            const pt=new V(x,y,z).applyMatrix4(mesh.matrixWorld).project(v.camera);max=Math.max(max,Math.abs(pt.x),Math.abs(pt.y));
          }
        });
      }
    }
    c.set('idle',Infinity);for(let i=0;i<60;i++)c.frame(p,1/60);v.render();
    return {max,maxJump,maxRunTilt,maxRunArm,finite:Number.isFinite(max)&&Number.isFinite(maxJump)};
  })()`);
  assert(animation.finite);assert(animation.max<1,'Character clips outside canvas: '+animation.max);assert(animation.maxJump<.3,'Abrupt joint motion: '+animation.maxJump);
  assert(animation.maxRunTilt<.1);assert(animation.maxRunArm<.75);
  await snap('animation-final');
  for(let pose=0;pose<3;pose++){await w.webContents.executeJavaScript("(()=>{const c=S.character,v=S.v3;if(c.mode!=='run')c.set('run',Infinity);for(let n=0;n<15;n++)c.frame(v.playerObject,1/60);v.render();})()");await snap('run-pose-'+pose);}
  await w.webContents.executeJavaScript("S.character.set('idle',Infinity)");
  await w.webContents.executeJavaScript("document.querySelector('[data-act=importpack]').click()");await pause(300);
  assert(await w.webContents.executeJavaScript("S.sel.startsWith('imported-')&&!S.importing&&!document.querySelector('#packDlg').open"));
  const beforeDrop=await w.webContents.executeJavaScript('S.db.instances.length');
  await w.webContents.executeJavaScript("(()=>{const transfer=new DataTransfer();transfer.items.add(new File(['test'],'example.mrpack'));document.dispatchEvent(new DragEvent('drop',{dataTransfer:transfer,bubbles:true,cancelable:true}));})()");await pause(300);
  assert.equal(await w.webContents.executeJavaScript('S.db.instances.length'),beforeDrop+1);
  console.log('Animation bounds and transitions:',JSON.stringify(animation));
  await w.webContents.executeJavaScript("document.querySelector('[data-act=play]').click()");await pause(200);
  assert(await w.webContents.executeJavaScript("document.querySelector('#compatDlg').open&&!S.playing"));assert.equal((await w.webContents.executeJavaScript('api.testCounts()')).launches,0);await snap('preflight');
  await w.webContents.executeJavaScript("document.querySelector('[data-act=cancelpreflight]').click()");await pause(250);assert(await w.webContents.executeJavaScript("!document.querySelector('#compatDlg').open&&!S.playing"));
  await w.webContents.executeJavaScript("document.querySelector('[data-act=play]').click()");await pause(150);await w.webContents.executeJavaScript("document.querySelector('[data-act=forceplay]').click()");await pause(200);assert.equal((await w.webContents.executeJavaScript('api.testCounts()')).launches,1);
  w.webContents.debugger.attach('1.3');
  await w.webContents.debugger.sendCommand('Emulation.setEmulatedMedia',{features:[{name:'prefers-reduced-motion',value:'reduce'}]});
  await w.webContents.executeJavaScript("document.querySelector('[data-tab=settings]').click()");await pause(100);
  await w.webContents.executeJavaScript("document.querySelector('[data-theme-choice=light]').click()");await pause(100);
  assert(await w.webContents.executeJavaScript("document.documentElement.dataset.theme==='light'&&!document.documentElement.classList.contains('theme-changing')&&getComputedStyle(document.querySelector('.theme-choice')).transitionDuration==='0s'"));
  w.webContents.debugger.detach();
  // The instance delete action is visible in the sidebar on every tab.
  assert(await w.webContents.executeJavaScript("document.querySelector('aside [data-act=del]').disabled"));
  await w.webContents.executeJavaScript("S.playing=false;S.gameRunning=false;S.installing=true;render()");assert(await w.webContents.executeJavaScript("document.querySelector('aside [data-act=del]').disabled"));
  await w.webContents.executeJavaScript("S.installing=false;S.tab='settings';render()");await snap('instance-delete-button');
  assert(await w.webContents.executeJavaScript("document.querySelector('aside [data-act=del]').textContent==='Удалить инстанс'&&!document.querySelector('aside [data-act=del]').disabled"));
  const deleteBefore=await w.webContents.executeJavaScript('S.db.instances.length');
  await w.webContents.executeJavaScript("api.testCancelDelete().then(()=>document.querySelector('aside [data-act=del]').click())");await pause(100);assert.equal(await w.webContents.executeJavaScript('S.db.instances.length'),deleteBefore);
  await w.webContents.executeJavaScript("document.querySelector('aside [data-act=del]').click()");await pause(150);assert.equal(await w.webContents.executeJavaScript('S.db.instances.length'),deleteBefore-1);assert(await w.webContents.executeJavaScript("!!cur()&&!S.deleting"));
  await w.webContents.executeJavaScript("(async()=>{const d=await api.getDb();d.instances=[d.instances[0]];await api.setDb(d);acceptDb(d);S.sel=d.instances[0].id;S.tab='play';render();})()");
  await w.webContents.executeJavaScript("document.querySelector('aside [data-act=del]').click()");await pause(150);assert(await w.webContents.executeJavaScript("S.db.instances.length===0&&!cur()&&!document.querySelector('aside [data-act=del]')&&!!document.querySelector('[data-act=new]')"));await snap('instance-deleted-last');
  assert(!errors.length,errors.join('\n'));
  console.log(JSON.stringify({passed:true,output,play}));app.exit(0);
 }catch(e){console.error(e.stack);app.exit(1);}
});
