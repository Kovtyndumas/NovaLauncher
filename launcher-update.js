module.exports=function({app,getFeed,getUpdater=()=>require('electron-updater').autoUpdater,notify=()=>{}}){
 let updater,feed='',state={phase:'idle',message:'Обновления ещё не проверялись'};
 function set(next){state=next;notify(state);return state;}
 function ready(){
  if(!app.isPackaged)throw Error('Обновления доступны в установленной сборке Nova');
  const url=String(getFeed()||'').trim();if(!url)throw Error('Не настроен адрес публикации обновлений Nova');const u=new URL(url);if(u.protocol!=='https:'||u.username||u.password)throw Error('Нужен HTTPS-адрес сервера обновлений');
  if(!updater){updater=getUpdater();updater.autoDownload=false;updater.autoInstallOnAppQuit=false;
   updater.on('checking-for-update',()=>set({phase:'checking',message:'Проверяем обновления…'}));
   updater.on('update-available',info=>set({phase:'available',message:'Доступна Nova '+info.version,version:info.version}));
   updater.on('update-not-available',()=>set({phase:'current',message:'Установлена последняя версия'}));
   updater.on('download-progress',p=>set({phase:'downloading',message:'Загрузка обновления · '+Math.round(p.percent)+'%',percent:p.percent}));
   updater.on('update-downloaded',info=>set({phase:'downloaded',message:'Nova '+info.version+' готова к установке',version:info.version}));
   updater.on('error',e=>set({phase:'error',message:e.message}));
  }
  if(url!==feed){if(['downloading','downloaded'].includes(state.phase))throw Error('Завершите текущее обновление перед сменой сервера');updater.setFeedURL({provider:'generic',url});feed=url;}
  return updater;
 }
 return {status:()=>state,async check(){if(['checking','downloading','downloaded'].includes(state.phase))return state;try{await ready().checkForUpdates();return state;}catch(e){return set({phase:'error',message:e.message});}},async download(){if(state.phase!=='available')throw Error('Сначала проверьте обновления');await ready().downloadUpdate();return state;},install(){if(state.phase!=='downloaded')throw Error('Обновление ещё не скачано');ready().quitAndInstall(false,true);}};
};
