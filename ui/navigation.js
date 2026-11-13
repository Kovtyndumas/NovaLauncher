function selectedInstanceId(db){const saved=db?.settings?.selectedInstance;return db?.instances?.some(inst=>inst.id===saved)?saved:db?.instances?.[0]?.id;}
async function selectInstance(id){
 const next=S.db.instances.some(inst=>inst.id===id)?id:selectedInstanceId(S.db);
 S.sel=next;
 if(next&&S.db.settings.selectedInstance!==next){S.db.settings.selectedInstance=next;await persist();}
 return next;
}
function contentInstancePicker(){return `<label class="content-instance-picker"><span>Сборка для контента</span><select id="contentInstance" aria-label="Сборка для установки контента" ${S.installing||S.importing||S.deleting||S.toolBusy||S.preflightBusy?'disabled':''}>${S.db.instances.map(inst=>`<option value="${esc(inst.id)}" ${inst.id===S.sel?'selected':''}>${esc(inst.name)} · ${esc(inst.loader)} ${esc(inst.version)}</option>`).join('')}</select></label>`;}
async function switchSelectedInstance(id){
 await selectInstance(id);if(S.tab==='settings')S.tab='play';S.crash=null;S.hits=[];S.contentFiles=[];S.catalogVersion=null;S.catalogLoader=null;S.catalogRequest++;
 await loadMods();
 if(['worlds','protection'].includes(S.tab)){NovaDesk.worlds=[];NovaDesk.points=[];render();await refreshDesk();return;}
 if(['health','changes'].includes(S.tab)){NovaHub.health=null;NovaHub.changes=[];render();await refreshHub();return;}
 render();if(S.tab==='content'&&S.contentPane!=='ai')await search();
}
function isRageLand(host){return String(host||'').trim().toLowerCase().replace(/\.$/,'').replace(/:25565$/,'').replace(/\.$/,'')==='mc.rageland.live';}
function closeLauncherTools(){const menu=document.querySelector('#launcherToolsMenu');if(menu)menu.hidden=true;document.querySelector('[data-launcher-tools]')?.setAttribute('aria-expanded','false');}
document.addEventListener('click',event=>{const trigger=event.target.closest('[data-launcher-tools]');if(trigger){const menu=document.querySelector('#launcherToolsMenu');menu.hidden=!menu.hidden;trigger.setAttribute('aria-expanded',String(!menu.hidden));if(!menu.hidden)menu.querySelector('button')?.focus();return;}if(!event.target.closest('#launcherToolsMenu')||event.target.closest('#launcherToolsMenu button'))closeLauncherTools();});
document.addEventListener('keydown',event=>{const menu=document.querySelector('#launcherToolsMenu');if(!menu||menu.hidden)return;if(event.key==='Escape'){event.preventDefault();closeLauncherTools();document.querySelector('[data-launcher-tools]')?.focus();}if(['ArrowDown','ArrowUp','Home','End'].includes(event.key)&&event.target.closest('#launcherToolsMenu')){event.preventDefault();const items=[...menu.querySelectorAll('button')].filter(item=>!item.disabled),index=items.indexOf(document.activeElement),next=event.key==='Home'?0:event.key==='End'?items.length-1:(index+(event.key==='ArrowDown'?1:-1)+items.length)%items.length;items[next]?.focus();}});
document.addEventListener('change',async event=>{if(event.target.id!=='contentInstance')return;try{await switchSelectedInstance(event.target.value);}catch(error){toast(error.message);render();}});

function updateLauncherToolContext(){const available=!!cur();for(const item of document.querySelectorAll('#launcherToolsMenu [data-tab="health"],#launcherToolsMenu [data-tab="changes"],#launcherToolsMenu [data-act="worlds"],#launcherToolsMenu [data-desk="points"]'))item.disabled=!available;}
