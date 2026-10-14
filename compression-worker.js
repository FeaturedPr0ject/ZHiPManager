importScripts('https://cdn.jsdelivr.net/npm/fflate@0.8.2/umd/index.js');

function crc32(data){
 const table=crc32.table||(crc32.table=(()=>{
  const values=new Uint32Array(256);
  for(let n=0;n<256;n++){let value=n;for(let k=0;k<8;k++)value=(value&1)?(0xedb88320^(value>>>1)):(value>>>1);values[n]=value>>>0;}
  return values;
 })());
 let value=0xffffffff;
 for(let i=0;i<data.length;i++)value=table[(value^data[i])&0xff]^(value>>>8);
 return (value^0xffffffff)>>>0;
}
function safeArchivePath(path){
 const normalized=path.replace(/\\/g,'/').replace(/^\.\//,'');
 return !!normalized&&!normalized.startsWith('/')&&!/^[a-zA-Z]:/.test(normalized)&&!normalized.split('/').some(part=>!part||part==='..');
}
function formatBytes(bytes){
 if(!bytes)return '0 B';
 const units=['B','KB','MB','GB'];const i=Math.min(Math.floor(Math.log(bytes)/Math.log(1024)),3);
 return (bytes/Math.pow(1024,i)).toLocaleString('en-US',{maximumFractionDigits:i===0?0:1})+' '+units[i];
}
self.onmessage=async event=>{
 const {files,mode}=event.data;
 try{
  const runs=mode==='fast'?[{level:1}]:mode==='smart'?[{level:1},{level:6}]:[{level:1},{level:6},{level:9}];
  let zstd=null;
  if(mode==='ultra'||mode==='ultimate'){
   self.postMessage({type:'status',message:'Loading Zstandard WebAssembly engine…'});
   const module=await import('https://cdn.jsdelivr.net/npm/@bokuweb/zstd-wasm@0.0.27/+esm');
   await module.init('https://cdn.jsdelivr.net/npm/@bokuweb/zstd-wasm@0.0.27/dist/web/zstd.wasm');
   zstd={compress:module.compress,decompress:module.decompress};
  }
  const encoder=new TextEncoder();
  const chunks=[];
  const header=new Uint8Array(9);
  const version=(mode==='ultra'||mode==='ultimate')?4:2;
  header.set([90,72,73,80,version],0);
  new DataView(header.buffer).setUint32(5,files.length,true);
  chunks.push(header);
  for(let i=0;i<files.length;i++){
   const {path,buffer}=files[i];
   if(!safeArchivePath(path))throw new Error('Invalid file path: '+path);
   const name=encoder.encode(path);
   if(name.length>65535)throw new Error('File path is too long: '+path);
   const raw=new Uint8Array(buffer);
   let best=raw,codec=0,label='raw';
   const checksum=crc32(raw);
   const benchmark=async(candidate,candidateCodec,candidateLabel,decode)=>{
    try{
     const decoded=decode(candidate);
     if(decoded.length!==raw.length||crc32(decoded)!==checksum){
      self.postMessage({type:'log',message:candidateLabel+' rejected: round-trip verification failed for '+path+'.'});
      return;
     }
     self.postMessage({type:'log',message:candidateLabel+' benchmark: '+formatBytes(candidate.length)+' for '+path+' (verified).'});
     if(candidate.length<best.length){best=candidate;codec=candidateCodec;label=candidateLabel;}
    }catch(error){
     self.postMessage({type:'log',message:candidateLabel+' rejected for '+path+': '+(error?.message||'verification failed')});
    }
   };
   for(const options of runs){
    const candidate=fflate.deflateSync(raw,options);
    await benchmark(candidate,1,'DEFLATE level '+options.level,data=>fflate.inflateSync(data));
   }
   if(mode==='ultra'||mode==='ultimate'){
    for(const level of [19,20,21,22]){
     try{
      const candidate=zstd.compress(raw,level);
      await benchmark(candidate,2,'Zstandard level '+level,data=>zstd.decompress(data));
     }catch(error){
      self.postMessage({type:'log',message:'Zstandard level '+level+' failed for '+path+': '+(error?.message||'codec error')});
     }
    }
   }
   const record=new Uint8Array(15+name.length);
   const view=new DataView(record.buffer);
   view.setUint16(0,name.length,true);view.setUint32(2,raw.length,true);
   view.setUint32(6,best.length,true);view.setUint32(10,checksum,true);
   record[14]=codec;record.set(name,15);
   chunks.push(record,best);
   self.postMessage({type:'progress',index:i+1,total:files.length,message:'Processed '+path+': '+formatBytes(raw.length)+' → '+formatBytes(best.length)+' ('+label+').'});
  }
  const total=chunks.reduce((sum,part)=>sum+part.byteLength,0);
  const output=new Uint8Array(total);let offset=0;
  for(const part of chunks){output.set(part,offset);offset+=part.byteLength;}
  self.postMessage({type:'done',buffer:output.buffer,version},[output.buffer]);
 }catch(error){self.postMessage({type:'error',message:error?.message||'Compression failed'});}
};