const layoutDefaults={info:{x:0,y:0,w:7,h:3},actions:{x:0,y:3,w:7,h:2},stats:{x:0,y:5,w:7,h:3},character:{x:7,y:0,w:5,h:8},favorite:{x:0,y:8,w:6,h:3},history:{x:6,y:8,w:6,h:3},quick:{x:0,y:11,w:6,h:3},memory:{x:6,y:11,w:6,h:3}};
const layoutNames=Object.fromEntries(Object.entries(moduleCatalog).map(([key,item])=>[key,item.name]));
let layoutEditing=false,layoutDock=null,layoutHistory=[],layoutDrag=null,layoutPreviousInert=[];
function cleanLayout(value){const result={};for(const [key,defaults]of Object.entries(layoutDefaults)){const source=value?.[key]||{};const integer=(name,min,max)=>Math.max(min,Math.min(max,Number.isFinite(Number(source[name]))?Math.round(Number(source[name])):defaults[name]));const w=integer('w',key==='character'?3:4,12),h=integer('h',key==='character'?6:2,18);result[key]={x:integer('x',0,12-w),y:integer('y',0,128),w,h};}return result;}
function layoutVisible(key){return appearanceValue().modules[key]===true;}
function layoutIntersects(a,b){return a.x<b.x+b.w&&a.x+a.w>b.x&&a.y<b.y+b.h&&a.y+a.h>b.y;}
function settleLayout(source,first){const result=cleanLayout(source),placed=[];const keys=Object.keys(result).filter(layoutVisible).sort((a,b)=>a===first?-1:b===first?1:result[a].y-result[b].y);for(const key of keys){const rect=result[key];let overlapping;while((overlapping=placed.find(other=>layoutIntersects(rect,result[other]))))rect.y=result[overlapping].y+result[overlapping].h;placed.push(key);}return result;}
function applyLayout(){
 const hero=document.querySelector('.hero2'),layout=appearanceValue().layout;
 document.documentElement.dataset.customLayout=layout||layoutEditing||Object.keys(moduleCatalog).some(key=>layoutVisible(key)!==['info','actions','stats','character'].includes(key))?'true':'false';
 if(!hero)return;applyDesign();
 const positions=settleLayout(layout||layoutDefaults);
 const empty=hero.querySelector('.module-empty');if(!Object.keys(moduleCatalog).some(layoutVisible)){if(!empty){const placeholder=document.createElement('div');placeholder.className='module-empty';placeholder.innerHTML='<h2>Твой экран Nova</h2><p>Выбери модули, которые нужны тебе.</p><button class="b" data-act="editmodules">Добавить модули</button>';hero.append(placeholder);}}else empty?.remove();
 for(const element of hero.querySelectorAll('[data-layout-block]')){
  const key=element.dataset.layoutBlock,position=positions[key];element.dataset.widgetCompact=String(position.h<=2);element.style.display=layoutVisible(key)?'':'none';
  if(document.documentElement.dataset.customLayout==='true'){element.style.gridColumn=`${position.x+1} / span ${position.w}`;element.style.gridRow=`${position.y+1} / span ${position.h}`;}
  else{element.style.removeProperty('grid-column');element.style.removeProperty('grid-row');}
  if(layoutEditing&&layoutVisible(key)&&!element.querySelector('.layout-grab')){
   const grab=document.createElement('button');grab.className='layout-grab';grab.type='button';grab.dataset.layoutHandle=key;grab.textContent=layoutNames[key];grab.setAttribute('aria-label','Переместить: '+layoutNames[key]);grab.title='Перетащи или используй стрелки. Shift + стрелки — размер.';
   const resize=document.createElement('button');resize.className='layout-resize';resize.type='button';resize.dataset.layoutHandle=key;resize.dataset.resize='true';resize.innerHTML='<span></span>';resize.setAttribute('aria-label','Размер: '+layoutNames[key]);
   const remove=document.createElement('button');remove.className='layout-remove';remove.type='button';remove.textContent='−';remove.setAttribute('aria-label','Убрать модуль: '+layoutNames[key]);remove.onclick=()=>setModuleEnabled(key,false);const config=document.createElement('button');config.className='layout-config';config.textContent='⚙';config.setAttribute('aria-label','Настроить: '+layoutNames[key]);config.onclick=()=>openModuleSettings(key);element.append(grab,resize,remove,config);for(const handle of [grab,resize]){handle.addEventListener('pointerdown',startLayoutDrag);handle.addEventListener('pointermove',moveLayoutDrag);handle.addEventListener('pointerup',finishLayoutDrag);handle.addEventListener('pointercancel',cancelLayoutDrag);handle.addEventListener('lostpointercapture',finishLayoutDrag);handle.addEventListener('keydown',layoutKey);}
  }
 }
}
function layoutRemember(before){layoutHistory.push(structuredClone(before));if(layoutHistory.length>20)layoutHistory.shift();const undo=layoutDock?.querySelector('[data-layout-action=undo]');if(undo)undo.disabled=false;}
function startLayoutDrag(event){
 if(!layoutEditing||event.button!==0)return;event.preventDefault();event.stopPropagation();motionUI.reset();
 const handle=event.currentTarget,key=handle.dataset.layoutHandle,hero=document.querySelector('.hero2'),view=document.querySelector('#view');
 layoutDrag={key,resize:handle.dataset.resize==='true',handle,before:structuredClone(appearanceDraft),x:event.clientX,y:event.clientY,scroll:view.scrollTop,step:(hero.clientWidth+12)/12,pointer:event.pointerId};
 handle.setPointerCapture(event.pointerId);handle.closest('[data-layout-block]').classList.add('layout-dragging');
}
function moveLayoutDrag(event){
 const drag=layoutDrag;if(!drag||drag.pointer!==event.pointerId)return;event.preventDefault();
 const dx=Math.round((event.clientX-drag.x)/drag.step),dy=Math.round((event.clientY-drag.y+document.querySelector('#view').scrollTop-drag.scroll)/54),source=cleanLayout(drag.before.layout),rect=source[drag.key];
 if(drag.resize){rect.w=Math.max(drag.key==='character'?3:4,Math.min(12-rect.x,rect.w+dx));rect.h=Math.max(drag.key==='character'?6:2,Math.min(18,rect.h+dy));}
 else{rect.x=Math.max(0,Math.min(12-rect.w,rect.x+dx));rect.y=Math.max(0,Math.min(60,rect.y+dy));}
 appearanceDraft.layout=settleLayout(source,drag.key);applyLayout();
 const message=layoutDock?.querySelector('[role=status]');if(message)message.textContent=layoutNames[drag.key];
}
function finishLayoutDrag(event){const drag=layoutDrag;if(!drag)return;layoutDrag=null;drag.handle.closest('[data-layout-block]')?.classList.remove('layout-dragging');if(drag.handle.hasPointerCapture(drag.pointer))drag.handle.releasePointerCapture(drag.pointer);if(JSON.stringify(drag.before.layout)!==JSON.stringify(appearanceDraft.layout))layoutRemember(drag.before);}
function cancelLayoutDrag(){const drag=layoutDrag;if(!drag)return;appearanceDraft=drag.before;finishLayoutDrag();applyLayout();}
function layoutKey(event){if(!['ArrowLeft','ArrowRight','ArrowUp','ArrowDown'].includes(event.key))return;event.preventDefault();event.stopPropagation();const key=event.currentTarget.dataset.layoutHandle,before=structuredClone(appearanceDraft),source=cleanLayout(appearanceDraft.layout),rect=source[key],resize=event.shiftKey||event.currentTarget.dataset.resize==='true';if(event.key==='ArrowLeft')rect[resize?'w':'x']--;if(event.key==='ArrowRight')rect[resize?'w':'x']++;if(event.key==='ArrowUp')rect[resize?'h':'y']--;if(event.key==='ArrowDown')rect[resize?'h':'y']++;rect.y=Math.max(0,Math.min(60,rect.y));appearanceDraft.layout=settleLayout(cleanLayout(source),key);layoutRemember(before);applyLayout();}
function endLayoutEdit(save=false){
 if(appearanceSaving)return;closeModuleGallery();layoutEditing=false;cancelLayoutDrag();document.documentElement.dataset.layoutEditing='false';layoutDock?.remove();layoutDock=null;
 for(const [element,value]of layoutPreviousInert)element.inert=value;layoutPreviousInert=[];appearanceDraft=null;render();
}
function startLayoutEdit(){
 if(layoutEditing)return;if(!cur()){toast('Сначала создайте сборку для предпросмотра интерфейса');return;}
 appearanceDraft=appearanceDraft||{...cleanAppearance(S.db.settings.appearance),theme:S.db.settings.theme||'black'};
 if(appearanceDraft.layoutVersion!==2)appearanceDraft.layout=structuredClone(layoutDefaults);appearanceDraft.layoutVersion=2;appearanceDraft.modules=cleanModules(appearanceDraft.modules);appearanceDraft.layout=settleLayout(appearanceDraft.layout||layoutDefaults);
 if(appearanceSheet){appearanceSheet.close();appearanceSheet.remove();appearanceSheet=null;}
 layoutEditing=true;layoutHistory=[];S.tab='play';render();document.documentElement.dataset.layoutEditing='true';
 for(const element of document.querySelectorAll('#side,nav')){layoutPreviousInert.push([element,element.inert]);element.inert=true;}
 const dock=document.createElement('div');dock.className='layout-dock';dock.setAttribute('role','region');dock.setAttribute('aria-label','Редактирование расположения');layoutDock=dock;
 dock.innerHTML=`<button class="layout-add" data-layout-action="add">${moduleIcon('plus')}<span>Добавить модуль</span></button><span class="layout-editor-title" role="status">Экран Nova</span><div class="layout-tools"><button class="layout-tool" data-layout-action="undo" disabled aria-label="Отменить действие">${moduleIcon('undo')}</button><button class="layout-tool" data-layout-action="menu" aria-label="Ещё" aria-expanded="false">${moduleIcon('more')}</button><button class="layout-done" data-layout-action="save">Готово</button></div><div class="layout-menu" hidden><button data-layout-action="reset">Стандартный экран</button><button data-layout-action="cancel">Выйти без сохранения</button></div>`;
 document.querySelector('main').append(dock);
 dock.addEventListener('click',async event=>{
  if(appearanceSaving)return;const action=event.target.closest('[data-layout-action]')?.dataset.layoutAction;
  if(action==='add'){openModuleGallery();return;}
  if(action==='menu'){const menu=dock.querySelector('.layout-menu');menu.hidden=!menu.hidden;event.target.closest('button').setAttribute('aria-expanded',String(!menu.hidden));return;}
  if(action==='cancel'){endLayoutEdit();return;}
  if(action==='undo'){const previous=layoutHistory.pop();if(previous){appearanceDraft=previous;render();}dock.querySelector('[data-layout-action=undo]').disabled=!layoutHistory.length;return;}
  if(action==='reset'){layoutRemember(appearanceDraft);appearanceDraft.layout=structuredClone(layoutDefaults);appearanceDraft.modules=cleanModules(null);appearanceDraft.character=true;appearanceDraft.stats=true;render();return;}
  if(action==='save'){
   appearanceSaving=true;dock.querySelectorAll('button').forEach(button=>button.disabled=true);const before=structuredClone(S.db.settings);
   appearanceDraft.layout=settleLayout(appearanceDraft.layout);S.db.settings.appearance=cleanAppearance(appearanceDraft);S.db.settings.theme=appearanceDraft.theme;
   try{await persist();appearanceSaving=false;endLayoutEdit(true);toast('Расположение сохранено');}
   catch(error){S.db.settings=before;appearanceSaving=false;dock.querySelectorAll('button').forEach(button=>button.disabled=false);dock.querySelector('[role=status]').textContent=error.message;}
  }
 });
 applyLayout();
}
document.addEventListener('keydown',event=>{if(moduleGallery?.open)return;if(layoutEditing&&event.key==='Escape'){event.preventDefault();endLayoutEdit();}});

