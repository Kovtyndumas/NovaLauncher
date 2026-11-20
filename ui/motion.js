/* Brief, interruptible feedback inspired by Apple's Motion HIG; no decorative card tilt. */
window.NovaMotion=class{
 constructor(){
  this.active=null;this.reveals=new Set();this.knownInstances=new Set();this.pendingReveal=new Set();this.revealFrame=null;this.presses=new Map();
  const reduced=()=>matchMedia('(prefers-reduced-motion: reduce)').matches;
  document.addEventListener('pointerdown',event=>{if(reduced()||event.button!==0)return;const button=event.target.closest('.play,.b,.theme-choice,.seg button,.appearance-launch,.experience-tools-trigger');if(!button||button.disabled)return;this.presses.get(button)?.cancel();const animation=button.animate([{scale:'1'},{scale:'.975'}],{duration:90,easing:'ease-out',fill:'forwards'});this.presses.set(button,animation);});
  const release=()=>{for(const [button,animation]of this.presses){animation.cancel();if(button.isConnected&&!reduced())button.animate([{scale:'.975'},{scale:'1.005',offset:.7},{scale:'1'}],{duration:240,easing:'cubic-bezier(.2,.8,.2,1)'});}this.presses.clear();};
  document.addEventListener('pointerup',release);document.addEventListener('pointercancel',release);addEventListener('blur',release);
  this.preference=matchMedia('(prefers-reduced-motion: reduce)');this.preference.addEventListener('change',()=>{this.reset();release();for(const animation of this.reveals)animation.cancel();});
  this.observer=new MutationObserver(records=>{
   for(const record of records)for(const node of record.addedNodes){if(node.nodeType!==1||node.closest('#onb,.startup-overlay'))continue;const selector='.grid>.card,.mod-row,.backup-card';if(node.matches(selector))this.pendingReveal.add(node);for(const child of node.querySelectorAll(selector))this.pendingReveal.add(child);}
   if(this.revealFrame!==null)return;
   this.revealFrame=requestAnimationFrame(()=>{
    this.revealFrame=null;const items=[...document.querySelectorAll('#side [data-sel]')];for(const item of items)if(!this.knownInstances.has(item.dataset.sel))this.pendingReveal.add(item);this.knownInstances=new Set(items.map(item=>item.dataset.sel));const nodes=[...this.pendingReveal].filter(node=>node.isConnected);this.pendingReveal.clear();if(reduced())return;
    const glass=document.documentElement.dataset.theme==='glass';
    for(const [index,node]of nodes.slice(0,24).entries()){const animation=node.animate(glass?[{opacity:0},{opacity:1}]:[{opacity:0,transform:'translateY(6px)'},{opacity:1,transform:'none'}],{duration:260,delay:Math.min(index*16,96),easing:'cubic-bezier(.2,.8,.2,1)',fill:'backwards'});this.reveals.add(animation);animation.finished.catch(()=>{}).finally(()=>{this.reveals.delete(animation);window.NovaGlass?.refresh();});}
   });
  });this.observer.observe(document.body,{childList:true,subtree:true});
 }
 reset(){this.active=null;}
 enter(view,direction){
  if(this.preference.matches)return;
  const glass=document.documentElement.dataset.theme==='glass',nodes=[...view.querySelectorAll('.home-info>.eyebrow,.home-info>h1,.home-info>.chips,.home-actions>.acts,.hl>.stats,.grp>.row,.grid>.card,.theme-choice,.backup-card')].slice(0,16);
  for(const [index,node]of nodes.entries()){const animation=node.animate(glass?[{opacity:0},{opacity:1}]:[{opacity:0,transform:`translateY(6px)`},{opacity:1,transform:'none'}],{duration:280,delay:Math.min(index*18,108),easing:'cubic-bezier(.2,.8,.2,1)',fill:'backwards'});this.reveals.add(animation);animation.finished.catch(()=>{}).finally(()=>{this.reveals.delete(animation);window.NovaGlass?.refresh();});}
 }
};
