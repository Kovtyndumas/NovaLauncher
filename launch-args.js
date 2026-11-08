// MCLC assumes Minecraft versions start with "1." and drops macOS's first-thread
// flag for year-based releases (26.x). Apply it to the final JVM argv, before
// spawning Java, and preserve the old LWJGL 2 behaviour for 1.12 and earlier.
function prepareLaunchArgs(args,version,platform=process.platform){
  const result=[...args], [major,minor]=String(version).split('.').map(Number);
  const modern=major>=26||(major===1&&minor>=13);
  if(platform==='darwin'&&modern&&!result.includes('-XstartOnFirstThread'))result.unshift('-XstartOnFirstThread');
  return result;
}
function patchClient(client,version,platform=process.platform){
  if(typeof client.startMinecraft!=='function')return;
  const start=client.startMinecraft.bind(client);
  client.startMinecraft=args=>start(prepareLaunchArgs(args,version,platform));
}
module.exports={prepareLaunchArgs,patchClient};