window.addEventListener('blur',()=>{if(layoutDrag)finishLayoutDrag();});

let moduleGallery=null,moduleGallerySize='standard';
function setModuleEnabled(key,enabled){
 if(!moduleCatalog[key]||!appearanceDraft)return;layoutRemember(appearanceDraft);appearanceDraft.modules=cleanModules(appearanceDraft.modules);appearanceDraft.modules[key]=enabled;if(key==='character'||key==='stats')appearanceDraft[key]=enabled;
 if(enabled){const size={...moduleCatalog[key].size};if(moduleGallerySize==='compact'){size.w=Math.max(key==='character'?3:4,size.w-2);}if(moduleGallerySize==='wide')size.w=Math.min(12,size.w+3);const layout=cleanLayout(appearanceDraft.layout);let position;
  for(let y=0;y<128&&!position;y++)for(let x=0;x<=12-size.w;x++){const rect={x,y,...size};if(Object.keys(layout).filter(other=>other!==key&&layoutVisible(other)).every(other=>!layoutIntersects(rect,layout[other]))){position=rect;break;}}
  layout[key]=position||{x:0,y:128,...size};appearanceDraft.layout=settleLayout(layout,key);
 }
 render();if(moduleGallery)renderModuleGallery(moduleGallery.querySelector('input')?.value||'');
}
function galleryPreview(key){const value={info:esc(cur()?.name||'Моя сборка'),actions:'Играть',character:'Мой персонаж',stats:`${S.mods.length} модов`,favorite:esc(S.db.servers.find(item=>item.favorite)?.name||'Твой сервер'),history:'Недавние игры',quick:'Миры · Папка · Моды',memory:`${Number(S.db.settings.ram)||4} ГБ`}[key];return `<div class="module-preview preview-${key}">${moduleIcon(key)}<strong>${value}</strong></div>`;}
function renderModuleGallery(query=''){if(!moduleGallery)return;const grid=moduleGallery.querySelector('.module-gallery-grid');const items=Object.entries(moduleCatalog).filter(([key,item])=>(item.name+' '+item.description).toLowerCase().includes(query.toLowerCase())).sort((a,b)=>Number(layoutVisible(a[0]))-Number(layoutVisible(b[0])));grid.innerHTML=items.map(([key,item])=>`<button class="module-choice" data-module-add="${key}" ${layoutVisible(key)?'disabled':''}>${galleryPreview(key)}<span class="module-choice-copy"><b>${item.name}</b><span>${item.description}</span></span><span class="module-choice-add">${moduleIcon(layoutVisible(key)?'check':'plus')}</span><small>${layoutVisible(key)?'На экране':'Добавить'}</small></button>`).join('')||'<p class="tool-status">Модули не найдены</p>';}
function closeModuleGallery(){if(moduleGallery){moduleGallery.close();moduleGallery.remove();moduleGallery=null;}}
function openModuleGallery(){if(!layoutEditing)return;if(moduleGallery)return;const dialog=document.createElement('dialog');dialog.className='module-gallery';dialog.setAttribute('aria-label','Галерея модулей');moduleGallery=dialog;dialog.innerHTML=`<div class="gallery-handle"></div><header><div><span class="appearance-overline">СОБЕРИ СВОЙ ЭКРАН</span><h2>Добавить модуль</h2></div><button class="appearance-close" data-gallery-close aria-label="Закрыть">×</button></header><input type="search" placeholder="Поиск модулей" aria-label="Поиск модулей"><div class="gallery-sizes" role="group" aria-label="Размер модуля">${[['compact','Маленький'],['standard','Средний'],['wide','Большой']].map(([key,name])=>`<button data-gallery-size="${key}" aria-pressed="${key===moduleGallerySize}">${name}</button>`).join('')}</div><div class="module-gallery-grid"></div>`;document.body.append(dialog);renderModuleGallery();dialog.showModal();if(!reducedMotion())dialog.animate([{opacity:0,transform:'translateY(32px) scale(.98)'},{opacity:1,transform:'none'}],{duration:380,easing:'cubic-bezier(.16,1,.3,1)'});dialog.addEventListener('cancel',event=>{event.preventDefault();closeModuleGallery();});dialog.querySelector('input').oninput=event=>renderModuleGallery(event.target.value);dialog.onclick=event=>{const button=event.target.closest('button');if(!button)return;if(button.hasAttribute('data-gallery-close')){closeModuleGallery();return;}if(button.dataset.gallerySize){moduleGallerySize=button.dataset.gallerySize;dialog.querySelectorAll('[data-gallery-size]').forEach(item=>item.setAttribute('aria-pressed',String(item===button)));return;}if(button.dataset.moduleAdd){setModuleEnabled(button.dataset.moduleAdd,true);closeModuleGallery();}};}
