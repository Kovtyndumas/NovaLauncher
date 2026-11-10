/* Pointer effects are event-driven; no background animation loop. */
window.NovaMotion=class{
  constructor(){
    this.active=null;this.frameId=null;this.pointer=null;
    const reduced=()=>document.documentElement.dataset.lowPower==='true'||document.documentElement.dataset.reduceMotion==='true'||matchMedia('(prefers-reduced-motion: reduce)').matches;
    document.addEventListener('pointerover',e=>{
      if(reduced()||e.pointerType==='touch')return;
      const element=e.target.closest('.st,.card,.theme-choice,.acts .play');
      if(element===this.active?.element)return;
      this.reset();
      if(element&&!element.disabled)this.active={element,rect:element.getBoundingClientRect()};
    });
    document.addEventListener('pointermove',e=>{
      if(!this.active||reduced())return;
      const bounds=this.active.rect;if(e.clientX<bounds.left-8||e.clientX>bounds.right+8||e.clientY<bounds.top-8||e.clientY>bounds.bottom+8){this.reset();return;}
      this.pointer={x:e.clientX,y:e.clientY};
      if(this.frameId!==null)return;
      this.frameId=requestAnimationFrame(()=>{this.frameId=null;if(!this.active||!this.pointer)return;
        const {element,rect}=this.active,x=Math.max(-1,Math.min(1,(this.pointer.x-rect.left)/rect.width*2-1)),y=Math.max(-1,Math.min(1,(this.pointer.y-rect.top)/rect.height*2-1));
        if(!element.isConnected){this.reset();return;}
        element.style.setProperty('--tilt-x',-y*4+'deg');element.style.setProperty('--tilt-y',x*5+'deg');
        element.style.setProperty('--magnet-x',x*5+'px');element.style.setProperty('--magnet-y',y*4+'px');element.classList.add('pointer-motion');
      });
    });
    document.addEventListener('pointerout',e=>{if(this.active&&!this.active.element.contains(e.relatedTarget))this.reset();});
    window.addEventListener('blur',()=>this.reset());
    document.documentElement.addEventListener('pointerleave',()=>this.reset());
    document.addEventListener('pointerdown',e=>{
      if(reduced()||e.button!==0)return;
      const element=e.target.closest('.play,.theme-choice,.fav');if(!element||element.disabled)return;
      const transform=getComputedStyle(element).transform,base=transform==='none'?'':transform;
      element.animate([{transform:base+' scale(1)'},{transform:base+' scale(.93)',offset:.28},{transform:base+' scale(1.035)',offset:.68},{transform:base+' scale(1)'}],{duration:420,easing:'cubic-bezier(.2,.8,.2,1)'});
    });
    this.preference=matchMedia('(prefers-reduced-motion: reduce)');this.preference.addEventListener('change',()=>{this.reset();for(const animation of this.reveals||[])animation.cancel();});
    this.reveals=new Set();this.knownInstances=new Set();this.pendingReveal=new Set();this.revealFrame=null;
    this.observer=new MutationObserver(records=>{
      for(const record of records)for(const node of record.addedNodes){
        if(node.nodeType!==1||node.closest('#onb,.startup-overlay'))continue;
        const selector='.grid>.card,.mod-row,.backup-card';
        if(node.matches(selector))this.pendingReveal.add(node);
        for(const child of node.querySelectorAll(selector))this.pendingReveal.add(child);
      }
      if(this.revealFrame!==null)return;
      this.revealFrame=requestAnimationFrame(()=>{
        this.revealFrame=null;
        const items=[...document.querySelectorAll('#side [data-sel]')];
        for(const item of items)if(!this.knownInstances.has(item.dataset.sel))this.pendingReveal.add(item);
        this.knownInstances=new Set(items.map(item=>item.dataset.sel));
        const nodes=[...this.pendingReveal].filter(node=>node.isConnected);this.pendingReveal.clear();
        if(reduced())return;
        for(const [index,node]of nodes.slice(0,24).entries()){
          const animation=node.animate([{opacity:0,transform:'translateY(9px) scale(.985)'},{opacity:1,transform:'none'}],{duration:360,delay:Math.min(index*24,180),easing:'cubic-bezier(.16,1,.3,1)',fill:'backwards'});
          this.reveals.add(animation);animation.finished.catch(()=>{}).finally(()=>this.reveals.delete(animation));
        }
      });
    });
    this.observer.observe(document.body,{childList:true,subtree:true});
  }
  reset(){
    if(this.frameId!==null){cancelAnimationFrame(this.frameId);this.frameId=null;}
    if(this.active){const e=this.active.element;e.classList.remove('pointer-motion');for(const v of ['--tilt-x','--tilt-y','--magnet-x','--magnet-y'])e.style.removeProperty(v);}
    this.active=null;this.pointer=null;
  }
  enter(view,direction){
    this.reset();if(matchMedia('(prefers-reduced-motion: reduce)').matches)return;
    const nodes=[...view.querySelectorAll('.home-info>.eyebrow,.home-info>h1,.home-info>.chips,.home-actions>.acts,.hl>.stats,.grp>.row,.grid>.card,.theme-choice,.backup-card')].slice(0,16);
    for(const [index,node]of nodes.entries())node.animate([{opacity:0,transform:`translate(${direction*8}px,14px) scale(.98)`},{opacity:1,transform:'translate(0,0) scale(1)'}],{duration:480,delay:index*38,easing:'cubic-bezier(.16,1,.3,1)',fill:'backwards'});
    for(const count of view.querySelectorAll('[data-count]')){
      const target=Number(count.dataset.count);if(!Number.isFinite(target)||target===0)continue;
      const start=performance.now();function tick(now){if(!count.isConnected)return;const t=Math.min(1,(now-start)/650);count.textContent=Math.round(target*(1-Math.pow(1-t,3)));if(t<1)requestAnimationFrame(tick);}requestAnimationFrame(tick);
    }
  }
};
