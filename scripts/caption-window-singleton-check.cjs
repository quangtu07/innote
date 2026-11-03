// Backend/native lifecycle regression. No audio capture and no provider call.
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict');
(async()=>{
 const browser=await chromium.connectOverCDP('http://127.0.0.1:9223');
 try{
  const pages=()=>browser.contexts().flatMap(c=>c.pages());
  const page=pages().find(p=>p.url().endsWith(':1420/'));assert(page);
  const api=(cmd)=>page.evaluate(cmd=>window.__TAURI_INTERNALS__.invoke(cmd),cmd);
  assert(!(await api('snapshot')).active_id&&!await api('caption_running'),'Never interrupt a recording');
  const phase=process.argv[2]||'show';
  if(phase==='hide'){
   await Promise.all(Array.from({length:24},()=>api('close_caption_window')));
   assert.equal(await api('caption_window_visible'),false);
  }else{
   if(phase==='cycle')for(let cycle=0;cycle<12;cycle++){
    await Promise.all(Array.from({length:8},()=>api('open_caption_window')));
    await Promise.all(Array.from({length:8},()=>api(cycle%2?'hide_caption_window':'close_caption_window')));
    assert.equal(await api('caption_window_visible'),false);
   }
   await Promise.all(Array.from({length:24},()=>api('open_caption_window')));
   await page.waitForTimeout(500);
   assert.equal(await api('caption_window_visible'),true);
  }
  const captions=pages().filter(p=>p.url().includes('/caption.html'));
  assert.equal(captions.length,1,'One retained caption WebView, including after concurrent calls and close/reopen');
  assert.equal(await api('caption_running'),false,'Preview never uploads audio');
  console.log(JSON.stringify({phase,captionWebviews:captions.length,visible:await api('caption_window_visible'),running:false,passed:true}));
 }finally{await browser.close();}
})().catch(e=>{console.error(e);process.exitCode=1});
