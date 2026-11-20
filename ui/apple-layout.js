/* Fit the actual loaded font inside a resized info widget, including saved small grids. */
function fitInfoWidgets(){
 if(document.documentElement.dataset.homeLayout!=='widgets')return;
 const canvas=document.createElement('canvas'),ctx=canvas.getContext('2d');
 for(const card of document.querySelectorAll('.home-info')){
  const title=card.querySelector('h1.g');if(!title||!card.clientWidth)continue;
  const style=getComputedStyle(card),label=card.querySelector('.eyebrow'),chips=card.querySelector('.chips');
  const width=card.clientWidth-parseFloat(style.paddingLeft)-parseFloat(style.paddingRight);
  const visible=el=>el&&getComputedStyle(el).display!=='none';
  const reserved=(visible(label)?label.getBoundingClientRect().height:0)+(visible(chips)?chips.getBoundingClientRect().height:0)+16;
  const height=card.clientHeight-parseFloat(style.paddingTop)-parseFloat(style.paddingBottom)-reserved;
  const font=getComputedStyle(title),scale=parseFloat(style.getPropertyValue('--widget-scale'))||1;
  ctx.font=`${font.fontWeight} 44px ${font.fontFamily}`;
  const measured=ctx.measureText(title.textContent).width;
  const size=Math.max(16,Math.min(44*scale,44*width/Math.max(measured,1),height/1.12));
  title.style.fontSize=Math.floor(size)+'px';title.title=title.textContent;
 }
}
addEventListener('resize',()=>requestAnimationFrame(fitInfoWidgets));
addEventListener('DOMContentLoaded',()=>document.fonts.ready.then(fitInfoWidgets));
