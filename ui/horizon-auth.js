let activeHorizonSignIn=null;
async function signInHorizon(id){
 if(activeHorizonSignIn||S.playing||S.toolBusy)return false;
 const dialog=document.createElement('dialog');dialog.className='horizon-signin';
 dialog.innerHTML='<h2>Вход в 26.3 RUST</h2><p class="tool-status">Horizon использует собственный вход Microsoft. Подтверди его один раз — он сохранится для всех RUST-сборок Nova.</p><p class="hz-status" role="status">Получаем код Microsoft…</p><div class="hz-code" hidden></div><div class="dialog-actions"><button class="b" data-hz="cancel">Отмена</button><button class="b" data-hz="open" hidden>Открыть Microsoft ↗</button></div>';
 const state={dialog,started:false,cancel:false};activeHorizonSignIn=state;S.toolBusy=true;document.body.append(dialog);dialog.showModal();
 const cancel=async()=>{state.cancel=true;dialog.querySelector('.hz-status').textContent='Отменяем вход…';if(state.started)await api.horizonCancelLogin();};
 dialog.addEventListener('cancel',event=>{event.preventDefault();cancel().catch(()=>{});});
 dialog.querySelector('[data-hz=cancel]').onclick=()=>cancel().catch(()=>{});
 dialog.querySelector('[data-hz=open]').onclick=()=>api.horizonLoginLink().catch(error=>toast(error.message));
 try{const result=await api.horizonLogin(id||null);if(S.tab==='horizon')S.horizonPath=(await api.horizonStatus()).path;return !state.cancel&&result.owner===true;}
 catch(error){if(!state.cancel)toast(error.message);return false;}
 finally{activeHorizonSignIn=null;dialog.close();dialog.remove();S.toolBusy=false;render();}
}
function horizonAuthStatus(status){
 const state=activeHorizonSignIn;if(!state)return;
 if(status.state==='starting'){state.started=true;if(state.cancel)api.horizonCancelLogin().catch(()=>{});}
 if(status.state==='code'&&!state.cancel){const code=state.dialog.querySelector('.hz-code');code.hidden=false;code.textContent=status.code;state.dialog.querySelector('.hz-status').textContent='Открой Microsoft, введи этот код и выбери аккаунт с Minecraft Java Edition. После подтверждения запуск продолжится.';state.dialog.querySelector('[data-hz=open]').hidden=false;}
 if(status.state==='done')state.dialog.querySelector('.hz-status').textContent='Вход подтверждён. Готовим запуск…';
}
api.on('horizon:auth',horizonAuthStatus);
async function ensureHorizonAccount(instance){
 S.toolBusy=true;render();let status;
 try{status=await api.horizonAccount(instance.id);}
 catch(error){toast(error.message);return false;}
 finally{S.toolBusy=false;render();}
 if(status.owner)return true;
 return signInHorizon(instance.id);
}
