/* Explicit order wins over the initial favourites-first order. IDs survive renaming. */
function orderedInstances(){
 const order=Array.isArray(S.db.settings.instanceOrder)?S.db.settings.instanceOrder:[];
 const rank=new Map(order.map((id,index)=>[id,index]));
 return [...S.db.instances].sort((a,b)=>{const ar=rank.get(a.id),br=rank.get(b.id);if(ar!==undefined||br!==undefined)return (ar??Infinity)-(br??Infinity);return Number(!!b.favorite)-Number(!!a.favorite);});
}
async function moveInstance(source,target,after=false){
 if(source===target)return;const order=orderedInstances().map(x=>x.id);if(!order.includes(source)||!order.includes(target))return;
 const old=S.db.settings.instanceOrder;order.splice(order.indexOf(source),1);order.splice(order.indexOf(target)+(after?1:0),0,source);S.db.settings.instanceOrder=order;
 try{await persist();render();document.querySelector(`[data-sel="${CSS.escape(source)}"]`)?.focus();}catch(error){S.db.settings.instanceOrder=old;render();toast('Не удалось сохранить порядок: '+error.message);}
}
let instanceDrag=null,instanceSuppressClick=false;
function clearInstanceDrop(){document.querySelectorAll('.instance-drop-before,.instance-drop-after').forEach(x=>x.classList.remove('instance-drop-before','instance-drop-after'));}
document.addEventListener('pointerdown',event=>{const item=event.target.closest('#side [data-sel]');if(!item||layoutEditing||event.button!==0)return;instanceDrag={id:item.dataset.sel,item,x:event.clientX,y:event.clientY,pointer:event.pointerId,active:false,target:null,after:false};});
document.addEventListener('pointermove',event=>{const drag=instanceDrag;if(!drag||event.pointerId!==drag.pointer)return;if(!drag.active&&Math.hypot(event.clientX-drag.x,event.clientY-drag.y)<6)return;if(!drag.active){drag.active=true;drag.item.setPointerCapture(event.pointerId);drag.item.classList.add('instance-dragging');}event.preventDefault();const item=document.elementFromPoint(event.clientX,event.clientY)?.closest('#side [data-sel]');clearInstanceDrop();drag.target=null;if(item&&item.dataset.sel!==drag.id){drag.target=item.dataset.sel;drag.after=event.clientY>item.getBoundingClientRect().top+item.offsetHeight/2;item.classList.add(drag.after?'instance-drop-after':'instance-drop-before');}},{passive:false});
function finishInstanceDrag(event){const drag=instanceDrag;if(!drag||event.pointerId!==drag.pointer)return;instanceDrag=null;clearInstanceDrop();drag.item.classList.remove('instance-dragging');if(drag.item.hasPointerCapture(drag.pointer))drag.item.releasePointerCapture(drag.pointer);if(drag.active){instanceSuppressClick=true;setTimeout(()=>{instanceSuppressClick=false;},0);if(event.type==='pointerup'&&drag.target)moveInstance(drag.id,drag.target,drag.after);}}
document.addEventListener('pointerup',finishInstanceDrag);document.addEventListener('pointercancel',finishInstanceDrag);document.addEventListener('lostpointercapture',finishInstanceDrag);
document.addEventListener('click',event=>{if(instanceSuppressClick){event.preventDefault();event.stopImmediatePropagation();instanceSuppressClick=false;}},true);
document.addEventListener('dragstart',event=>{if(event.target.closest('#side [data-sel]'))event.preventDefault();});
document.addEventListener('keydown',event=>{const item=event.target.closest('#side [data-sel]');if(!item||!event.altKey||!['ArrowUp','ArrowDown'].includes(event.key))return;event.preventDefault();const ids=orderedInstances().map(x=>x.id),step=event.key==='ArrowUp'?-1:1,target=ids[ids.indexOf(item.dataset.sel)+step];if(target)moveInstance(item.dataset.sel,target,step>0);});
