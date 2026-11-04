// Run an older production client against the published endpoint. Downloads and
// verifies through Tauri's updater, then frees resources; never installs/exits.
const fs=require('node:fs'),assert=require('node:assert/strict');
const expectedVersion=process.argv[2]||JSON.parse(fs.readFileSync('package.json','utf8')).version;
const expectedCurrent=process.argv[3]||'0.8.4';
(async()=>{
 const tabs=await(await fetch('http://127.0.0.1:9223/json/list')).json();
 const target=tabs.find(t=>t.type==='page'&&/^https?:\/\/tauri\.localhost\/?$/.test(t.url));assert(target);
 const socket=new WebSocket(target.webSocketDebuggerUrl),pending=new Map();let id=0;
 await new Promise((resolve,reject)=>{socket.onopen=resolve;socket.onerror=reject;});
 socket.onmessage=e=>{const message=JSON.parse(e.data);const promise=pending.get(message.id);if(promise){pending.delete(message.id);message.error?promise.reject(Error(message.error.message)):promise.resolve(message.result);}};
 const call=(method,params)=>new Promise((resolve,reject)=>{const next=++id;pending.set(next,{resolve,reject});socket.send(JSON.stringify({id:next,method,params}));});
 try{
  const result=await call('Runtime.evaluate',{awaitPromise:true,returnByValue:true,expression:`(async()=>{
   const api=window.__TAURI_INTERNALS__;
   if((await api.invoke('snapshot')).active_id||await api.invoke('caption_running'))throw Error('User session active');
   const update=await api.invoke('plugin:updater|check',{timeout:30000});
   if(!update||update.version!==${JSON.stringify(expectedVersion)}||update.currentVersion!==${JSON.stringify(expectedCurrent)})throw Error('Older client did not find the expected update');
   let bytesRid;const callback=api.transformCallback(()=>{});
   try{
    bytesRid=await api.invoke('plugin:updater|download',{rid:update.rid,onEvent:'__CHANNEL__:'+callback,timeout:120000});
    return {currentVersion:update.currentVersion,version:update.version,downloadVerified:true,installationTriggered:false,passed:true};
   }finally{
    if(bytesRid!==undefined)await api.invoke('plugin:resources|close',{rid:bytesRid});
    await api.invoke('plugin:resources|close',{rid:update.rid});
    api.unregisterCallback(callback);
   }
  })()`});
  if(result.exceptionDetails)throw Error(result.exceptionDetails.exception?.description||result.exceptionDetails.text);
  fs.writeFileSync('test-results/release-'+expectedVersion+'-old-client-updater.json',JSON.stringify(result.result.value,null,2));console.log(JSON.stringify(result.result.value));
 }finally{socket.close();}
})().catch(e=>{console.error(e);process.exitCode=1});
