const {spawnSync}=require('node:child_process');
const env={...process.env};
// Missing Actions secrets arrive as empty strings. electron-builder treats an
// empty CSC_LINK as a local certificate path instead of disabling signing.
for(const key of ['CSC_LINK','CSC_KEY_PASSWORD','WIN_CSC_LINK','WIN_CSC_KEY_PASSWORD','APPLE_ID','APPLE_APP_SPECIFIC_PASSWORD','APPLE_TEAM_ID'])if(!env[key]?.trim())delete env[key];
const result=spawnSync(process.execPath,[require.resolve('electron-builder/cli.js'),...process.argv.slice(2)],{env,stdio:'inherit'});
if(result.error){console.error(result.error.message);process.exit(1);}
process.exit(result.status??1);
