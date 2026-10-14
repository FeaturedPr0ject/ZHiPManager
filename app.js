(function(){
'use strict';
const MAX_BYTES=400*1024*1024;
const files=[];
const el=id=>document.getElementById(id);
const dropzone=el('dropzone'),fileInput=el('fileInput'),folderInput=el('folderInput'),fileList=el('fileList');
let busy=false;
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
 const total=totalSize(),over=total>MAX_BYTES;
 el('fileCount').textContent=files.length+' file'+(files.length===1?'':'s');
 el('sizeLabel').textContent=formatBytes(total)+' / 400 MB';
 el('sizeBar').style.width=Math.min(100,total/MAX_BYTES*100)+'%';
 el('sizeBar').style.background=over?'var(--danger)':'linear-gradient(90deg,#7898ff,#70e0d0)';
 el('percentLabel').textContent=over?'Limit exceeded':files.length?Math.min(100,total/MAX_BYTES*100).toFixed(1)+'% of limit':'Ready when you are';
 el('createBtn').disabled=busy||!files.length||over||!window.fflate;
 el('createText').textContent=busy?'Creating your archive…':!window.fflate?'Compression library unavailable':'Create .zhip archive';
 el('clearBtn').disabled=busy;
 if(over){showMessage('Total input is '+formatBytes(total)+'. The 400 MB limit has been exceeded. Remove some files to continue.','error');}
}
function addFiles(list){
 clearMessage();let added=0,skipped=0;
 for(const file of list){
  const path=file.webkitRelativePath||file.name;
  if(files.some(x=>x.path===path)){skipped++;continue;}
  if(totalSize()+file.size>MAX_BYTES){skipped++;continue;}
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
 const total=totalSize();if(total>MAX_BYTES){showMessage('The total input must not exceed 400 MB.','error');return;}
 const archiveName=safeName(el('archiveName').value);
 const compressionMode=el('compression').value; const levels=compressionMode==='fast'?[1]:compressionMode==='9'?[9]:[1,6,9];
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
  showMessage('Compressing '+files.length+' file(s)… keep this tab open.','ok');addLog('create','Compression started.');
  original.textContent='Compressing…';
  await new Promise(resolve=>setTimeout(resolve,30));
  const encoder=new TextEncoder();
  const chunks=[];
  const header=new Uint8Array(9);
  header.set([90,72,73,80,2],0);
  new DataView(header.buffer).setUint32(5,files.length,true);
  chunks.push(header);
  let archiveBytes=header.length;
  for(let i=0;i<files.length;i++){
   const item=files[i];
   let path=item.path.replace(/^\/+|^\.\//,'');
   if(!path||!safeArchivePath(path))throw new Error('Invalid file path: '+item.path);
   const nameBytes=encoder.encode(path);
   if(nameBytes.length>65535)throw new Error('File path is too long: '+path);
   const raw=input[item.path.replace(/^\/+|^\.\//,'')]||input[path];
   let best=raw; let codec=0;
   for(const candidateLevel of levels){const candidate=fflate.deflateSync(raw,{level:candidateLevel});if(candidate.length<best.length){best=candidate;codec=1;}}
   const record=new Uint8Array(15+nameBytes.length);
   const view=new DataView(record.buffer);
   view.setUint16(0,nameBytes.length,true);
   view.setUint32(2,raw.length,true);
   view.setUint32(6,best.length,true);
   view.setUint32(10,crc32(raw),true);
   record[14]=codec;
   record.set(nameBytes,15);
   chunks.push(record,best);
   archiveBytes+=record.length+best.length;
   addLog('create','Processed '+path+': '+formatBytes(raw.length)+' → '+formatBytes(best.length)+'.');
   original.textContent='Compressing… '+Math.round((i+1)/files.length*100)+'%';
  }
  const blob=new Blob(chunks,{type:'application/vnd.zhip'});
  const url=URL.createObjectURL(blob);const a=document.createElement('a');a.href=url;a.download=archiveName+'.zhip';document.body.append(a);a.click();a.remove();setTimeout(()=>URL.revokeObjectURL(url),30000);
  const ratio=total?Math.max(0,(1-blob.size/total)*100):0;
  showMessage('Done! '+archiveName+'.zhip was created ('+formatBytes(blob.size)+'). This archive uses the native ZHIP v2 hybrid container. It is not ZIP.','ok');
  addLog('create','Created '+archiveName+'.zhip ('+formatBytes(blob.size)+').','success');
  addLog('create',ratio>0?'Total size reduced by '+ratio.toFixed(1)+'%.':'No overall size reduction for this input.','info');
  toast('Your .zhip archive is ready');
 }catch(error){console.error(error);addLog('create','Failed: '+(error&&error.message?error.message:'Unknown error'),'error');showMessage('Could not create the archive: '+(error&&error.message?error.message:'Unknown error')+'. Your original files were not changed.','error');}
 finally{busy=false;render();if(el('message').textContent.startsWith('Done!')){el('createText').textContent='Create another .zhip archive';}}
});

const extractInput=el('extractInput'),extractDrop=el('extractDrop'),extractList=el('extractList');
let extractedEntries=[];
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
 clearExtract();addLog('extract','Opening '+file.name+' ('+formatBytes(file.size)+').');
 el('extractSummary').textContent='Reading '+file.name+'…';
 el('extractProgress').classList.remove('hidden');el('extractProgressFill').style.width='20%';
 try{
  if(!window.fflate)throw new Error('Compression library is unavailable. Reload the page and try again.');
  const buffer=new Uint8Array(await file.arrayBuffer());
  if(buffer.length<9||buffer[0]!==90||buffer[1]!==72||buffer[2]!==73||buffer[3]!==80)throw new Error('Invalid ZHIP signature.');
  const version=buffer[4];if(version!==1&&version!==2)throw new Error('Unsupported ZHIP version: '+version);
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
   if(version===2&&codec!==0&&codec!==1)throw new Error('Unsupported compression codec: '+codec);
   offset+=fixedSize;
   if(nameLength===0||offset+nameLength+compressedLength>buffer.length)throw new Error('Invalid entry lengths or truncated archive.');
   const name=new TextDecoder('utf-8',{fatal:true}).decode(buffer.subarray(offset,offset+nameLength));
   offset+=nameLength;
   if(!safeArchivePath(name))throw new Error('Unsafe path rejected: '+name);
   const compressed=buffer.subarray(offset,offset+compressedLength);
   offset+=compressedLength;
   totalExpanded+=originalLength;
   if(totalExpanded>2*1024*1024*1024)throw new Error('Expanded data exceeds the 2 GiB safety limit.');
   const data=codec===0?compressed.slice():fflate.inflateSync(compressed);
   if(data.length!==originalLength)throw new Error('Size verification failed for '+name);
   if(crc32(data)!==expectedCrc)throw new Error('Integrity check failed for '+name);
   if(!name.endsWith('/')){entries.push({name,data});addLog('extract','Read '+name+' ('+formatBytes(data.length)+').');}
   el('extractProgressFill').style.width=(35+Math.round((i+1)/Math.max(1,count)*60))+'%';
  }
  if(offset!==buffer.length)throw new Error('Unexpected trailing data after ZHIP entries.');
  entries.sort((a,b)=>a.name.localeCompare(b.name));
  el('extractProgressFill').style.width='100%';
  renderExtracted(entries);
  const bytes=entries.reduce((sum,item)=>sum+item.data.length,0);
  el('extractSummary').textContent=entries.length+' file(s) · '+formatBytes(bytes)+' extracted from '+file.name;
  const msg=el('extractMessage');msg.textContent=entries.length?'Archive opened successfully. Choose Download next to a file to save it.':'The archive contains no extractable files.';msg.className='message show '+(entries.length?'ok':'error');addLog('extract','Completed: '+entries.length+' file(s), '+formatBytes(bytes)+'.','success');
 }catch(error){
  console.error(error);addLog('extract','Failed: '+(error&&error.message?error.message:'Unknown error'),'error');el('extractSummary').textContent='Could not open archive.';
  const msg=el('extractMessage');msg.textContent='Unable to extract this file. It may be damaged, or it may not be a supported ZHIP v1 archive. '+(error&&error.message?error.message:'');msg.className='message show error';
  extractList.replaceChildren();
 }finally{el('extractProgress').classList.add('hidden');}
}
el('extractChoose').addEventListener('click',()=>extractInput.click());
el('extractClear').addEventListener('click',clearExtract);
extractInput.addEventListener('change',()=>{const file=extractInput.files&&extractInput.files[0];if(file)readArchive(file);});
extractDrop.addEventListener('dragover',e=>{e.preventDefault();extractDrop.style.borderColor='#87a5ff';});
extractDrop.addEventListener('dragleave',()=>{extractDrop.style.borderColor='';});
extractDrop.addEventListener('drop',e=>{e.preventDefault();extractDrop.style.borderColor='';const file=e.dataTransfer.files&&e.dataTransfer.files[0];if(file)readArchive(file);});

render();
})();