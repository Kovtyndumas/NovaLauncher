const {test}=require('node:test'),assert=require('node:assert/strict'),net=require('net'),fs=require('fs'),os=require('os'),path=require('path');
const {Presence,packet,activity,endpoints}=require('../discord-presence');
const wait=ms=>new Promise(r=>setTimeout(r,ms));
test('presence contains game metadata but no account tokens, server addresses or private paths',()=>{const a=activity({phase:'playing',started:123,instance:{name:'Survival',loader:'Fabric',version:'26.2',token:'secret',path:'/private',server:'private'}});assert.equal(a.timestamps.start,123);assert.match(a.state,/Fabric 26.2/);assert(!JSON.stringify(a).includes('secret'));assert(!JSON.stringify(a).includes('private'));assert(!activity({phase:'launcher'}).timestamps);});
test('disabled or invalid application id opens no connection',()=>{const p=new Presence({connect:()=>{throw Error('Should not connect');}});p.configure(true,'not-a-token');assert.equal(p.state,'missing-id');p.configure(false,'1568956638310699119');assert.equal(p.state,'disabled');p.stop();});
test('IPC handshake, split packets, heartbeat, activity and clearing work',async t=>{
 const dir=fs.mkdtempSync(path.join(os.tmpdir(),'nova-rpc-')),socketPath=process.platform==='win32'?'\\\\.\\pipe\\nova-rpc-'+path.basename(dir):path.join(dir,'discord-ipc-0'),messages=[];let client;
 const server=net.createServer(s=>{client=s;let buffer=Buffer.alloc(0);s.on('error',()=>{});s.on('data',chunk=>{buffer=Buffer.concat([buffer,chunk]);while(buffer.length>=8){const n=buffer.readUInt32LE(4);if(buffer.length<8+n)return;const op=buffer.readUInt32LE(),data=JSON.parse(buffer.subarray(8,8+n));buffer=buffer.subarray(8+n);messages.push({op,data});if(op===0){const ready=packet(1,{evt:'READY'});s.write(ready.subarray(0,5));setTimeout(()=>{s.write(ready.subarray(5));s.write(packet(3,{nonce:'ping'}));},5);}}});});
 await new Promise(r=>server.listen(socketPath,r));const p=new Presence({paths:[socketPath]});t.after(()=>{p.stop();client?.destroy();server.close();fs.rmSync(dir,{recursive:true,force:true});});
 p.update({phase:'playing',started:100,instance:{name:'Test',loader:'Fabric',version:'26.2'}});p.configure(true,'1568956638310699119');
 for(let i=0;i<100&&p.state!=='connected';i++)await wait(10);
 assert.equal(p.state,'connected');await wait(20);assert.equal(messages[0].data.client_id,'1568956638310699119');assert(messages.some(m=>m.op===4));assert(messages.some(m=>m.data.cmd==='SET_ACTIVITY'&&m.data.args.activity?.timestamps.start===100));p.stop();await wait(20);assert(messages.some(m=>m.data.cmd==='SET_ACTIVITY'&&m.data.args.activity===null));
});
test('Windows pipe names are supported',()=>{assert.equal(endpoints('win32')[0],'\\\\.\\pipe\\discord-ipc-0');});
