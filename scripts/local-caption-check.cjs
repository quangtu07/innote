// Real loopback/local ASR, reference audio only. No API key or meeting is used.
const{chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path'),os=require('node:os');
(async()=>{
 const browser=await chromium.connectOverCDP('http://127.0.0.1:9223');
 const page=browser.contexts().flatMap(c=>c.pages()).find(p=>p.url().endsWith(':1420/'));assert(page);
 const api=(cmd,args)=>page.evaluate(({cmd,args})=>window.__TAURI_INTERNALS__.invoke(cmd,args),{cmd,args});
 const wait=async fn=>{const end=Date.now()+90000;while(Date.now()<end){if(await fn())return;await page.waitForTimeout(200);}throw Error('Timed out local captions');};
 const initial=await api('snapshot');assert(!initial.active_id&&!await api('caption_running'));
 const saved=await page.evaluate(()=>localStorage.getItem('may.caption.standalone'));
 const before=new Set(fs.readdirSync(os.tmpdir()).filter(n=>/^innote-caption-.*\.wav$/.test(n)));
 const results=[];fs.mkdirSync('test-results/local-caption-native',{recursive:true});
 try{
  for(const[model,language,file,terms]of [['moonshine-vi','vi','vietnamese-pcm.wav',['động vật','quan sát']],['moonshine-en','en','english.wav',['engineering','friday','million']],['nemotron-3.5-q8','en','english.wav',['engineering','friday','million']]]){
   if(process.env.LOCAL_MODEL&&model!==process.env.LOCAL_MODEL)continue;
   assert(initial.models.find(m=>m.id===model)?.installed,'Install the local model first');
   await page.evaluate(async()=>{window.localText=[];window.localStatuses=[];const{listen}=await import('/node_modules/@tauri-apps/api/event.js');window.localOff=[await listen('caption-text',({payload})=>{if(payload.text.trim())window.localText.push({at:Date.now(),...payload})}),await listen('caption-status',({payload})=>window.localStatuses.push(payload))];});
   await page.getByRole('button',{name:'Phụ đề',exact:true}).click();await page.getByLabel('Dịch sang',{exact:true}).uncheck();await page.getByLabel('Ngôn ngữ gốc').selectOption(language);await page.getByLabel('Cấu hình AI phụ đề').selectOption('__local__');await page.getByLabel('Model phụ đề').selectOption(model);
   await page.screenshot({path:`test-results/local-caption-native/${model}-setup.png`});
   await page.getByRole('button',{name:'Bắt đầu phụ đề',exact:true}).click();await wait(async()=>await api('caption_running'));
   await wait(async()=>['active','error'].includes((await api('caption_snapshot')).status.state));assert.equal((await api('caption_snapshot')).status.state,'active');
   const started=Date.now();await page.evaluate(async file=>{const{convertFileSrc}=await import('/node_modules/@tauri-apps/api/core.js');window.localAudio=new Audio(convertFileSrc(file));await window.localAudio.play();await new Promise((resolve,reject)=>{window.localAudio.onended=resolve;window.localAudio.onerror=reject;});},path.resolve('test-results/fixtures',file));
   await wait(async()=>await page.evaluate(()=>window.localText.length>0||window.localStatuses.some(s=>s.state==='error')));
   const tailDeadline=Date.now()+30000;
   while(Date.now()<tailDeadline && await page.evaluate(()=>window.localText.length<2&&!window.localStatuses.some(s=>s.state==='error')))await page.waitForTimeout(200);
   const audit=await page.evaluate(()=>({text:window.localText,statuses:window.localStatuses}));
   fs.writeFileSync(`test-results/local-caption-native/${model}-audit.json`,JSON.stringify(audit,null,2));
   assert(audit.text.length,JSON.stringify(audit.statuses));assert(!audit.statuses.some(s=>s.sending||s.state==='error'),JSON.stringify(audit.statuses));
   const overlay=browser.contexts().flatMap(c=>c.pages()).filter(p=>p.url().includes('caption.html')).at(-1);assert(overlay);assert((await overlay.locator('.caption-root').innerText()).trim());
   await overlay.screenshot({path:`test-results/local-caption-native/${model}.png`});
   const stopAt=Date.now();await api('stop_caption');await wait(async()=>!await api('caption_running'));await api('close_caption_window');
   await page.evaluate(()=>window.localOff.forEach(fn=>fn()));
   results.push({model,language,firstTextMs:audit.text[0].at-started,stopMs:Date.now()-stopAt,text:audit.text.map(e=>e.text).join(' '),missingReferenceTerms:terms.filter(t=>!audit.text.map(e=>e.text).join(' ').toLowerCase().includes(t)),noCloud:audit.statuses.every(s=>s.provider==='local'&&!s.sending),noMeeting:(await api('snapshot')).sessions.length===initial.sessions.length,passed:true});
  }
 }finally{
  fs.writeFileSync('test-results/local-caption-native/last-audit.json',JSON.stringify(await page.evaluate(()=>({text:window.localText,statuses:window.localStatuses})),null,2));
  await page.evaluate(()=>{window.localAudio?.pause();window.localOff?.forEach(fn=>fn());}).catch(()=>{});await api('stop_caption');await wait(async()=>!await api('caption_running'));await api('close_caption_window');
  const leftovers=fs.readdirSync(os.tmpdir()).filter(n=>/^innote-caption-.*\.wav$/.test(n)&&!before.has(n));
  results.push({test:'temporary-audio-cleanup',leftovers,passed:leftovers.length===0});
  await page.evaluate(saved=>{if(saved===null)localStorage.removeItem('may.caption.standalone');else localStorage.setItem('may.caption.standalone',saved);},saved);await page.reload();await page.getByRole('button',{name:'Phụ đề',exact:true}).click();
  fs.writeFileSync('test-results/local-caption-native/results.json',JSON.stringify(results,null,2));console.log(JSON.stringify(results,null,2));await browser.close();
 }
 assert(results.every(r=>r.passed));
})().catch(e=>{console.error(e);process.exitCode=1});
