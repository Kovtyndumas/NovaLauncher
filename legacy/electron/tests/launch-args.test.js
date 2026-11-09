const {test}=require('node:test'),assert=require('node:assert/strict'),{prepareLaunchArgs,patchClient}=require('../launch-args'),analyze=require('../crash-analysis');
test('adds first-thread flag to modern Minecraft on Mac without mutating original argv',()=>{
 for(const v of ['26.3','26.1','1.21.1','1.13']){const args=['-Xmx6G','-cp','libraries','net.minecraft.client.main.Main'],result=prepareLaunchArgs(args,v,'darwin');assert.equal(result[0],'-XstartOnFirstThread');assert.equal(args[0],'-Xmx6G');assert.equal(result.at(-1),args.at(-1));}
});
test('keeps legacy Mac launches and other platforms unchanged; avoids duplicate flags',()=>{
 const args=['-Xmx4G'];assert.deepEqual(prepareLaunchArgs(args,'1.12.2','darwin'),args);for(const os of ['win32','linux'])assert.deepEqual(prepareLaunchArgs(args,'26.3',os),args);
 assert.equal(prepareLaunchArgs(['-XstartOnFirstThread',...args],'26.3','darwin').filter(x=>x==='-XstartOnFirstThread').length,1);
});
test('client wrapper patches final launch arguments and preserves return value',()=>{
 let received;const marker={};const client={startMinecraft(args){received=args;return marker;}};patchClient(client,'26.3','darwin');assert.equal(client.startMinecraft(['-cp','game']),marker);assert.equal(received[0],'-XstartOnFirstThread');
});
test('SDL initialization crash is prioritized over incidental authentication errors',()=>{
 const text='[MCLC]: Launching with arguments -cp lwjgl-opengl.jar authlib.jar\nFailed to fetch user properties: Status 401\njava.lang.IllegalStateException: Unable to initialize SDL: No available video device';
 const report=analyze(text,255);assert.match(report.title,/окно игры/);assert.match(report.evidence[0],/Unable to initialize SDL/);assert(!report.evidence.some(x=>x.includes('Launching with arguments')));
});
test('OpenGL library names and successful backend initialization are not failures',()=>{
 const text='[MCLC]: Launching with arguments -cp lwjgl-opengl.jar\nBackend library: OpenGL\nOpenGL initialized successfully';assert.equal(analyze(text,255).title,'Игра завершилась с ошибкой');assert.equal(analyze('GLFW error',0),null);assert.equal(analyze('OpenGL error: context failed',1).title,'Ошибка графики');
});
