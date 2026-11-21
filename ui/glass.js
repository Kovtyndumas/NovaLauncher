/* Optional, restrained panel material. The application window always remains opaque. */
window.NovaGlass=(()=>{
 const preference=matchMedia('(prefers-reduced-transparency: reduce)');
 function update(){
  const enabled=(appearanceDraft?.glassEffects??S.db?.settings.glassEffects??true)&&!preference.matches;
  document.documentElement.dataset.glass=enabled?'on':'off';
  return Promise.resolve(enabled);
 }
 function migrate(settings){
  if(settings.theme!=='glass')return false;
  settings.theme='light';if(typeof settings.glassEffects!=='boolean')settings.glassEffects=true;
  return true;
 }
 function refresh(){update();}
 addEventListener('DOMContentLoaded',()=>{update();preference.addEventListener('change',update);});
 return {refresh,update,migrate};
})();
