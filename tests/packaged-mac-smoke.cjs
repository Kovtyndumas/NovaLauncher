const {spawnSync,execFileSync}=require('node:child_process'),fs=require('node:fs'),path=require('node:path');
const dir=process.argv[2]||'dist';
const apps=fs.readdirSync(dir,{withFileTypes:true}).filter(e=>e.isDirectory()&&/^mac(?:-|$)/.test(e.name)).map(e=>path.resolve(dir,e.name,'Nova.app')).filter(p=>fs.existsSync(p));
if(!apps.length)throw Error('No packaged macOS apps found');
for(const app of apps){execFileSync('codesign',['--verify','--deep','--strict','--verbose=2',app],{stdio:'inherit'});const binary=path.join(app,'Contents/MacOS/Nova'),architecture=execFileSync('lipo',['-archs',binary],{encoding:'utf8'}).trim();
 if(!architecture.split(/\s+/).includes(process.arch==='arm64'?'arm64':'x86_64')){console.log('Signature verified; startup tested on native architecture only:',architecture);continue;}
 const env={...process.env,NOVA_SMOKE_TEST:'1'};delete env.ELECTRON_RUN_AS_NODE;
 const r=spawnSync(binary,[],{env,encoding:'utf8',timeout:45000});if(r.status!==0||!r.stdout?.includes('NOVA_PACKAGED_SMOKE_OK'))throw Error('Packaged app failed: '+(r.error?.message||r.stderr||r.stdout));console.log(r.stdout.trim());
}

// Validate the actual distributed images as well as the build directory.
for(const name of fs.readdirSync(dir).filter(n=>n.endsWith('.dmg'))){
 const image=path.resolve(dir,name),mount=fs.mkdtempSync(path.join(require('node:os').tmpdir(),'nova-dmg-check-'));let attached=false;
 try{execFileSync('hdiutil',['verify',image],{stdio:'pipe'});execFileSync('hdiutil',['attach','-readonly','-nobrowse','-mountpoint',mount,image],{stdio:'pipe'});attached=true;execFileSync('codesign',['--verify','--deep','--strict',path.join(mount,'Nova.app')],{stdio:'inherit'});console.log('DMG verified:',name);}
 finally{if(attached)execFileSync('hdiutil',['detach',mount],{stdio:'pipe'});fs.rmSync(mount,{recursive:true,force:true});}
}
