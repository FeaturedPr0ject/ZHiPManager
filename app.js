(function(){
'use strict';
const BASE_MAX_BYTES=400*1024*1024;
const PRO_MAX_BYTES=800*1024*1024;
let proUnlocked=false;
const isPro=()=>proUnlocked;
const currentMaxBytes=()=>isPro()?PRO_MAX_BYTES:BASE_MAX_BYTES;
const files=[];
const el=id=>document.getElementById(id);
const dropzone=el('dropzone'),fileInput=el('fileInput'),folderInput=el('folderInput'),fileList=el('fileList');
let busy=false;

const CACHE_TTL=48*60*60*1000;
const CACHE_LIMIT=20*1024*1024;
const CACHE_API='https://vbmngpqaoerptdgbhqsm.supabase.co/functions/v1/cache-gateway';
function openCacheDb(){
 return new Promise((resolve,reject)=>{
  const request=indexedDB.open('zhipmanager-cache',1);
  request.onupgradeneeded=()=>request.result.createObjectStore('archives',{keyPath:'key'});
  request.onsuccess=()=>resolve(request.result);
  request.onerror=()=>reject(request.error);
 });
}
async function localCacheGet(key){
 try{
  const db=await openCacheDb();
  const entry=await new Promise((resolve,reject)=>{
   const request=db.transaction('archives','readonly').objectStore('archives').get(key);
   request.onsuccess=()=>resolve(request.result||null);
   request.onerror=()=>reject(request.error);
  });
  if(!entry)return null;
  if(!entry.expiresAt||entry.expiresAt<=Date.now()){
   db.transaction('archives','readwrite').objectStore('archives').delete(key);
   return null;
  }
  return entry.blob instanceof Blob?entry.blob:null;
 }catch(error){console.warn('Local cache unavailable',error);return null;}
}
async function localCachePut(key,blob){
 try{
  const db=await openCacheDb();
  await new Promise((resolve,reject)=>{
   const request=db.transaction('archives','readwrite').objectStore('archives').put({key,blob,expiresAt:Date.now()+CACHE_TTL});
   request.onsuccess=resolve;request.onerror=()=>reject(request.error);
  });
  db.close();
 }catch(error){console.warn('Could not save local cache',error);}
}
async function archiveCacheKey(items,mode){
 const encoder=new TextEncoder();
 const parts=[encoder.encode('ZHiPManager-CHC-v1\0'+mode)];
 for(const item of items){
  const path=encoder.encode(item.path);
  const digest=new Uint8Array(await crypto.subtle.digest('SHA-256',item.bytes));
  parts.push(path,new Uint8Array([0]),digest);
 }
 const length=parts.reduce((sum,part)=>sum+part.length,0);
 const joined=new Uint8Array(length);let offset=0;
 for(const part of parts){joined.set(part,offset);offset+=part.length;}
 const hash=new Uint8Array(await crypto.subtle.digest('SHA-256',joined));
 return Array.from(hash,value=>value.toString(16).padStart(2,'0')).join('');
}
async function cloudCacheGet(key){
 try{
  const response=await fetch(CACHE_API+'?key='+key,{headers:{'X-Cache-Key':key}});
  if(!response.ok)return null;
  const bytes=new Uint8Array(await response.arrayBuffer());
  if(bytes.length<4||bytes[0]!==67||bytes[1]!==72||bytes[2]!==67||bytes[3]!==1)return null;
  return new Blob([bytes.slice(4)],{type:'application/vnd.zhip'});
 }catch(error){console.warn('Cloud cache unavailable',error);return null;}
}
async function cloudCachePut(key,blob){
 if(blob.size+4>CACHE_LIMIT)return;
 try{
  const bytes=new Uint8Array(await blob.arrayBuffer());
  const payload=new Uint8Array(bytes.length+4);
  payload.set([67,72,67,1],0);payload.set(bytes,4);
  await fetch(CACHE_API,{method:'POST',headers:{'Content-Type':'application/vnd.zhipmanager.cache','X-Cache-Key':key},body:payload});
 }catch(error){console.warn('Cloud cache upload skipped',error);}
}
function downloadArchive(blob,name){
 const url=URL.createObjectURL(blob);
 const anchor=document.createElement('a');anchor.href=url;anchor.download=name+'.zhip';
 document.body.append(anchor);anchor.click();anchor.remove();
 setTimeout(()=>URL.revokeObjectURL(url),30000);
}

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
let zstdApi=null;
let zstdLoadPromise=null;
async function ensureZstd(){
 if(zstdApi)return zstdApi;
 if(!zstdLoadPromise){
  zstdLoadPromise=(async()=>{
   const module=await import('https://cdn.jsdelivr.net/npm/@bokuweb/zstd-wasm@0.0.27/+esm');
   await module.init('https://cdn.jsdelivr.net/npm/@bokuweb/zstd-wasm@0.0.27/dist/web/zstd.wasm');
   if(typeof module.compress!=='function'||typeof module.decompress!=='function')throw new Error('The Zstandard module did not expose compress/decompress functions.');
   zstdApi={compress:module.compress,decompress:module.decompress};
   return zstdApi;
  })().catch(error=>{zstdLoadPromise=null;throw error;});
 }
 return zstdLoadPromise;
}

function formatBytes(bytes){
 if(bytes===0)return '0 B';
 const units=['B','KB','MB','GB'];const i=Math.min(Math.floor(Math.log(bytes)/Math.log(1024)),3);
 return (bytes/Math.pow(1024,i)).toLocaleString('en-US',{maximumFractionDigits:i===0?0:1})+' '+units[i];
}
function safeName(s){return s.replace(/[\\/:*?"<>|\u0000-\u001f]/g,'_').replace(/\s+/g,' ').trim().replace(/^\.+/,'')||'MyArchive';}
function showMessage(text,type){const m=el('message');m.textContent=text;m.className='message show '+type;}
function clearMessage(){el('message').className='message';el('message').textContent='';}
function addLog(target,message,type='info'){
 const list=el(target==='create'?'createLog':'extractLog');
 const count=el(target==='create'?'createLogCount':'extractLogCount');
 const empty=list.querySelector('.activity-empty');if(empty)empty.remove();
 const row=document.createElement('div');row.className='activity-entry '+type;
 const time=document.createElement('time');time.textContent=new Date().toLocaleTimeString();
 const text=document.createElement('span');text.textContent=message;
 row.append(time,text);list.append(row);list.scrollTop=list.scrollHeight;
 while(list.children.length>100)list.firstElementChild.remove();
 count.textContent=list.children.length+' event'+(list.children.length===1?'':'s');
}
function toast(text){const t=el('toast');t.textContent=text;t.classList.add('show');setTimeout(()=>t.classList.remove('show'),2600);}
function totalSize(){return files.reduce((n,f)=>n+f.file.size,0);}
function removeLocalCache(key){return openCacheDb().then(db=>new Promise((resolve,reject)=>{const tx=db.transaction('archives','readwrite');const request=key?tx.objectStore('archives').delete(key):tx.objectStore('archives').clear();tx.oncomplete=()=>{db.close();resolve();};tx.onerror=()=>{db.close();reject(tx.error);};})).catch(error=>console.warn('Cache cleanup failed',error));}
async function clearCloudCache(key){try{await fetch(CACHE_API+'?key='+encodeURIComponent(key),{method:'DELETE',headers:{'X-Cache-Key':key}});}catch(error){console.warn('Cloud cache cleanup skipped',error);}}
async function validateProCode(code){const response=await fetch(CACHE_API,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'validate-pro',code})});if(!response.ok)return false;const result=await response.json();return result.valid===true;}
function syncProUi(){const max=currentMaxBytes();el('limitPill').textContent=isPro()?'PRO · 800 MB':'400 MB limit';el('proStatus').textContent=isPro()?'Pro unlocked · 800 MB':'Unlock up to 800 MB';el('sizeLabel').textContent=formatBytes(totalSize())+' / '+(isPro()?'800 MB':'400 MB');el('proCodeBtn').textContent=isPro()?'Pro unlocked ✓':'＋ Pro code';el('ultimateProNote').textContent=isPro()?'Pro enabled · strongest Zstandard mode':'Requires Pro code';el('maxLimitLabel').textContent=isPro()?'800 MB maximum':'400 MB maximum';}
function render(){
 fileList.replaceChildren();
 if(!files.length){const empty=document.createElement('div');empty.className='empty-state';const ico=document.createElement('div');ico.className='empty-symbol';ico.textContent='＋';const copy=document.createElement('div');const strong=document.createElement('strong');strong.style.cssText='display:block;color:#b0bfd5;font-size:11px;margin-bottom:4px';strong.textContent='Your archive starts here';copy.append(strong,document.createTextNode('Choose one or more files to see them listed here.'));empty.append(ico,copy);fileList.append(empty);}
 files.forEach((item,index)=>{
  const row=document.createElement('div');row.className='file-row';
  const type=document.createElement('div');type.className='file-type';type.innerHTML='<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7"><path d="M6 3h8l5 5v13H6z"/><path d="M14 3v5h5M9 13h6m-6 4h6"/></svg>';
  const info=document.createElement('div');info.className='file-info';const name=document.createElement('div');name.className='file-name';name.textContent=item.path;name.title=item.path;const meta=document.createElement('div');meta.className='file-meta';meta.textContent='File · '+(item.file.type||'Unknown type');info.append(name,meta);
  const size=document.createElement('div');size.className='file-size';size.textContent=formatBytes(item.file.size);
  const remove=document.createElement('button');remove.type='button';remove.className='remove-file';remove.setAttribute('aria-label','Remove '+item.path);remove.textContent='×';remove.addEventListener('click',()=>{files.splice(index,1);render();clearMessage();});
  row.append(type,info,size,remove);fileList.append(row);
 });
 const maxBytes=currentMaxBytes();const total=totalSize(),over=total>maxBytes;
 el('fileCount').textContent=files.length+' file'+(files.length===1?'':'s');
 el('sizeLabel').textContent=formatBytes(total)+' / '+(isPro()?'800 MB':'400 MB');
 el('sizeBar').style.width=Math.min(100,total/maxBytes*100)+'%';
 el('sizeBar').style.background=over?'var(--danger)':'linear-gradient(90deg,#7898ff,#70e0d0)';
 el('percentLabel').textContent=over?'Limit exceeded':files.length?Math.min(100,total/maxBytes*100).toFixed(1)+'% of limit':'Ready when you are';
 el('createBtn').disabled=busy||!files.length||over||!window.fflate;
 el('createText').textContent=busy?'Creating your archive…':!window.fflate?'Compression library unavailable':'Create .zhip archive';
 el('clearBtn').disabled=busy;
 syncProUi();
 if(over){showMessage('Total input is '+formatBytes(total)+'. The input limit has been exceeded. Remove some files or unlock Pro to continue.','error');}
}
function addFiles(list){
 clearMessage();let added=0,skipped=0;
 for(const file of list){
  const path=file.webkitRelativePath||file.name;
  if(files.some(x=>x.path===path)){skipped++;continue;}
  if(totalSize()+file.size>currentMaxBytes()){skipped++;continue;}
  files.push({file,path});added++;
 }
 render();
 if(skipped)showMessage(skipped+' file(s) were skipped because they would exceed the 400 MB limit or were already selected.','error');
 else if(added)toast(added+' file(s) added');
}
el('chooseBtn').addEventListener('click',e=>{e.stopPropagation();fileInput.click();});
dropzone.addEventListener('click',e=>{if(e.target.closest('button'))return;fileInput.click();});
dropzone.addEventListener('keydown',e=>{if(e.key==='Enter'||e.key===' '){e.preventDefault();fileInput.click();}});
fileInput.addEventListener('change',()=>{addFiles(Array.from(fileInput.files||[]));fileInput.value='';});
folderInput.addEventListener('change',()=>{addFiles(Array.from(folderInput.files||[]));folderInput.value='';});
dropzone.addEventListener('dragover',e=>{e.preventDefault();dropzone.classList.add('dragover');});
dropzone.addEventListener('dragleave',e=>{if(!dropzone.contains(e.relatedTarget))dropzone.classList.remove('dragover');});
dropzone.addEventListener('drop',e=>{e.preventDefault();dropzone.classList.remove('dragover');if(e.dataTransfer.files)addFiles(Array.from(e.dataTransfer.files));});
el('clearBtn').addEventListener('click',()=>{if(busy)return;files.splice(0);clearMessage();render();});
el('archiveName').addEventListener('input',()=>{el('archiveName').setCustomValidity('');});
el('createBtn').addEventListener('click',async()=>{
 if(busy||!files.length)return;
 if(!window.fflate){showMessage('The compression library did not load. Check your internet connection and reload the page.','error');return;}
 const total=totalSize();if(total>currentMaxBytes()){showMessage('The total input exceeds your current plan limit.','error');return;}
 const archiveName=safeName(el('archiveName').value);
 const compressionMode=el('compression').value; const proJob=isPro()&&(total>BASE_MAX_BYTES||compressionMode==='ultimate'); const compressionRuns=compressionMode==='fast'?[{level:1}]:compressionMode==='smart'?[{level:1},{level:6}]:[{level:1},{level:6},{level:9}];
 if(compressionMode==='ultimate'&&!isPro()){showMessage('Ultimate ZSTD requires a valid Pro code.','error');return;}
 if((compressionMode==='ultimate'||(isPro()&&total>BASE_MAX_BYTES))&&!window.confirm('WARNING: Ultimate ZSTD uses very high CPU and memory. The archive can take a long time, the browser may run out of memory, and codec or device issues may produce a damaged archive. Original files are not changed. Continue?'))return;
 busy=true;clearMessage();render();addLog('create','Started archive creation: '+files.length+' file(s), '+formatBytes(total)+'.');
 const button=el('createBtn'),original=el('createText');
 try{
  showMessage('Preparing files for compression…','ok');
  const input={};
  for(let i=0;i<files.length;i++){
   const item=files[i];let key=item.path.replace(/^\/+|^\.\//,'');addLog('create','Reading '+item.path+' ('+formatBytes(item.file.size)+').');
   if(!key||key.split('/').some(part=>part==='..'))throw new Error('A file has an invalid path: '+item.path);
   if(Object.prototype.hasOwnProperty.call(input,key))key=i+'_'+key;
   input[key]=new Uint8Array(await item.file.arrayBuffer());
   original.textContent='Reading files… '+Math.round((i+1)/files.length*100)+'%';
  }
  const cacheItems=files.map(item=>({path:item.path,bytes:input[item.path.replace(/^\/+|^\.\//,'')]||input[item.path]}));
  const cacheKey=await archiveCacheKey(cacheItems,compressionMode);
  let cachedArchive=proJob?null:await localCacheGet(cacheKey);
  if(cachedArchive){addLog('create','Local CHC cache hit. Skipping compression.','success');}
  else if(!proJob){
   addLog('create','Local cache miss; checking shared Supabase cache.');
   cachedArchive=await cloudCacheGet(cacheKey);
   if(cachedArchive){addLog('create','Shared Supabase CHC cache hit.','success');await localCachePut(cacheKey,cachedArchive);}
  }
  if(cachedArchive){
   downloadArchive(cachedArchive,archiveName);
   showMessage('Cache hit! '+archiveName+'.zhip was restored from a verified content-keyed cache.','ok');
   addLog('create','Downloaded cached archive ('+formatBytes(cachedArchive.size)+').','success');
   toast('Cached archive is ready');
   return;
  }
  showMessage('Compressing '+files.length+' file(s)… the interface will stay responsive.','ok');
  addLog('create','Compression moved to a background Web Worker.');
  original.textContent='Compressing…';
  const workerFiles=files.map(item=>({path:item.path.replace(/^\/+|^\.\//,''),buffer:(input[item.path.replace(/^\/+|^\.\//,'')]||input[item.path]).buffer}));
  const worker=new Worker('compression-worker.js');
  const transfer=workerFiles.map(item=>item.buffer);
  const result=await new Promise((resolve,reject)=>{
   worker.onmessage=event=>{
    const data=event.data;
    if(data.type==='status'){showMessage(data.message,'ok');return;}
    if(data.type==='log'){addLog('create',data.message);return;}
    if(data.type==='progress'){
     addLog('create',data.message,'success');
     original.textContent='Compressing… '+Math.round(data.index/data.total*100)+'%';
     return;
    }
    if(data.type==='done'){resolve(data);return;}
    if(data.type==='error')reject(new Error(data.message));
   };
   worker.onerror=event=>reject(new Error(event.message||'Compression worker failed.'));
   worker.postMessage({files:workerFiles,mode:compressionMode},transfer);
  }).finally(()=>worker.terminate());
  const blob=new Blob([result.buffer],{type:'application/vnd.zhip'});
  downloadArchive(blob,archiveName);
  if(proJob){
   await removeLocalCache(cacheKey);
   await clearCloudCache(cacheKey);
   addLog('create','Pro job finished: removed this job’s local and cloud cache.','success');
  }else{
   await localCachePut(cacheKey,blob);
   addLog('create','Saved a local CHC cache entry with a 48-hour expiry.','success');
   await cloudCachePut(cacheKey,blob);
  }
  const archiveVersion=result.version;
  const ratio=total?Math.max(0,(1-blob.size/total)*100):0;

  showMessage('Done! '+archiveName+'.zhip was created ('+formatBytes(blob.size)+'). This archive uses the native ZHIP v'+archiveVersion+' container. It is not ZIP.','ok');
  addLog('create','Created '+archiveName+'.zhip (ZHIP v'+archiveVersion+', '+formatBytes(blob.size)+').','success');
  addLog('create',ratio>0?'Total size reduced by '+ratio.toFixed(1)+'%.':'No overall size reduction for this input.','info');
  toast('Your .zhip archive is ready');
 }catch(error){console.error(error);addLog('create','Failed: '+(error&&error.message?error.message:'Unknown error'),'error');showMessage('Could not create the archive: '+(error&&error.message?error.message:'Unknown error')+'. Your original files were not changed.','error');}
 finally{busy=false;render();if(el('message').textContent.startsWith('Done!')){el('createText').textContent='Create another .zhip archive';}}
});

const extractInput=el('extractInput'),extractDrop=el('extractDrop'),extractList=el('extractList');
let extractedEntries=[];
let currentArchiveName='MyArchive';
const convertZipBtn=el('convertZipBtn');
function clearExtract(){
 extractedEntries=[];extractInput.value='';extractList.replaceChildren();
 const empty=document.createElement('div');empty.className='empty-state';const icon=document.createElement('div');icon.className='empty-symbol';icon.textContent='↧';const copy=document.createElement('div');copy.textContent='Select a .zhip archive to view its contents.';empty.append(icon,copy);extractList.append(empty);
 el('extractSummary').textContent='No archive selected.';el('extractProgress').classList.add('hidden');el('extractProgressFill').style.width='0';el('extractMessage').className='message';el('extractMessage').textContent='';
}
function safeArchivePath(path){
 const normalized=path.replace(/\\/g,'/').replace(/^\.\//,'');
 return normalized&&!normalized.startsWith('/')&&!/^[a-zA-Z]:/.test(normalized)&&!normalized.split('/').some(part=>part==='..');
}
function renderExtracted(entries){
 extractedEntries=entries;extractList.replaceChildren();
 if(!entries.length){const empty=document.createElement('div');empty.className='empty-state';empty.textContent='No files were found in this archive.';extractList.append(empty);return;}
 entries.forEach((entry,index)=>{
  const row=document.createElement('div');row.className='extract-item';
  const icon=document.createElement('div');icon.className='file-type';icon.textContent='↓';
  const name=document.createElement('div');name.className='file-name';name.textContent=entry.name;name.title=entry.name;
  const size=document.createElement('div');size.className='file-size';size.textContent=formatBytes(entry.data.length);
  const link=document.createElement('a');link.href='#';link.textContent='Download';link.addEventListener('click',e=>{e.preventDefault();downloadExtracted(entry);});
  row.append(icon,name,size,link);extractList.append(row);
 });
}
function downloadExtracted(entry){
 const blob=new Blob([entry.data],{type:'application/octet-stream'});const url=URL.createObjectURL(blob);const a=document.createElement('a');a.href=url;a.download=entry.name.split('/').pop()||'extracted-file';document.body.append(a);a.click();a.remove();setTimeout(()=>URL.revokeObjectURL(url),30000);
}
async function readArchive(file){
 currentArchiveName=safeName(file.name.replace(/\.(zhip|zip)$/i,'')||'ConvertedArchive');
 convertZipBtn.classList.add('hidden');
 convertZipBtn.disabled=true;
 clearExtract();addLog('extract','Opening '+file.name+' ('+formatBytes(file.size)+').');
 el('extractSummary').textContent='Reading '+file.name+'…';
 el('extractProgress').classList.remove('hidden');el('extractProgressFill').style.width='20%';
 try{
  if(!window.fflate)throw new Error('Compression library is unavailable. Reload the page and try again.');
  const buffer=new Uint8Array(await file.arrayBuffer());
  if(buffer.length<9||buffer[0]!==90||buffer[1]!==72||buffer[2]!==73||buffer[3]!==80)throw new Error('Invalid ZHIP signature.');
  const version=buffer[4];if(version!==1&&version!==2&&version!==3&&version!==4)throw new Error('Unsupported ZHIP version: '+version);
  const view=new DataView(buffer.buffer,buffer.byteOffset,buffer.byteLength);
  const count=view.getUint32(5,true);
  if(count>100000)throw new Error('Archive contains an unreasonable number of entries.');
  let offset=9;
  const entries=[];
  let totalExpanded=0;
  el('extractProgressFill').style.width='35%';
  for(let i=0;i<count;i++){
   const fixedSize=version===1?14:15;
   if(offset+fixedSize>buffer.length)throw new Error('Archive is truncated in an entry header.');
   const nameLength=view.getUint16(offset,true);
   const originalLength=view.getUint32(offset+2,true);
   const compressedLength=view.getUint32(offset+6,true);
   const expectedCrc=view.getUint32(offset+10,true);
   const codec=version===1?1:buffer[offset+14];
   if(codec!==0&&codec!==1&&!(version===4&&codec===2))throw new Error('Unsupported compression codec '+codec+' in ZHIP v'+version+'.');
   offset+=fixedSize;
   if(nameLength===0||offset+nameLength+compressedLength>buffer.length)throw new Error('Invalid entry lengths or truncated archive.');
   const name=new TextDecoder('utf-8',{fatal:true}).decode(buffer.subarray(offset,offset+nameLength));
   offset+=nameLength;
   if(!safeArchivePath(name))throw new Error('Unsafe path rejected: '+name);
   const compressed=buffer.subarray(offset,offset+compressedLength);
   offset+=compressedLength;
   totalExpanded+=originalLength;
   if(totalExpanded>2*1024*1024*1024)throw new Error('Expanded data exceeds the 2 GiB safety limit.');
   const data=codec===0?compressed.slice():codec===2?(await ensureZstd()).decompress(compressed):fflate.inflateSync(compressed);
   if(data.length!==originalLength)throw new Error('Size verification failed for '+name);
   if(crc32(data)!==expectedCrc)throw new Error('Integrity check failed for '+name);
   if(!name.endsWith('/')){entries.push({name,data});addLog('extract','Read '+name+' ('+formatBytes(data.length)+').');}
   el('extractProgressFill').style.width=(35+Math.round((i+1)/Math.max(1,count)*60))+'%';
  }
  if(version===3){
   if(offset+8>buffer.length)throw new Error('This ZHIP v3 archive has no digital signature.');
   if(buffer[offset]!==83||buffer[offset+1]!==73||buffer[offset+2]!==71||buffer[offset+3]!==90)throw new Error('Digital signature trailer is missing.');
   const publicLength=view.getUint16(offset+4,true),signatureLength=view.getUint16(offset+6,true);
   const trailerLength=8+publicLength+signatureLength;
   if(publicLength===0||signatureLength===0||offset+trailerLength!==buffer.length)throw new Error('Invalid digital signature block.');
   const publicBytes=buffer.slice(offset+8,offset+8+publicLength);
   const signature=buffer.slice(offset+8+publicLength,offset+trailerLength);
   const publicKey=await crypto.subtle.importKey('spki',publicBytes,{name:'ECDSA',namedCurve:'P-256'},true,['verify']);
   const valid=await crypto.subtle.verify({name:'ECDSA',hash:'SHA-256'},publicKey,signature,buffer.subarray(0,offset));
   addLog('extract',valid?'Digital signature verified.':'Digital signature INVALID.',valid?'success':'error');
   if(!valid)throw new Error('Digital signature verification failed. The archive may have been changed.');
   const fingerprint=new Uint8Array(await crypto.subtle.digest('SHA-256',publicBytes));
   const fingerprintHex=Array.from(fingerprint,x=>x.toString(16).padStart(2,'0')).join('');
   el('extractSummary').dataset.signature='valid';
  }else if(offset!==buffer.length)throw new Error('Unexpected trailing data after ZHIP entries.');
  entries.sort((a,b)=>a.name.localeCompare(b.name));
  el('extractProgressFill').style.width='100%';
  renderExtracted(entries);
  convertZipBtn.classList.toggle('hidden',entries.length===0);
  convertZipBtn.disabled=entries.length===0;
  const bytes=entries.reduce((sum,item)=>sum+item.data.length,0);
  el('extractSummary').textContent=entries.length+' file(s) · '+formatBytes(bytes)+' extracted from '+file.name;
  const msg=el('extractMessage');msg.textContent=entries.length?'Archive opened successfully. Choose Download next to a file to save it.':'The archive contains no extractable files.';msg.className='message show '+(entries.length?'ok':'error');addLog('extract','Completed: '+entries.length+' file(s), '+formatBytes(bytes)+'.','success');
 }catch(error){
  console.error(error);addLog('extract','Failed: '+(error&&error.message?error.message:'Unknown error'),'error');el('extractSummary').textContent='Could not open archive.';
  const msg=el('extractMessage');msg.textContent='Unable to extract this file. It may be damaged, or it may not be a supported ZHIP v1 archive. '+(error&&error.message?error.message:'');msg.className='message show error';
  extractList.replaceChildren();
 }finally{el('extractProgress').classList.add('hidden');}
}
convertZipBtn.addEventListener('click',()=>{
 try{
  if(!extractedEntries.length)throw new Error('Open a valid ZHIP archive first.');
  const zipEntries={};
  for(const entry of extractedEntries)zipEntries[entry.name]=entry.data;
  const zipBytes=window.fflate.zipSync(zipEntries,{level:6});
  const blob=new Blob([zipBytes],{type:'application/zip'});
  const url=URL.createObjectURL(blob);
  const a=document.createElement('a');a.href=url;a.download=currentArchiveName+'.zip';document.body.append(a);a.click();a.remove();
  setTimeout(()=>URL.revokeObjectURL(url),30000);
  el('extractMessage').textContent='Converted '+extractedEntries.length+' file(s) from ZHIP to ZIP successfully.';
  el('extractMessage').className='message show ok';
  addLog('extract','Converted '+currentArchiveName+'.zhip to '+currentArchiveName+'.zip.','success');
 }catch(error){
  el('extractMessage').textContent='ZIP conversion failed: '+error.message;
  el('extractMessage').className='message show error';
  addLog('extract','ZIP conversion failed: '+error.message,'error');
 }
});
el('extractChoose').addEventListener('click',()=>extractInput.click());
el('extractClear').addEventListener('click',()=>{clearExtract();convertZipBtn.classList.add('hidden');convertZipBtn.disabled=true;});
extractInput.addEventListener('change',()=>{const file=extractInput.files&&extractInput.files[0];if(file)readArchive(file);});
extractDrop.addEventListener('dragover',e=>{e.preventDefault();extractDrop.style.borderColor='#87a5ff';});
extractDrop.addEventListener('dragleave',()=>{extractDrop.style.borderColor='';});
extractDrop.addEventListener('drop',e=>{e.preventDefault();extractDrop.style.borderColor='';const file=e.dataTransfer.files&&e.dataTransfer.files[0];if(file)readArchive(file);});


const themeToggle=el('themeToggle'),themeLabel=el('themeLabel');
function applyTheme(mode){
 if(mode==='auto'){document.documentElement.removeAttribute('data-theme');themeLabel.textContent='Auto';}
 else{document.documentElement.setAttribute('data-theme',mode);themeLabel.textContent=mode==='dark'?'Dark':'Light';}
 themeToggle.setAttribute('aria-label','Current theme: '+(mode==='auto'?'system':mode)+'. Click to change theme.');
 try{localStorage.setItem('zhipmanager-theme',mode);}catch(error){}
}
let themeMode='auto';
try{const savedTheme=localStorage.getItem('zhipmanager-theme');if(['auto','light','dark'].includes(savedTheme))themeMode=savedTheme;}catch(error){}
applyTheme(themeMode);
themeToggle.addEventListener('click',()=>{
 themeMode=themeMode==='auto'?'dark':themeMode==='dark'?'light':'auto';
 applyTheme(themeMode);
});


const glassInteractiveNodes=document.querySelectorAll('.choose-btn,.primary-btn,.secondary-btn,.select-wrap select,.theme-toggle,.local-pill,.limit-pill,.glass-select-trigger,.glass-select-option');
glassInteractiveNodes.forEach(node=>{
 node.addEventListener('pointermove',event=>{
  const bounds=node.getBoundingClientRect();
  node.style.setProperty('--pointer-x',((event.clientX-bounds.left)/Math.max(1,bounds.width)*100)+'%');
  node.style.setProperty('--pointer-y',((event.clientY-bounds.top)/Math.max(1,bounds.height)*100)+'%');
 });
 node.addEventListener('pointerleave',()=>{
  node.style.setProperty('--pointer-x','50%');
  node.style.setProperty('--pointer-y','0%');
 });
});

const jellyButtons=document.querySelectorAll('.choose-btn,.primary-btn,.secondary-btn,.theme-toggle,.glass-select-trigger,.glass-select-option');
jellyButtons.forEach(button=>{
 button.addEventListener('click',()=>{
  if(button.disabled)return;
  button.classList.remove('jelly-trigger');
  void button.offsetWidth;
  button.classList.add('jelly-trigger');
 });
 button.addEventListener('animationend',event=>{
  if(event.animationName==='jellyPress')button.classList.remove('jelly-trigger');
 });
});


const compressionPicker=el('compressionPicker');
const compressionTrigger=el('compressionTrigger');
const compressionLabel=el('compressionLabel');
const compressionOptions=el('compressionOptions');
const compressionSelect=el('compression');
const compressionOptionButtons=Array.from(compressionOptions.querySelectorAll('[role="option"]'));
function setCompressionMenu(open){
 compressionPicker.classList.toggle('is-open',open);
 compressionOptions.classList.toggle('hidden',!open);
 compressionTrigger.setAttribute('aria-expanded',String(open));
}
function syncCompressionValue(){
 const selected=compressionOptionButtons.find(option=>option.dataset.value===compressionSelect.value)||compressionOptionButtons[1];
 compressionLabel.textContent=selected.querySelector('span').textContent;
 compressionOptionButtons.forEach(option=>{
  const isSelected=option===selected;
  option.classList.toggle('is-selected',isSelected);
  option.setAttribute('aria-selected',String(isSelected));
 });
}
compressionTrigger.addEventListener('click',()=>{
 const open=compressionTrigger.getAttribute('aria-expanded')!=='true';
 setCompressionMenu(open);
 if(open){
  const selected=compressionOptionButtons.find(option=>option.dataset.value===compressionSelect.value);
  (selected||compressionOptionButtons[0]).focus();
 }
});
compressionOptionButtons.forEach((option,index)=>{
 option.addEventListener('click',()=>{
  compressionSelect.value=option.dataset.value;
  compressionSelect.dispatchEvent(new Event('change',{bubbles:true}));
  syncCompressionValue();
  setCompressionMenu(false);
  compressionTrigger.focus();
 });
 option.addEventListener('keydown',event=>{
  let next=index;
  if(event.key==='ArrowDown')next=(index+1)%compressionOptionButtons.length;
  else if(event.key==='ArrowUp')next=(index-1+compressionOptionButtons.length)%compressionOptionButtons.length;
  else if(event.key==='Home')next=0;
  else if(event.key==='End')next=compressionOptionButtons.length-1;
  else if(event.key==='Escape'){event.preventDefault();setCompressionMenu(false);compressionTrigger.focus();return;}
  else if(event.key==='Tab'){setCompressionMenu(false);return;}
  else return;
  event.preventDefault();
  compressionOptionButtons[next].focus();
 });
});
compressionTrigger.addEventListener('keydown',event=>{
 if(event.key==='ArrowDown'||event.key==='ArrowUp'){
  event.preventDefault();
  setCompressionMenu(true);
  const selected=compressionOptionButtons.find(option=>option.dataset.value===compressionSelect.value);
  const index=Math.max(0,compressionOptionButtons.indexOf(selected));
  const next=event.key==='ArrowDown'?Math.min(index+1,compressionOptionButtons.length-1):Math.max(index-1,0);
  compressionOptionButtons[next].focus();
 }
 if(event.key==='Escape')setCompressionMenu(false);
});
document.addEventListener('pointerdown',event=>{
 if(!compressionPicker.contains(event.target))setCompressionMenu(false);
});
document.addEventListener('focusin',event=>{
 if(!compressionPicker.contains(event.target))setCompressionMenu(false);
});
syncCompressionValue();

el('proCodeBtn').addEventListener('click',async()=>{
 if(isPro()){toast('Pro is already unlocked for this tab');return;}
 const code=window.prompt('Enter your Pro code to unlock 800 MB and Ultimate ZSTD:');
 if(code===null||!code.trim())return;
 el('proCodeBtn').disabled=true;
 try{
  const valid=await validateProCode(code);
  if(!valid){showMessage('Invalid Pro code. Please check the code and try again.','error');return;}
  proUnlocked=true;
  syncProUi();render();
  showMessage('Pro unlocked for this tab: 800 MB and Ultimate ZSTD are now available.','ok');
  addLog('create','Pro plan unlocked for this browser tab.','success');
 }catch(error){showMessage('Could not validate the Pro code. Check your connection and try again.','error');}
 finally{el('proCodeBtn').disabled=false;}
});
el('compression').addEventListener('change',()=>{
 const mode=el('compression').value;
 el('proWarning').classList.toggle('show',mode==='ultimate');
 if(mode==='ultimate'&&!isPro()){showMessage('Ultimate ZSTD is a Pro feature. Enter your Pro code first.','error');el('compression').value='smart';el('proWarning').classList.remove('show');syncCompressionValue();}
});

render();
})();