// Native local-caption lifecycle; no key, microphone, or user meeting involved.
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict'),fs=require('node:fs');
(async()=>{
 const browser=await chromium.connectOverCDP('http://127.0.0.1:9223');
 const page=browser.contexts().flatMap(c=>c.pages()).find(p=>p.url().endsWith(':1420/'));assert(page);
 const api=(cmd,args)=>page.evaluate(({cmd,args})=>window.__TAURI_INTERNALS__.invoke(cmd,args),{cmd,args});
 const wait=async predicate=>{const end=Date.now()+12000;while(Date.now()<end){if(await predicate())return;await page.waitForTimeout(50);}throw Error('Local lifecycle timeout');};
 const initial=await api('snapshot');assert(!initial.active_id&&!await api('caption_running'));
 const visible=await api('caption_window_visible'),results=[];
 const options={provider:'',apiKey:'',baseUrl:'',profileId:'__local__',model:'nemotron-3.5-q8',sourceLanguage:'en',translate:false,targetLanguage:'',mode:'standalone',device:null,sessionDir:null};
 try{
  for(const phase of ['cancel-loading','stop-ready','restart']){
   await api('start_caption',{options});
   if(phase!=='cancel-loading')await wait(async()=>{const s=(await api('caption_snapshot')).status;assert.notEqual(s.state,'error',s.message);return s.state==='active';});
   if(phase==='restart')await page.waitForTimeout(800);
   const started=Date.now();await api('stop_caption');await wait(async()=>!await api('caption_running'));
   const status=(await api('caption_snapshot')).status;assert.equal(status.state,'idle',status.message);
   results.push({phase,stopMs:Date.now()-started,state:status.state});
  }
  const after=await api('snapshot');assert.equal(after.sessions.length,initial.sessions.length);assert(!after.active_id);
 }finally{
  await api('stop_caption');await wait(async()=>!await api('caption_running'));
  if(visible)await api('open_caption_window');else await api('close_caption_window');
  fs.mkdirSync('test-results/realtime-performance',{recursive:true});fs.writeFileSync('test-results/realtime-performance/local-lifecycle.json',JSON.stringify(results,null,2));
  await browser.close();
 }
 console.log(JSON.stringify(results));
})().catch(error=>{console.error(error);process.exitCode=1});
