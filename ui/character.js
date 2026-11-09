/* Nova character controller. Angles in radians, time in seconds. */
window.NovaCharacter = class {
  constructor(viewer, canvas, hint, running, settings = {}, memory = {}) {
    this.settings=settings;
    this.memory=memory;
    this.lastHover=-100;
    this.viewer = viewer;
    this.hint = hint;
    this.mode = running ? 'run' : settings.characterAuto===false||memory.greeted?'idle':'wave';
    if(!running)memory.greeted=true;
    this.transition=null;
    this.time = 0;
    this.clock = 0;
    this.running = running;
    this.cursor = {x:0,y:0,active:false};
    this.dragging = false;
    this.lastInteraction = -10;
    this.lastAction = null;
    this.duration = running||settings.characterAuto===false ? Infinity : this.mode==='idle'?8:2.8;
    this.reduced = matchMedia('(prefers-reduced-motion: reduce)').matches;
    this.parts = ['head', 'body', 'leftArm', 'rightArm', 'leftLeg', 'rightLeg'];
    this.motion=this.createMotion(this.mode);
    this.base = Object.fromEntries(this.parts.map(k => [k, viewer.playerObject.skin[k].position.clone()]));
    viewer.animation = new skinview3d.FunctionAnimation((player, _, delta) => this.frame(player, delta));
    viewer.playerWrapper.rotation.y = 0;
    this.abort = new AbortController();
    const on=(target,type,fn)=>target.addEventListener(type,fn,{signal:this.abort.signal});
    let down=null;
    on(document,'pointermove',e=>{
      // Track across the whole window, relative to the character's centre.
      const r=canvas.getBoundingClientRect();
      this.cursor.x=Math.max(-1,Math.min(1,(e.clientX-r.left-r.width/2)/(r.width*1.4)));
      this.cursor.y=Math.max(-1,Math.min(1,(e.clientY-r.top-r.height*.32)/(r.height*.9)));
      this.cursor.active=true;
      if(down&&Math.hypot(e.clientX-down.x,e.clientY-down.y)>6)this.dragging=true;
    });
    on(document,'pointerout',e=>{if(!e.relatedTarget)this.cursor.active=false;});
    on(window,'blur',()=>{this.cursor.active=false;down=null;this.dragging=false;});
    on(canvas,'pointerenter',()=>{if(this.mode==='idle'&&!this.running&&!this.reduced&&this.settings.characterReact!==false&&this.clock-this.lastHover>12){this.lastHover=this.clock;this.set('wave',2.8);}});
    on(canvas,'pointerdown',e=>{if(e.button===0){down={x:e.clientX,y:e.clientY};this.dragging=false;}});
    on(document,'pointerup',e=>{
      const click=down&&!this.dragging&&Math.hypot(e.clientX-down.x,e.clientY-down.y)<=6;
      down=null;this.dragging=false;
      if(click)this.react();
    });
    on(document,'pointercancel',()=>{down=null;this.dragging=false;});
    on(canvas,'keydown',e=>{
      if(e.code==='Enter'||e.code==='Space'){e.preventDefault();if(!e.repeat)this.react();}
    });
    canvas.tabIndex=0;
    canvas.setAttribute('role','button');
    canvas.setAttribute('aria-label','Персонаж следит за курсором. Нажмите для реакции, перетащите для вращения');
    canvas.style.touchAction='none';
    this.label();
  }
  label() {
    const names={sneak:'Крадётся',look:'Осматривается',cool:'Поза',victory:'Победа',sad:'Грустит',dance:'Танцует',idle:'Отдыхает',walk:'Идёт',run:'Бежит',wave:'Привет!',jump:'Прыжок',punch:'Удар',crouch:'Приседает'};
    if(this.hint)this.hint.textContent=names[this.mode]+(this.settings.characterReact===false?' · перетащи для вращения':' · нажми или перетащи');
  }
  createMotion(mode){
    if(mode==='run'){
      // Compact Minecraft gait: hips stay centred and opposite limbs share a cycle.
      const motion=new skinview3d.FunctionAnimation((p,t)=>{
        const phase=t*Math.PI*2*1.85,stride=Math.sin(phase),bounce=1-Math.cos(phase*2);
        p.rotation.x=0.035;p.rotation.z=0;
        p.position.y=bounce*0.085;
        p.skin.body.rotation.x=0.075;
        p.skin.body.rotation.y=stride*0.025;
        p.skin.head.rotation.x=-0.035;
        p.skin.head.rotation.y=-stride*0.012;
        p.skin.leftLeg.rotation.x=-stride*0.68;
        p.skin.rightLeg.rotation.x=stride*0.68;
        p.skin.leftLeg.rotation.z=0.015;p.skin.rightLeg.rotation.z=-0.015;
        p.skin.leftArm.rotation.x=stride*0.54-0.06;
        p.skin.rightArm.rotation.x=-stride*0.54-0.06;
        p.skin.leftArm.rotation.z=0.07;p.skin.rightArm.rotation.z=-0.07;
        p.cape.rotation.x=0.32+bounce*0.025;
      });
      motion.controlsLegs=true;return motion;
    }

    if(mode==='nod'||mode==='jump'){
      const idle=MillidaAnimations.create('idle');idle.suppressAutoLook=true;
      return new skinview3d.FunctionAnimation((p,t,dt)=>{
        idle.update(p,dt);
        if(mode==='nod'){
          const envelope=Math.sin(Math.PI*Math.min(1,t/1.8));
          p.skin.head.rotation.x+=Math.sin(t*7)*.32*envelope;
          p.skin.body.rotation.z+=Math.sin(t*3)*.025*envelope;
        }else{
          const lift=Math.max(0,Math.min(1,(t-.2)/.8)),air=Math.sin(Math.PI*lift);
          const prepare=t<.2?Math.sin(t/.2*Math.PI)*.8:0;
          const land=t>1&&t<1.3?Math.sin((t-1)/.3*Math.PI)*.5:0;
          p.position.y=air*3.2-prepare-land;
          p.skin.leftArm.rotation.z+=air*.65;p.skin.rightArm.rotation.z-=air*.65;
          p.skin.leftLeg.rotation.x-=air*.15;p.skin.rightLeg.rotation.x+=air*.18;
          p.skin.body.rotation.x+=(prepare+land)*.12;
        }
      });
    }
    const motion=MillidaAnimations.create(mode);motion.speed=mode==='run'?.9:mode==='dance'?.78:.9;return motion;}
  objects(p){return [p,...this.parts.map(k=>p.skin[k]),p.cape];}
  capture(p){return this.objects(p).map(o=>({position:o.position.clone(),quaternion:o.quaternion.clone()}));}
  set(mode, duration){
    this.transition={from:this.capture(this.viewer.playerObject),time:0,duration:.55};
    this.mode=mode;this.time=0;this.duration=duration;
    this.motion=this.createMotion(mode);this.label();
  }
  react(){
    if(this.settings.characterReact===false||this.running||this.reduced||this.clock-this.lastInteraction<.7)return;
    this.lastInteraction=this.clock;
    const actions=['nod','jump','wave','dance','victory','cool'];
    let index=this.memory.reactionIndex||0;
    if(actions[index%actions.length]===this.mode)index++;
    const action=actions[index%actions.length];this.memory.reactionIndex=index+1;
    this.set(action,action==='jump'?1.6:action==='nod'?1.9:action==='dance'?4:2.8);
  }

  gaze(p){
    if(this.settings.characterLook===false||!this.cursor.active||this.dragging||this.motion.blocksCursorLook)return;
    // Convert camera-facing gaze into the model's local space, even after orbiting.
    const V=p.position.constructor,Q=p.quaternion.constructor;
    p.updateWorldMatrix(true,true);
    const head=p.skin.head.getWorldPosition(new V());
    const eye=this.viewer.camera.getWorldPosition(new V());
    const direction=eye.sub(head);
    const distance=direction.length();
    const offset=new V(this.cursor.x*distance*.65,-this.cursor.y*distance*.5,0);
    offset.applyQuaternion(this.viewer.camera.getWorldQuaternion(new Q()));
    direction.add(offset).applyQuaternion(p.getWorldQuaternion(new Q()).invert());
    const yaw=Math.max(-.65,Math.min(.65,Math.atan2(direction.x,direction.z)));
    const pitch=Math.max(-.28,Math.min(.28,-Math.atan2(direction.y,Math.hypot(direction.x,direction.z))));
    // Preserve expressive gestures; idle yields fully to the pointer.
    const weight=this.mode==='idle'?1:this.mode==='run'?.12:this.mode==='nod'?.1:this.mode==='cool'?.45:.2;
    p.skin.head.rotation.y=p.skin.head.rotation.y*(1-weight)+yaw*weight;
    p.skin.head.rotation.x=p.skin.head.rotation.x*(1-weight)+pitch*weight;
    // Keep the torso facing forward; the head follows the pointer independently.
  }
  next(){
    if(this.running)return;
    if(this.mode!=='idle'){
      const pause={calm:[12,8],normal:[6,5],active:[3,3]}[this.settings.characterFrequency]||[6,5];
      this.set('idle',this.settings.characterAuto===false?Infinity:pause[0]+Math.random()*pause[1]);
      return;
    }
    if(this.settings.characterAuto===false)return;
    const pool=this.settings.characterFrequency==='calm'?['look','cool','wave','nod']:['look','wave','nod','cool','dance','jump'];
    const actions=pool.filter(x=>x!==this.lastAction);
    const action=actions[Math.floor(Math.random()*actions.length)];
    this.lastAction=action;
    this.set(action,action==='wave'?2.8:action==='jump'?1.6:action==='nod'?1.9:action==='dance'?4:3.5+Math.random()*1.5);
  }
  frame(p, delta) {
    if(document.hidden)return;
    const dt=Math.min(Math.max(delta,0),.05);
    this.time+=dt;this.clock+=dt;
    if(this.reduced)return;
    const objects=this.objects(p),before=this.capture(p);
    // Millida evaluates an absolute pose; restore Nova's own rig pivot positions.
    p.position.set(0,0,0);p.rotation.set(0,0,0);
    for(const k of this.parts){p.skin[k].position.copy(this.base[k]);p.skin[k].rotation.set(0,0,0);}
    p.cape.rotation.set(.1885,Math.PI,0);
    if(this.mode==='idle')this.motion.suppressAutoLook=this.settings.characterLook!==false&&this.cursor.active&&!this.dragging;
    this.motion.update(p,this.dragging?0:dt);
    if(this.mode==='idle')p.skin.body.rotation.y=0;
    this.gaze(p);
    // Use quaternion crossfades so shoulders take the short route between poses.
    let from=before,alpha=1-Math.exp(-(this.mode==='run'?22:10)*dt);
    if(this.transition){
      this.transition.time+=dt;
      const t=Math.min(1,this.transition.time/this.transition.duration);
      alpha=t*t*t*(t*(t*6-15)+10);
      from=this.transition.from;
      if(t===1)this.transition=null;
    }
    objects.forEach((o,i)=>{
      o.position.lerpVectors(from[i].position,o.position.clone(),alpha);
      o.quaternion.slerpQuaternions(from[i].quaternion,o.quaternion.clone(),alpha);
    });
    if(this.time>=this.duration)this.next();
  }

  dispose(){this.abort.abort();}
};
