const {signAsync}=require('@electron/osx-sign');
const {execFileSync}=require('node:child_process');
module.exports=async options=>{
 // electron-builder 25 leaves Electron's linker signature invalid when no
 // Developer ID is configured. Sign every nested binary and seal the bundle.
 const identity=options.identity||'-';
 await signAsync({...options,identity,identityValidation:false,preAutoEntitlements:false,preEmbedProvisioningProfile:false,gatekeeperAssess:false,strictVerify:undefined,...(identity==='-'?{timestamp:'none'}:{})});
 execFileSync('codesign',['--verify','--deep','--strict','--verbose=2',options.app],{stdio:'inherit'});
};
