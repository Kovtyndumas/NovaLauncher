const fs=require('fs'),path=require('path'),crypto=require('crypto'),compatibility=require('./mod-compatibility');
const rules=crypto.createHash('sha256').update(fs.readFileSync(require.resolve('./mod-compatibility'))).digest('hex');
async function check(root,inst,options={},inspect=compatibility.check){
 const dir=path.join(root,'mods'),files=fs.existsSync(dir)?fs.readdirSync(dir).filter(n=>/\.jar(?:\.disabled)?$/.test(n)).sort().map(name=>{const s=fs.statSync(path.join(dir,name));return [name,s.size,s.mtimeMs,s.ctimeMs,s.ino];}):[];
 const key=crypto.createHash('sha256').update(JSON.stringify([rules,inst.version,inst.loader,inst.loaderVersion,files])).digest('hex'),file=path.join(root,'cache','nova-compatibility.json');
 try{if(fs.statSync(file).size<4*1024*1024){const old=JSON.parse(fs.readFileSync(file,'utf8'));if(old.key===key&&Date.now()-old.at<86400000&&Array.isArray(old.report.issues))return {...old.report,cached:true};}}catch{}
 const report=await inspect(root,inst,options);
 try{fs.mkdirSync(path.dirname(file),{recursive:true});const temp=file+'.tmp';fs.writeFileSync(temp,JSON.stringify({key,at:Date.now(),report}));fs.renameSync(temp,file);}catch{}
 return report;
}
module.exports={check};
