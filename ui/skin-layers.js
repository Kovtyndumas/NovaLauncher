/* Voxel overlay for the existing skinview3d meshes. Base skin and joints stay intact. */
window.NovaSkinLayers=class {
 constructor(viewer){this.viewer=viewer;this.parts=[];this.pixelCount=0;}
 dispose(){for(const item of this.parts){item.mesh.parent?.remove(item.mesh);item.mesh.geometry.dispose();item.mesh.material.dispose();item.outer.visible=item.visible;}this.parts=[];this.pixelCount=0;}
 apply(enabled=true){
  this.dispose();if(!enabled)return {applied:false,pixels:0};
  const viewer=this.viewer,skin=viewer.playerObject?.skin,source=viewer.skinCanvas;
  if(!skin||!source?.width||!source.height)return {applied:false,pixels:0};
  try{
   const sample=document.createElement('canvas');sample.width=64;sample.height=64;const context=sample.getContext('2d',{willReadFrequently:true});context.imageSmoothingEnabled=false;context.drawImage(source,0,0,64,64);const pixels=context.getImageData(0,0,64,64).data;
   for(const key of ['head','body','rightArm','leftArm','rightLeg','leftLeg']){
    const outer=skin[key]?.outerLayer;if(!outer?.geometry?.getAttribute||!outer.parent)continue;
    const original=outer.geometry,positions=original.getAttribute('position'),normals=original.getAttribute('normal'),uvs=original.getAttribute('uv'),index=original.index;
    if(positions.count!==24||!index||!uvs)continue;
    const scale=[outer.scale.x,outer.scale.y,outer.scale.z],dimensions=[original.parameters?.width||1,original.parameters?.height||1,original.parameters?.depth||1];
    const vertices=[],normalData=[],textureData=[],indices=[];let count=0;
    const blend=(attr,start,s,t,axis)=>attr.array[(start+0)*attr.itemSize+axis]*(1-s)*(1-t)+attr.array[(start+1)*attr.itemSize+axis]*s*(1-t)+attr.array[(start+2)*attr.itemSize+axis]*(1-s)*t+attr.array[(start+3)*attr.itemSize+axis]*s*t;
    for(let face=0;face<6;face++){
     const start=face*4;const cols=Math.round((Math.abs(uvs.getX(start+1)-uvs.getX(start))+Math.abs(uvs.getY(start+1)-uvs.getY(start)))*64),rows=Math.round((Math.abs(uvs.getX(start+2)-uvs.getX(start))+Math.abs(uvs.getY(start+2)-uvs.getY(start)))*64);
     if(!cols||!rows)continue;
     for(let row=0;row<rows;row++)for(let col=0;col<cols;col++){
      const s=(col+.5)/cols,t=(row+.5)/rows,u=blend(uvs,start,s,t,0),v=blend(uvs,start,s,t,1),x=Math.min(63,Math.max(0,Math.floor(u*64))),y=Math.min(63,Math.max(0,Math.floor((1-v)*64)));
      if(pixels[(y*64+x)*4+3]<16)continue;
      const center=[0,1,2].map(axis=>blend(positions,start,s,t,axis)*scale[axis]);
      const thickness=key==='head'?.65:.4;
      const sizes=[0,1,2].map(axis=>(Math.abs(positions.array[(start+1)*3+axis]-positions.array[start*3+axis])/cols+Math.abs(positions.array[(start+2)*3+axis]-positions.array[start*3+axis])/rows)*scale[axis]+Math.abs(normals.array[start*3+axis])*thickness);
      const offset=vertices.length/3;
      for(let vertex=0;vertex<24;vertex++){for(let axis=0;axis<3;axis++){vertices.push(center[axis]+positions.array[vertex*3+axis]/dimensions[axis]*sizes[axis]);normalData.push(normals.array[vertex*3+axis]);}textureData.push(u,v);}
      for(const value of index.array)indices.push(value+offset);count++;
     }
    }
    if(!count)continue;
    const geometry=original.clone(),Attribute=positions.constructor;
    geometry.clearGroups();geometry.setIndex(indices);geometry.setAttribute('position',new Attribute(new Float32Array(vertices),3));geometry.setAttribute('normal',new Attribute(new Float32Array(normalData),3));geometry.setAttribute('uv',new Attribute(new Float32Array(textureData),2));geometry.computeBoundingBox();geometry.computeBoundingSphere();
    const material=outer.material.clone();material.alphaTest=.03;material.polygonOffset=false;
    const mesh=new outer.constructor(geometry,material);mesh.name='nova-voxel-'+key;mesh.position.copy(outer.position);mesh.rotation.copy(outer.rotation);mesh.visible=outer.visible;mesh.userData.novaSkinLayers=true;
    this.parts.push({mesh,outer,visible:outer.visible});outer.parent.add(mesh);outer.visible=false;this.pixelCount+=count;
   }
   return {applied:this.pixelCount>0,pixels:this.pixelCount,meshes:this.parts.length};
  }catch(error){this.dispose();return {applied:false,pixels:0,error:String(error.message||error)};}
 }
};
