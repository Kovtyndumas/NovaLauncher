(()=>{
 let active=false,finishCurrent;
 const motion=()=>!matchMedia('(prefers-reduced-motion: reduce)').matches;
 const get=()=>document.getElementById('startupOverlay');
 async function play({first=false,enabled=true,force=false}={}){
  if(active)return;const overlay=get();if(!overlay)return;active=true;
  document.documentElement.classList.add('startup-lock');document.body.setAttribute('aria-busy','true');
  const content=[...document.querySelectorAll('#side,main')],previousInert=content.map(el=>el.inert);content.forEach(el=>el.inert=true);
  overlay.hidden=false;overlay.dataset.mode=first?'first':'returning';overlay.classList.remove('is-playing');
  overlay.querySelector('.startup-caption').textContent=first?'Твой мир начинается здесь':'С возвращением';
  overlay.querySelector('.startup-tag').textContent=first?'ДОБРО ПОЖАЛОВАТЬ':'MINECRAFT LAUNCHER';
  const button=overlay.querySelector('button');button.textContent='Пропустить';button.onclick=()=>finishCurrent?.();button.focus({preventScroll:true});
  // Restart the CSS sequence when previewing the first opening from Settings.
  const animate=(enabled||force)&&document.documentElement.dataset.lowPower!=='true'&&document.documentElement.dataset.reduceMotion!=='true'&&motion(),hold=animate?(first?3200:820):first?260:0;
  void overlay.offsetWidth;if(animate)overlay.classList.add('is-playing');
  return new Promise(resolve=>{
   let finished=false,cleaned=false,revealTimer;const timer=setTimeout(finish,hold);
   function key(e){if(['Escape','Enter',' '].includes(e.key)){e.preventDefault();finish();}}
   document.addEventListener('keydown',key);
   function finish(){
    if(finished)return;finished=true;clearTimeout(timer);document.removeEventListener('keydown',key);finishCurrent=null;
    document.documentElement.classList.remove('startup-lock');document.body.removeAttribute('aria-busy');content.forEach((el,index)=>el.inert=previousInert[index]);
    const duration=animate?first?650:420:0;
    if(duration){
     // A minimized or occluded window can defer animation completion indefinitely.
     revealTimer=setTimeout(cleanup,duration+100);
     document.querySelectorAll('#side,main').forEach((el,index)=>el.animate([{opacity:0,transform:index?'translateY(14px) scale(.985)':'translateX(-12px)',filter:'blur(4px)'},{opacity:1,transform:'none',filter:'blur(0px)'}],{duration:duration+100,delay:index?60:0,easing:'cubic-bezier(.16,1,.3,1)',fill:'backwards'}));
     overlay.animate([{opacity:1,transform:'scale(1)'},{opacity:0,transform:'scale(1.045)'}],{duration,easing:'cubic-bezier(.4,0,.2,1)'}).finished.catch(()=>{}).then(cleanup);
    }else cleanup();
   }
   function cleanup(){if(cleaned)return;cleaned=true;clearTimeout(revealTimer);overlay.hidden=true;overlay.classList.remove('is-playing');overlay.getAnimations({subtree:true}).forEach(a=>a.cancel());active=false;resolve();}
   finishCurrent=finish;
  });
 }
 function fail(message){const overlay=get();if(!overlay)return;overlay.hidden=false;overlay.dataset.mode='error';overlay.querySelector('.startup-tag').textContent='НЕ УДАЛОСЬ ОТКРЫТЬ NOVA';overlay.querySelector('.startup-caption').textContent=String(message);overlay.classList.add('is-playing');const button=overlay.querySelector('button');button.textContent='Повторить';button.onclick=()=>location.reload();document.body.removeAttribute('aria-busy');}
 window.NovaStartup={play,fail,skip:()=>finishCurrent?.(),get active(){return active;}};
})();
