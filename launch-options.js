function splitJvmArgs(value){
 const result=[];let token='',quote=null,started=false;
 for(const c of String(value||'')){
  if(quote){if(c===quote)quote=null;else token+=c;started=true;}
  else if(c==='"'||c==="'"){quote=c;started=true;}
  else if(/\s/.test(c)){if(started){result.push(token);token='';started=false;}}
  else {token+=c;started=true;}
 }
 if(quote)throw Error('Не закрыта кавычка в аргументах JVM');if(started)result.push(token);return result;
}
function quickPlay(version,address){
 if(!address)return undefined;
 const server=String(address).trim();if(!server||/[\s\x00]/.test(server))throw Error('Некорректный адрес сервера');
 const [major,minor,patch=0]=version.split('.').map(Number),modern=major>=26||(major===1&&(minor>=20));
 return {type:modern?'multiplayer':'legacy',identifier:server};
}
function neoPrefix(version){const [major,minor,patch=0]=version.split('.').map(Number);return major===1?`${minor}.${patch}.`:`${major}.${minor}.`;}
function compareVersions(a,b){const aa=a.split(/[.-]/),bb=b.split(/[.-]/);for(let i=0;i<Math.max(aa.length,bb.length);i++){const x=Number(aa[i]||0),y=Number(bb[i]||0);if(x!==y)return x-y;}return a.localeCompare(b);}
module.exports={splitJvmArgs,quickPlay,neoPrefix,compareVersions};
