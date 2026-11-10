const appearanceDefaults={accent:'#0a84ff',radius:18,compact:false,wallpaper:'none',character:true,stats:true,shortcuts:true,layout:null,modules:null,layoutVersion:2};
let appearanceDraft=null,appearanceSheet=null,appearanceSaving=false;
function cleanAppearance(value={}){if(!value||typeof value!=='object')value={};const result={...appearanceDefaults};if(/^#[\da-f]{6}$/i.test(value.accent||''))result.accent=value.accent.toLowerCase();if(Number.isFinite(Number(value.radius)))result.radius=Math.max(8,Math.min(28,Number(value.radius)));if(['none','ocean','violet','forest'].includes(value.wallpaper))result.wallpaper=value.wallpaper;for(const key of ['compact','character','stats','shortcuts'])if(typeof value[key]==='boolean')result[key]=value[key];Object.assign(result,cleanDesignFields(value));result.modules=cleanModules(value.modules);if(value.character===false)result.modules.character=false;if(value.stats===false)result.modules.stats=false;result.layoutVersion=value.layoutVersion===2?2:1;if(value.layout)result.layout=cleanLayout(value.layout);return result;}
function appearanceValue(){return cleanAppearance(appearanceDraft||S.db?.settings.appearance||{});}
function appearanceAccent(){return appearanceValue().accent;}
function applyAppearance(){
 const value=appearanceValue(),root=document.documentElement;root.dataset.look=appearanceDraft||S.db.settings.appearance?'custom':'standard';
 root.style.setProperty('--acc',value.accent);const rgb=value.accent.slice(1).match(/../g).map(v=>parseInt(v,16)/255).map(v=>v<=.04045?v/12.92:((v+.055)/1.055)**2.4);root.style.setProperty('--acc-fg',.2126*rgb[0]+.7152*rgb[1]+.0722*rgb[2]>.35?'#101114':'#ffffff');root.style.setProperty('--appearance-radius',value.radius+'px');root.dataset.density=value.compact?'compact':'normal';
 for(const key of ['character','stats','shortcuts'])root.dataset['show'+key[0].toUpperCase()+key.slice(1)]=String(value[key]);
 const color={ocean:'#1879ff',violet:'#9e67ff',forest:'#25a875'}[value.wallpaper];
 applyDesign();
 document.querySelector('main').style.backgroundImage=color?`linear-gradient(135deg,var(--bg) 35%,color-mix(in srgb,var(--bg) 82%,${color} 18%))`:'';
}
function appearanceControls(){const value=appearanceValue();return `<div class="appearance-head"><div><span class="appearance-overline">NOVA · ПОД ТЕБЯ</span><h2>Оформление</h2></div><button class="appearance-close" data-look="cancel" aria-label="Закрыть без сохранения">×</button></div><p class="tool-status">Меняй настройки и смотри результат сразу.</p><div class="cap">Тема</div><div class="appearance-themes">${[['black','Чёрная'],['graphite','Графит'],['light','Светлая']].map(([key,title])=>`<button class="appearance-theme ${appearanceDraft.theme===key?'selected':''}" data-look-theme="${key}" aria-pressed="${appearanceDraft.theme===key}"><span class="theme-preview ${key}"></span>${title}</button>`).join('')}</div><div class="appearance-control"><div class="cap">Акцент</div><div class="appearance-colors">${['#0a84ff','#29a6b8','#9965e8','#db6397','#39a974','#e27845'].map(color=>`<button style="--swatch:${color}" class="appearance-color" data-look-accent="${color}" aria-label="Цвет ${color}" aria-pressed="${value.accent===color}">${value.accent===color?'✓':''}</button>`).join('')}<label class="appearance-custom-color" title="Свой цвет"><span>+</span><input aria-label="Свой цвет акцента" type="color" data-look-input="accent" value="${value.accent}"></label></div></div><div class="appearance-control"><label for="appearanceRadius">Скругления <span id="appearanceRadiusValue">${value.radius}</span></label><input id="appearanceRadius" data-look-input="radius" type="range" min="8" max="28" step="1" value="${value.radius}"></div><div class="cap">Фон</div><div class="appearance-wallpapers">${[['none','Чистый'],['ocean','Океан'],['violet','Сумерки'],['forest','Лес']].map(([key,title])=>`<button class="appearance-wallpaper ${value.wallpaper===key?'selected':''}" data-look-wallpaper="${key}" aria-pressed="${value.wallpaper===key}"><span data-wallpaper="${key}"></span>${title}</button>`).join('')}</div><label class="appearance-toggle"><span>Компактный интерфейс</span><input type="checkbox" data-look-input="compact" ${value.compact?'checked':''}><i></i></label><button class="appearance-move" data-look="move">Редактировать экран <span>⠿ ↗</span></button><div class="appearance-block-header"><div class="cap">Видимость</div><button class="b" data-look="edit">Все модули</button></div><div class="appearance-blocks">${[['character','Персонаж','◈'],['stats','Статистика','▥'],['shortcuts','Быстрые ссылки','↗']].map(([key,title,icon])=>`<button class="appearance-tile ${value[key]?'selected':''}" data-look-block="${key}" aria-pressed="${value[key]}"><span class="appearance-tile-symbol">${icon}</span><span>${title}</span><i>${value[key]?'−':'+'}</i></button>`).join('')}</div><p class="tool-status">Блоки можно скрывать и возвращать. Цвет общий для всех сборок.</p>${designControls()}<p class="appearance-error" role="alert"></p><div class="appearance-footer"><button class="b" data-look="reset">Сбросить</button><button class="b appearance-save" data-look="save">Сохранить</button></div>`;}
function refreshAppearanceSheet(){appearanceSheet.innerHTML=appearanceControls();if(appearanceSheet.dataset.editing==='true'){const button=appearanceSheet.querySelector('[data-look=edit]');button.textContent='Готово';button.setAttribute('aria-pressed','true');}}
function previewAppearance(){applyTheme();applyAppearance();applyLayout();}
function closeAppearance(){if(appearanceSaving)return;const sheet=appearanceSheet;if(!sheet)return;appearanceDraft=null;appearanceSheet=null;sheet.close();sheet.remove();render();}
function openAppearance(){
 if(appearanceSheet)return;appearanceDraft={...cleanAppearance(S.db.settings.appearance),theme:S.db.settings.theme||'black'};
 const sheet=document.createElement('dialog');sheet.className='appearance-sheet';sheet.setAttribute('aria-label','Оформление Nova');appearanceSheet=sheet;refreshAppearanceSheet();document.body.append(sheet);sheet.showModal();
 if(!reducedMotion())sheet.animate([{opacity:0,transform:'translateY(-36px) scale(.96)'},{opacity:1,transform:'none'}],{duration:480,easing:'cubic-bezier(.16,1,.3,1)'});
 sheet.addEventListener('cancel',event=>{event.preventDefault();closeAppearance();});
 sheet.addEventListener('click',async event=>{
  if(appearanceSaving)return;const target=event.target.closest('button');if(!target)return;const data=target.dataset;if(await designAction(target))return;
  if(data.look==='move'){startLayoutEdit();return;}
  if(data.look==='cancel'){closeAppearance();return;}
  if(data.look==='edit'){startLayoutEdit();openModuleGallery();return;}
  if(data.look==='reset'){appearanceDraft={...appearanceDefaults,modules:cleanModules(null),theme:'black'};previewAppearance();refreshAppearanceSheet();return;}
  if(data.look==='save'){
   appearanceSaving=true;sheet.querySelectorAll('button,input').forEach(control=>control.disabled=true);const before=structuredClone(S.db.settings);
   if(appearanceDraft.layout)appearanceDraft.layout=settleLayout(appearanceDraft.layout);S.db.settings.appearance=cleanAppearance(appearanceDraft);S.db.settings.theme=appearanceDraft.theme;
   try{await persist();appearanceSaving=false;closeAppearance();toast('Оформление сохранено');}
   catch(error){S.db.settings=before;appearanceSaving=false;sheet.querySelectorAll('button,input').forEach(control=>control.disabled=false);sheet.querySelector('.appearance-error').textContent=error.message;}
   return;
  }
  if(data.lookTheme)appearanceDraft.theme=data.lookTheme;
  if(data.lookAccent)appearanceDraft.accent=data.lookAccent;
  if(data.lookWallpaper){appearanceDraft.wallpaper=data.lookWallpaper;appearanceDraft.wallpaperImage='';}
  if(data.lookBlock){appearanceDraft[data.lookBlock]=!appearanceDraft[data.lookBlock];if(data.lookBlock==='character'||data.lookBlock==='stats')appearanceDraft.modules[data.lookBlock]=appearanceDraft[data.lookBlock];target.classList.toggle('selected',appearanceDraft[data.lookBlock]);target.setAttribute('aria-pressed',String(appearanceDraft[data.lookBlock]));target.querySelector('i').textContent=appearanceDraft[data.lookBlock]?'−':'+';previewAppearance();return;}
  previewAppearance();refreshAppearanceSheet();
 });
 sheet.addEventListener('input',event=>{
  if(appearanceSaving)return;const key=event.target.dataset.lookInput;if(!key)return;
  appearanceDraft[key]=event.target.type==='checkbox'?event.target.checked:event.target.type==='range'?Number(event.target.value):event.target.value;
  if(key==='radius')sheet.querySelector('#appearanceRadiusValue').textContent=appearanceDraft.radius;
  previewAppearance();
  if(key==='accent')sheet.querySelectorAll('[data-look-accent]').forEach(button=>{const selected=button.dataset.lookAccent===appearanceDraft.accent;button.setAttribute('aria-pressed',String(selected));button.textContent=selected?'✓':'';});
 });
}
