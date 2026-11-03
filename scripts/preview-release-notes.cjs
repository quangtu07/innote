// Connect to the main WebView target directly; some CDP clients cannot attach
// to its shared workers. Preview never changes real upgrade dismissal state.
const fs=require('node:fs'),assert=require('node:assert/strict');
(async()=>{
 const tabs=await(await fetch('http://127.0.0.1:9223/json/list')).json();
 const target=tabs.find(t=>t.type==='page'&&t.url.endsWith(':1420/'));assert(target);
 const socket=new WebSocket(target.webSocketDebuggerUrl),pending=new Map();let id=0;
 await new Promise((resolve,reject)=>{socket.onopen=resolve;socket.onerror=reject;});
 socket.onmessage=e=>{const message=JSON.parse(e.data);const promise=pending.get(message.id);if(promise){pending.delete(message.id);message.error?promise.reject(Error(message.error.message)):promise.resolve(message.result);}};
 const call=(method,params={})=>new Promise((resolve,reject)=>{const next=++id;pending.set(next,{resolve,reject});socket.send(JSON.stringify({id:next,method,params}));});
 const evaluate=async expression=>{const result=await call('Runtime.evaluate',{expression,awaitPromise:true,returnByValue:true});if(result.exceptionDetails)throw Error(result.exceptionDetails.text);return result.result.value;};
 try{
  const idle=await evaluate(`(async()=>{const invoke=window.__TAURI_INTERNALS__.invoke;return !(await invoke('snapshot')).active_id&&!await invoke('caption_running');})()`);
  if(!idle){console.log('Preview deferred: a user session is active. Isolated screenshots are available for review.');return;}
  await evaluate(`window.dispatchEvent(new Event('innote:preview-whats-new'))`);
  const bounds=await evaluate(`(async()=>{for(let i=0;i<30;i++){const dialog=document.querySelector('dialog.whats-new[open]');if(dialog){const b=dialog.getBoundingClientRect();return {x:b.x,y:b.y,width:b.width,height:b.height,scale:1};}await new Promise(resolve=>setTimeout(resolve,100));}throw Error('Preview dialog did not open');})()`);
  const screenshot=await call('Page.captureScreenshot',{format:'png',clip:bounds});
  fs.mkdirSync('test-results/release-notice',{recursive:true});fs.writeFileSync('test-results/release-notice/desktop-preview.png',Buffer.from(screenshot.data,'base64'));
  console.log('Development release-notes preview opened; no version change, desktop package, push or publication.');
 }finally{socket.close();}
})().catch(e=>{console.error(e);process.exitCode=1});
