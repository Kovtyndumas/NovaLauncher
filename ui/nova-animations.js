/* Original Nova poses, authored for this launcher. Time is seconds; angles are radians. */
window.NovaAnimations={create(mode){
 const animation=new skinview3d.FunctionAnimation((player,time)=>{
  const rig=player.skin,t=time,breath=Math.sin(t*2.1);
  rig.leftArm.rotation.z=.055+breath*.018;rig.rightArm.rotation.z=-.055-breath*.018;
  rig.body.rotation.x=breath*.012;
  if(!animation.suppressAutoLook){rig.head.rotation.y=Math.sin(t*.7)*.09;rig.head.rotation.x=Math.sin(t*.9)*.035;}
  if(mode==='walk'||mode==='sneak'||mode==='crouch'){
   const swing=Math.sin(t*(mode==='walk'?6:3.5))*(mode==='walk'?.4:.2);
   rig.leftLeg.rotation.x=swing;rig.rightLeg.rotation.x=-swing;
   rig.leftArm.rotation.x=-swing*.75;rig.rightArm.rotation.x=swing*.75;
   animation.controlsLegs=true;
   if(mode!=='walk'){rig.body.rotation.x=.2;player.position.y=-.65;rig.head.rotation.x=-.1;}
  }else if(mode==='wave'){
   rig.rightArm.rotation.z=-2.25+Math.sin(t*10)*.22;
   rig.rightArm.rotation.x=-.15;rig.head.rotation.z=-.06;
  }else if(mode==='look'){
   rig.head.rotation.y=Math.sin(t*1.1)*.6;rig.head.rotation.x=Math.sin(t*.8)*.14;
  }else if(mode==='dance'){
   const beat=Math.sin(t*7.5);rig.body.rotation.z=beat*.13;
   rig.leftArm.rotation.z=.85+beat*.45;rig.rightArm.rotation.z=-.85+beat*.45;
   rig.leftLeg.rotation.x=beat*.22;rig.rightLeg.rotation.x=-beat*.22;
   player.position.y=Math.abs(beat)*.35;animation.controlsLegs=true;
  }else if(mode==='victory'){
   rig.leftArm.rotation.z=2.5+breath*.08;rig.rightArm.rotation.z=-2.5-breath*.08;
   rig.head.rotation.x=-.12;
  }else if(mode==='sad'){
   rig.head.rotation.x=.35;rig.body.rotation.x=.08;
   rig.leftArm.rotation.x=.15;rig.rightArm.rotation.x=.15;
  }else if(mode==='cool'){
   rig.body.rotation.y=.13;rig.head.rotation.y=-.22;rig.leftArm.rotation.z=.45;rig.rightArm.rotation.x=-.5;
  }else if(mode==='punch'){
   const strike=Math.max(0,Math.sin(Math.min(t,1)*Math.PI));rig.rightArm.rotation.x=-1.5*strike;rig.body.rotation.y=-.18*strike;
  }
 });animation.controlsLegs=['walk','sneak','crouch','dance'].includes(mode);return animation;
}};
