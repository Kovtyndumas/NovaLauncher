// Release-version Java requirements shared by the UI and launcher process.
(function(root){
  function javaMajor(version){
    const [major,minor,patch=0]=String(version).split('.').map(Number);
    if(major>=26)return 25;
    if(major===1){
      if(minor>20||(minor===20&&patch>=5))return 21;
      if(minor>=18)return 17;
      if(minor===17)return 16;
      return 8;
    }
    throw new Error('Неизвестный формат версии Minecraft: '+version);
  }
  if(typeof module==='object'&&module.exports)module.exports=javaMajor;
  else root.novaJavaMajor=javaMajor;
})(globalThis);
