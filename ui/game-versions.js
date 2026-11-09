(function(root){
 function releases(manifest){return [...new Set((manifest?.versions||[]).filter(v=>v.type==='release'&&/^(?:1\.(?:0|[1-9]\d*)(?:\.\d+)?|(?:[2-9]\d+)\.\d+(?:\.\d+)?)$/.test(v.id)).map(v=>v.id))];}
 if(typeof module==='object')module.exports={releases};else root.novaReleaseVersions=releases;
})(typeof globalThis==='object'?globalThis:this);
