// One minute of public reference audio through Windows speakers, no microphone.
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path');
(async()=>{
 const browser=await chromium.connectOverCDP('http://127.0.0.1:9223');
 const page=browser.contexts().flatMap(c=>c.pages()).find(p=>p.url().endsWith(':1420/'));assert(page);
 const api=(cmd,args)=>page.evaluate(({cmd,args})=>window.__TAURI_INTERNALS__.invoke(cmd,args),{cmd,args});
 const wait=async(fn,ms=12000)=>{const end=Date.now()+ms;while(Date.now()<end){if(await fn())return;await page.waitForTimeout(100);}throw Error('Local soak timeout');};
 assert(!(await api('snapshot')).active_id&&!await api('caption_running'));
 const visible=await api('caption_window_visible');let result;
 try{
  await page.evaluate(async()=>{const{listen}=await import('/node_modules/@tauri-apps/api/event.js');window.localSoak={texts:[],errors:[]};window.localSoakOff=[await listen('caption-text',({payload})=>window.localSoak.texts.push(payload)),await listen('caption-status',({payload})=>{if(payload.state==='error')window.localSoak.errors.push(payload.message);})];});
  await api('start_caption',{options:{provider:'',apiKey:'',baseUrl:'',profileId:'__local__',model:'nemotron-3.5-q8',sourceLanguage:'en',targetLanguage:'',translate:false,mode:'standalone',device:null,sessionDir:null}});
  await wait(async()=>{const s=(await api('caption_snapshot')).status;assert.notEqual(s.state,'error',s.message);return s.state==='active';});
  await page.evaluate(async file=>{const{convertFileSrc}=await import('/node_modules/@tauri-apps/api/core.js');window.localSoakAudio=new Audio(convertFileSrc(file));window.localSoakAudio.loop=true;await window.localSoakAudio.play();},path.resolve('test-results/fixtures/realtime/reference.wav'));
  const start=Date.now();
  while(Date.now()-start<60000){await page.waitForTimeout(1000);const s=(await api('caption_snapshot')).status;assert.notEqual(s.state,'error',s.message);}
  await page.evaluate(()=>window.localSoakAudio.pause());const beforeStop=(await api('caption_snapshot')).status;
  const stopped=Date.now();await api('stop_caption');await wait(async()=>!await api('caption_running'));
  const afterStop=(await api('caption_snapshot')).status;
  const audit=await page.evaluate(()=>window.localSoak);
  result={durationMs:Date.now()-start,stopMs:Date.now()-stopped,beforeStop,afterStop,events:audit.texts.length,finals:audit.texts.filter(t=>t.final_).map(t=>t.text),errors:audit.errors};
  assert.equal(beforeStop.state,'active');assert.equal(afterStop.state,'idle',afterStop.message);assert.equal(audit.errors.length,0);assert(result.finals.filter(t=>t.includes('meeting')).length>=3,'Lost later utterances during continuous decoding');
 }finally{
  await page.evaluate(()=>{window.localSoakAudio?.pause();window.localSoakOff?.forEach(off=>off());}).catch(()=>{});
  await api('stop_caption');await wait(async()=>!await api('caption_running'));
  if(visible)await api('open_caption_window');else await api('close_caption_window');
  if(result){fs.writeFileSync('test-results/realtime-performance/local-soak.json',JSON.stringify(result,null,2));console.log(JSON.stringify(result));}
  await browser.close();
 }
})().catch(error=>{console.error(error);process.exitCode=1});
