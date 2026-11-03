// Uses saved profile IDs through Rust. Does not read, decrypt or print API keys.
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict'),fs=require('node:fs');
(async()=>{
 const browser=await chromium.connectOverCDP('http://127.0.0.1:9223');
 try{
  const page=browser.contexts().flatMap(c=>c.pages()).find(p=>p.url().endsWith(':1420/'));assert(page);
  const results=await page.evaluate(async()=>{
   const invoke=window.__TAURI_INTERNALS__.invoke;
   if((await invoke('snapshot')).active_id||await invoke('caption_running'))throw Error('User session active');
   const profiles=await invoke('list_ai_profiles'),results=[];
   for(const provider of ['soniox','groq']){
    const p=profiles.find(p=>p.provider===provider&&p.has_key);if(!p){results.push({provider,skipped:'No saved key'});continue;}
    const start=performance.now();
    const result=await invoke('test_ai_connection',{input:{provider,profileId:p.id,apiKey:'',model:'',baseUrl:p.base_url}});
    // This command resolves to a human-readable success string and rejects on
    // failure; it intentionally distinguishes key-only vs model validation.
    results.push({provider,keyOnly:true,passed:typeof result==='string'&&result.length>0,elapsedMs:Math.round(performance.now()-start)});
   }
   return results;
  });
  assert(results.every(r=>r.passed||r.skipped));
  fs.writeFileSync('test-results/final-review-saved-profiles.json',JSON.stringify(results,null,2));console.log(JSON.stringify(results));
 }finally{await browser.close();}
})().catch(e=>{console.error(e);process.exitCode=1});
