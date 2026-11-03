// Owned speaker fixture only. Never extracts credentials; audit is opt-in in Rust.
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
const {spawn}=require('node:child_process');
const fs=require('node:fs'),assert=require('node:assert/strict');
(async()=>{
 const browser=await chromium.connectOverCDP('http://127.0.0.1:9223');
 const main=browser.contexts().flatMap(c=>c.pages()).find(p=>p.url().endsWith(':1420/'));assert(main);
 try{
  await main.evaluate(async()=>{
   const invoke=window.__TAURI_INTERNALS__.invoke;
   if((await invoke('snapshot')).active_id||await invoke('caption_running'))throw Error('User session active');
   const {listen}=await import('/node_modules/@tauri-apps/api/event.js');
   window.groqBoundaryAudit=[];
   window.groqBoundaryOff=await listen('groq-caption-audit',({payload})=>window.groqBoundaryAudit.push(payload));
  });
  await new Promise((resolve,reject)=>{
   const child=spawn(process.execPath,['scripts/realtime-benchmark.cjs'],{env:{...process.env,BENCH_PROVIDER:'groq',BENCH_LEVEL:'accurate',BENCH_TRIALS:'1',BENCH_MODES:'source',BENCH_LABEL:'final-review-groq-audit'},stdio:'ignore'});
   child.on('error',reject);child.on('exit',code=>code===0?resolve():reject(Error('Fixture benchmark failed '+code)));
  });
  const requests=await main.evaluate(()=>window.groqBoundaryAudit);
  assert(requests.length,'Launch the debug app with INNOTE_GROQ_AUDIT=1 first');
  fs.writeFileSync('test-results/final-review-groq-audit.json',JSON.stringify(requests,null,2));
  console.log(JSON.stringify({requests:requests.length,fixtureOnly:true,credentialsExtracted:false}));
 }finally{
  await main.evaluate(()=>{window.groqBoundaryOff?.();delete window.groqBoundaryAudit;delete window.groqBoundaryOff;}).catch(()=>{});
  await browser.close();
 }
})().catch(e=>{console.error(e);process.exitCode=1});
