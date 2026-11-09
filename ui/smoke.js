(async()=>{
 const sleep=ms=>new Promise(r=>setTimeout(r,ms));
 const checks=[];
 const assert=(ok,message)=>{if(!ok)throw Error(message);};
 const waitFor=async condition=>{for(let n=0;n<150;n++){if(condition())return;await sleep(30);}throw Error('Timed out waiting for UI');};
 const luminance=color=>{const values=color.match(/[\d.]+/g).slice(0,3).map(Number).map(v=>{v/=255;return v<=.04045?v/12.92:((v+.055)/1.055)**2.4;});return .2126*values[0]+.7152*values[1]+.0722*values[2];};
 try{
  await waitFor(()=>window.NovaReady===true&&!NovaStartup.active);
  await document.fonts.ready;assert(document.fonts.check('12px Unbounded'),'Unbounded did not load');checks.push('font-Unbounded');
  const animatedGrid=document.createElement('div');animatedGrid.className='grid';const animatedCard=document.createElement('article');animatedCard.className='card';animatedCard.textContent='Motion test';let contentAnimated=false;const nativeAnimate=animatedCard.animate.bind(animatedCard);animatedCard.animate=(...args)=>{contentAnimated=true;return nativeAnimate(...args);};animatedGrid.append(animatedCard);document.querySelector('#view').append(animatedGrid);const reduced=matchMedia('(prefers-reduced-motion: reduce)').matches;if(reduced){await sleep(150);assert(!contentAnimated,'Reduced motion preference ignored');}else{await waitFor(()=>contentAnimated);assert(contentAnimated,'Async content did not animate');}animatedGrid.remove();checks.push(reduced?'async-content-reduced-motion':'async-content-motion');
  for(const tab of ['play','mods','settings','servers','console','history','horizon']){S.tab=tab;render();await sleep(70);assert(document.querySelector('#view').textContent.trim(),'Empty '+tab);checks.push(tab);}
  const id=S.sel,initial=await api.getDb();const copy=await api.duplicateInstance(id);assert(copy.db.instances.length===initial.instances.length+1,'Duplicate failed');acceptDb(copy.db);checks.push('rust-duplicate');
  const colours=[];
  for(const theme of ['black','graphite','light']){
   S.db.settings.theme=theme;applyTheme();
   for(const instance of S.db.instances){S.sel=instance.id;S.tab='play';render();await sleep(230);window.getSelection()?.removeAllRanges();const title=document.querySelector('h1.g');const css=getComputedStyle(title),fg=css.webkitTextFillColor||css.color,bg=getComputedStyle(document.body).backgroundColor;assert(fg!=='transparent'&&!fg.endsWith(', 0)'),'Transparent instance title');const a=luminance(fg),b=luminance(bg),contrast=(Math.max(a,b)+.05)/(Math.min(a,b)+.05);assert(contrast>=7,`Title contrast ${theme}: ${contrast} (${fg} on ${bg})`);const accent=getComputedStyle(document.documentElement).getPropertyValue('--acc').trim();assert(accent==='#0a84ff','Instance changed the blue accent: '+accent);colours.push({theme,instance:instance.id,fg,contrast});}
   checks.push('theme-'+theme+'-readable-title');
  }
  checks.push('single-blue-accent');S.sel=id;S.db.settings.theme='black';applyTheme();
  // Check the real first-run form, including cancelled and duplicate login attempts.
  S.db.settings.onboarded=false;S.db.settings.name='Steve';onb();await sleep(30);
  assert(document.activeElement!==document.querySelector('#o2'),'Nickname was forcibly focused');
  const field=document.querySelector('#o2');field.focus();assert(getComputedStyle(field).boxShadow==='none','Nickname has a focus halo');field.blur();
  document.querySelector('.welcome-card').requestSubmit();await sleep(40);assert(field.getAttribute('aria-invalid')==='true','Empty nickname accepted');
  assert(document.querySelector('#welcomeError').textContent,'Missing nickname validation');
  const realLogin=api.login;let loginCalls=0,rejectLogin;
  api.login=()=>{loginCalls++;return new Promise((_,reject)=>{rejectLogin=reject;});};
  document.querySelector('#o1').click();document.querySelector('#o1').onclick();
  assert(loginCalls===1,'Duplicate Microsoft windows requested');assert(field.disabled,'Nickname editable during sign-in');
  rejectLogin(Error('Вход отменён'));await waitFor(()=>!field.disabled);assert(document.querySelector('#o1').textContent==='Войти через Microsoft','Cancelled sign-in did not reset button');api.login=realLogin;checks.push('welcome-cancelled-login');
  field.value='bad name';document.querySelector('.welcome-card').requestSubmit();await sleep(30);assert(field.getAttribute('aria-invalid')==='true','Invalid nickname accepted');
  field.value='NovaUser';field.dispatchEvent(new Event('input'));
  const realSet=api.setDb;let fail=true;api.setDb=(...args)=>{if(fail){fail=false;return Promise.reject(Error('Test save failure'));}return realSet(...args);};
  document.querySelector('.welcome-card').requestSubmit();await waitFor(()=>!field.disabled);assert(document.querySelector('.welcome-card'),'Welcome closed despite save failure');assert(!S.db.settings.onboarded,'Failed welcome was marked completed');
  document.querySelector('.welcome-card').requestSubmit();await waitFor(()=>!document.querySelector('.welcome-card'));api.setDb=realSet;
  let saved=await api.getDb();assert(saved.settings.name==='NovaUser'&&saved.settings.onboarded,'Welcome retry did not persist nickname');checks.push('welcome-validation-and-save-retry');
  // A rejected save must not make the next save silently omit the failed setting.
  let rejectSave=true;api.setDb=(...args)=>{if(rejectSave){rejectSave=false;return Promise.reject(Error('Test save failure'));}return realSet(...args);};
  S.db.settings.ram=6;await persist().catch(()=>{});S.db.settings.jvm='-Dnova.test=true';await persist();api.setDb=realSet;
  saved=await api.getDb();assert(saved.settings.ram===6&&saved.settings.jvm==='-Dnova.test=true','Failed save lost a setting');checks.push('settings-save-recovery');
  const worlds=await api.backupWorlds(id);assert(worlds.id,'World backup failed');checks.push('rust-backup');
  acceptDb(await api.getDb());S.sel=id;S.tab='play';render();await sleep(250);const canvas=document.querySelector('#skin3d'),fallback=document.querySelector('.sk [data-character-fallback]');assert(canvas?.width>0||fallback,'Character representation absent');checks.push(canvas?'character-canvas':'character-image-fallback');
  await window.__TAURI__.core.invoke('dispatch',{method:'smokeReport',args:[{ok:true,checks,colours,webview:navigator.userAgent}]});
 }catch(e){await window.__TAURI__.core.invoke('dispatch',{method:'smokeReport',args:[{ok:false,error:e.message,stack:e.stack,startup:document.querySelector('.startup-caption')?.textContent,reducedMotion:matchMedia('(prefers-reduced-motion: reduce)').matches,visibility:document.visibilityState,checks}]});}
})();
