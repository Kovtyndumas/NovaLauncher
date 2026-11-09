const layoutDefaults={info:{x:0,y:0,w:7,h:3},actions:{x:0,y:3,w:7,h:4},stats:{x:0,y:7,w:7,h:3},character:{x:7,y:0,w:5,h:10}};
const layoutNames={info:'Название сборки',actions:'Кнопки запуска',stats:'Статистика',character:'Персонаж'};
let layoutEditing=false,layoutDock=null,layoutHistory=[],layoutDrag=null,layoutPreviousInert=[];
function cleanLayout(value){const result={};for(const [key,defaults]of Object.entries(layoutDefaults)){const source=value?.[key]||{};const integer=(name,min,max)=>Math.max(min,Math.min(max,Number.isFinite(Number(source[name]))?Math.round(Number(source[name])):defaults[name]));const w=integer('w',key==='character'?3:4,12),h=integer('h',key==='character'?6:2,18);result[key]={x:integer('x',0,12-w),y:integer('y',0,128),w,h};}return result;}
function layoutVisible(key){const value=appearanceValue();return key!=='character'&&key!=='stats'||value[key];}
function layoutIntersects(a,b){return a.x<b.x+b.w&&a.x+a.w>b.x&&a.y<b.y+b.h&&a.y+a.h>b.y;}
function settleLayout(source,first){const result=cleanLayout(source),placed=[];const keys=Object.keys(result).filter(layoutVisible).sort((a,b)=>a===first?-1:b===first?1:result[a].y-result[b].y);for(const key of keys){const rect=result[key];let overlapping;while((overlapping=placed.find(other=>layoutIntersects(rect,result[other]))))rect.y=result[overlapping].y+result[overlapping].h;placed.push(key);}return result;}
function applyLayout(){
 const hero=document.querySelector('.hero2'),layout=appearanceValue().layout;
 document.documentElement.dataset.customLayout=layout||layoutEditing?'true':'false';
 if(!hero)return;
 const positions=settleLayout(layout||layoutDefaults);
 for(const element of hero.querySelectorAll('[data-layout-block]')){
  const key=element.dataset.layoutBlock,position=positions[key];
  if(layout||layoutEditing){element.style.gridColumn=`${position.x+1} / span ${position.w}`;element.style.gridRow=`${position.y+1} / span ${position.h}`;}
  else{element.style.removeProperty('grid-column');element.style.removeProperty('grid-row');}
  if(layoutEditing&&!element.querySelector('.layout-grab')){
   const grab=document.createElement('button');grab.className='layout-grab';grab.type='button';grab.dataset.layoutHandle=key;grab.textContent='⠿ '+layoutNames[key];grab.setAttribute('aria-label','Переместить: '+layoutNames[key]);grab.title='Перетащи или используй стрелки. Shift + стрелки — размер.';
   const resize=document.createElement('button');resize.className='layout-resize';resize.type='button';resize.dataset.layoutHandle=key;resize.dataset.resize='true';resize.textContent='↘';resize.setAttribute('aria-label','Размер: '+layoutNames[key]);
   element.append(grab,resize);for(const handle of [grab,resize]){handle.addEventListener('pointerdown',startLayoutDrag);handle.addEventListener('pointermove',moveLayoutDrag);handle.addEventListener('pointerup',finishLayoutDrag);handle.addEventListener('pointercancel',cancelLayoutDrag);handle.addEventListener('lostpointercapture',finishLayoutDrag);handle.addEventListener('keydown',layoutKey);}
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
 const message=layoutDock?.querySelector('[role=status]');if(message)message.textContent=`${layoutNames[drag.key]} · колонка ${rect.x+1}, строка ${rect.y+1} · ${rect.w} × ${rect.h}`;
}
function finishLayoutDrag(event){const drag=layoutDrag;if(!drag)return;layoutDrag=null;drag.handle.closest('[data-layout-block]')?.classList.remove('layout-dragging');if(drag.handle.hasPointerCapture(drag.pointer))drag.handle.releasePointerCapture(drag.pointer);if(JSON.stringify(drag.before.layout)!==JSON.stringify(appearanceDraft.layout))layoutRemember(drag.before);}
function cancelLayoutDrag(){const drag=layoutDrag;if(!drag)return;appearanceDraft=drag.before;finishLayoutDrag();applyLayout();}
function layoutKey(event){if(!['ArrowLeft','ArrowRight','ArrowUp','ArrowDown'].includes(event.key))return;event.preventDefault();event.stopPropagation();const key=event.currentTarget.dataset.layoutHandle,before=structuredClone(appearanceDraft),source=cleanLayout(appearanceDraft.layout),rect=source[key],resize=event.shiftKey||event.currentTarget.dataset.resize==='true';if(event.key==='ArrowLeft')rect[resize?'w':'x']--;if(event.key==='ArrowRight')rect[resize?'w':'x']++;if(event.key==='ArrowUp')rect[resize?'h':'y']--;if(event.key==='ArrowDown')rect[resize?'h':'y']++;rect.y=Math.max(0,Math.min(60,rect.y));appearanceDraft.layout=settleLayout(cleanLayout(source),key);layoutRemember(before);applyLayout();}
function endLayoutEdit(save=false){
 if(appearanceSaving)return;layoutEditing=false;cancelLayoutDrag();document.documentElement.dataset.layoutEditing='false';layoutDock?.remove();layoutDock=null;
 for(const [element,value]of layoutPreviousInert)element.inert=value;layoutPreviousInert=[];appearanceDraft=null;render();
}
function startLayoutEdit(){
 if(layoutEditing)return;if(!cur()){toast('Сначала создайте сборку для предпросмотра интерфейса');return;}
 appearanceDraft=appearanceDraft||{...cleanAppearance(S.db.settings.appearance),theme:S.db.settings.theme||'black'};
 appearanceDraft.layout=settleLayout(appearanceDraft.layout||layoutDefaults);
 if(appearanceSheet){appearanceSheet.close();appearanceSheet.remove();appearanceSheet=null;}
 layoutEditing=true;layoutHistory=[];S.tab='play';render();document.documentElement.dataset.layoutEditing='true';
 for(const element of document.querySelectorAll('#side,nav')){layoutPreviousInert.push([element,element.inert]);element.inert=true;}
 const dock=document.createElement('div');dock.className='layout-dock';dock.setAttribute('role','region');dock.setAttribute('aria-label','Редактирование расположения');layoutDock=dock;
 dock.innerHTML='<div><b>Редактирование дизайна</b><p role="status">Тяни за название блока. За угол ↘ — меняй размер.</p></div><div class="layout-tools"><button class="b" data-layout-action="undo" disabled>↶ Отменить</button><button class="b" data-layout-action="reset">Сбросить</button><button class="b" data-layout-action="cancel">Выйти без сохранения</button><button class="b appearance-save" data-layout-action="save">Сохранить</button></div>';
 document.body.append(dock);
 dock.addEventListener('click',async event=>{
  if(appearanceSaving)return;const action=event.target.closest('[data-layout-action]')?.dataset.layoutAction;
  if(action==='cancel'){endLayoutEdit();return;}
  if(action==='undo'){const previous=layoutHistory.pop();if(previous){appearanceDraft=previous;render();}dock.querySelector('[data-layout-action=undo]').disabled=!layoutHistory.length;return;}
  if(action==='reset'){layoutRemember(appearanceDraft);appearanceDraft.layout=structuredClone(layoutDefaults);render();return;}
  if(action==='save'){
   appearanceSaving=true;dock.querySelectorAll('button').forEach(button=>button.disabled=true);const before=structuredClone(S.db.settings);
   appearanceDraft.layout=settleLayout(appearanceDraft.layout);S.db.settings.appearance=cleanAppearance(appearanceDraft);S.db.settings.theme=appearanceDraft.theme;
   try{await persist();appearanceSaving=false;endLayoutEdit(true);toast('Расположение сохранено');}
   catch(error){S.db.settings=before;appearanceSaving=false;dock.querySelectorAll('button').forEach(button=>button.disabled=false);dock.querySelector('[role=status]').textContent=error.message;}
  }
 });
 applyLayout();
}
document.addEventListener('keydown',event=>{if(layoutEditing&&event.key==='Escape'){event.preventDefault();endLayoutEdit();}});

window.addEventListener('blur',()=>{if(layoutDrag)finishLayoutDrag();});
