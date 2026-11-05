// Renderer skew regression plus optional real Soniox source/translation pairing.
const{chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path');
(async()=>{
 const browser=await chromium.connectOverCDP('http://127.0.0.1:9223');
 const pages=()=>browser.contexts().flatMap(c=>c.pages());const main=pages().find(p=>p.url().endsWith(':1420/'));assert(main);
 const api=(cmd,args)=>main.evaluate(({cmd,args})=>window.__TAURI_INTERNALS__.invoke(cmd,args),{cmd,args});
 const wait=async(fn,ms=30000)=>{const end=Date.now()+ms;while(Date.now()<end){if(await fn())return;await main.waitForTimeout(100);}throw Error('Timed out bilingual state');};
 assert(!(await api('snapshot')).active_id&&!await api('caption_running'),'Do not interrupt a session');
 const visible=await api('caption_window_visible'),saved=await main.evaluate(()=>localStorage.getItem('innote.caption.style')),snapshot=await api('caption_snapshot');
 const emit=(event,payload)=>main.evaluate(async({event,payload})=>{const{emit}=await import('/node_modules/@tauri-apps/api/event.js');await emit(event,payload);},{event,payload});
 const style=async showOriginal=>{await main.evaluate(showOriginal=>{const key='innote.caption.style',old=JSON.parse(localStorage.getItem(key)||'{}');localStorage.setItem(key,JSON.stringify({...old,showOriginal}));},showOriginal);await emit('caption-style',{});};
 const results=[];let overlay,ownMeeting;
 try{
  await api('open_caption_window');overlay=pages().filter(p=>p.url().includes('caption.html')).at(-1);assert(overlay);await overlay.locator('.caption-root').waitFor();await main.waitForTimeout(500);
  await style(true);await emit('caption-mode',true);await emit('caption-status',{state:'sending',provider:'soniox',sending:true,message:'QA'});
  const cue=async(text,original,final_=false,translating=true)=>{await emit('caption-text',{text,original,final_,translating});await main.waitForTimeout(100);};
  const display=()=>overlay.evaluate(()=>({text:document.querySelector('.caption-text')?.textContent??'',original:document.querySelector('.caption-original')?.textContent??''}));
  await cue('','source A');assert.deepEqual(await display(),{text:'',original:''});
  await cue('partial A','source A');assert.deepEqual(await display(),{text:'',original:''});
  await cue('translation A','source A',true);assert.deepEqual(await display(),{text:'translation A',original:'source A'});
  await cue('','source B');assert.deepEqual(await display(),{text:'translation A',original:'source A'});
  await cue('partial B','source B');assert.deepEqual(await display(),{text:'translation A',original:'source A'});
  await style(false);await wait(async()=>(await display()).text==='partial B');assert.equal((await display()).original,'');
  await style(true);await wait(async()=>(await display()).text==='translation A');assert.equal((await display()).original,'source A');
  await cue('translation B','source B',true);assert.deepEqual(await display(),{text:'translation B',original:'source B'});
  await cue('source only live','',false,false);assert.deepEqual(await display(),{text:'source only live',original:''});
  results.push({test:'renderer-reordered-events-toggle-source-only',passed:true});
  await emit('caption-status',snapshot.status);
  if(process.env.BILINGUAL_REAL==='1'){
   const profile=(await api('list_ai_profiles')).find(p=>p.provider==='soniox');assert(profile);
   await main.evaluate(async()=>{window.pairFinals=[];window.pairMessages=[];window.pairAudits=[];const{listen}=await import('/node_modules/@tauri-apps/api/event.js');window.pairAuditOff=await listen('soniox-pair-audit',({payload})=>window.pairAudits.push(payload));window.pairOff=await listen('caption-text',({payload})=>{window.pairMessages.push({final:payload.final_,originalChars:payload.original.length,textChars:payload.text.length});if(payload.final_&&payload.text.trim()&&payload.original.trim())window.pairFinals.push(payload)});});
   await overlay.evaluate(()=>{window.pairFrames=[];window.pairObserver=new MutationObserver(()=>{const text=document.querySelector('.caption-text')?.textContent??'',original=document.querySelector('.caption-original')?.textContent??'';if(text||original)window.pairFrames.push({text,original});});window.pairObserver.observe(document.querySelector('.caption-lines'),{subtree:true,childList:true,characterData:true});});
   if(process.env.BILINGUAL_MEETING==='1'){
    const snap=await api('snapshot');const session=await api('start',{options:{title:'QA Song ngữ đồng bộ — Soniox dùng chung',system:snap.devices.find(d=>d.kind==='system'&&d.default).id,microphone:null,language:'vi',model:'moonshine-vi',transcribe:true,provider:'soniox',profileId:profile.id,translationLanguage:'en',captionsEnabled:true,localFallback:false,speechQuality:process.env.BILINGUAL_LEVEL||'balanced'}});ownMeeting=session.id;
   }else await api('start_caption',{options:{provider:'',apiKey:'',baseUrl:'',profileId:profile.id,model:'stt-rt-v6',mode:'standalone',sourceLanguage:'vi',targetLanguage:'en',translate:true,device:null,sessionDir:null}});
   if(ownMeeting)await wait(async()=>((await api('snapshot')).sessions.find(s=>s.id===ownMeeting)?.speech?.connections??0)>0);
   else await wait(async()=>(await api('caption_snapshot')).status.sending);
   await main.evaluate(async file=>{const{convertFileSrc}=await import('/node_modules/@tauri-apps/api/core.js');window.pairAudio=new Audio(convertFileSrc(file));await window.pairAudio.play();await new Promise((resolve,reject)=>{window.pairAudio.onended=resolve;window.pairAudio.onerror=reject;});},path.resolve('test-results/fixtures/vietnamese-pcm.wav'));
   await wait(async()=>await main.evaluate(()=>window.pairFinals.length>0));await main.waitForTimeout(2000);
   const finals=await main.evaluate(()=>window.pairFinals),frames=await overlay.evaluate(()=>{window.pairObserver.disconnect();return window.pairFrames;});
   assert(frames.length);assert(frames.every(frame=>finals.some(pair=>pair.text===frame.text&&pair.original===frame.original)),'Every visible pair must be a finalized event, with no mixed source/target');
   const cache=await api('caption_snapshot');assert(cache.translating&&cache.paired?.original&&cache.paired?.text);
   await overlay.reload();await wait(async()=>JSON.stringify(await display())===JSON.stringify({text:cache.paired.text,original:cache.paired.original}));
   results.push({test:ownMeeting?'real-shared-meeting-pairs-and-reopen-cache':'real-soniox-frame-pairs-and-reopen-cache',finalPairs:finals.length,renderedFrames:frames.length,passed:true});
   if(ownMeeting){await api('stop');await wait(async()=>{const s=(await api('snapshot')).sessions.find(s=>s.id===ownMeeting);return s?.status==='completed'||s?.status==='failed';});const session=(await api('snapshot')).sessions.find(s=>s.id===ownMeeting);assert.equal(session.status,'completed');assert.equal(session.speech_quality,process.env.BILINGUAL_LEVEL||'balanced');assert.equal(session.speech.connections,1);results.push({test:'shared-meeting-one-connection',level:session.speech_quality,connections:session.speech.connections,originals:session.segments.length,translations:session.translations.length,passed:true});}
   await api('stop_caption');await wait(async()=>!await api('caption_running'));await main.evaluate(()=>window.pairOff());
  }
 }finally{
  fs.mkdirSync('test-results/bilingual-pairs',{recursive:true});fs.writeFileSync('test-results/bilingual-pairs/diagnostics.json',JSON.stringify(await main.evaluate(()=>({audits:window.pairAudits,messages:window.pairMessages})),null,2));await main.evaluate(()=>window.pairAuditOff?.());
  await overlay?.evaluate(()=>window.pairObserver?.disconnect()).catch(()=>{});
  await main.evaluate(()=>{window.pairAudio?.pause();window.pairOff?.();}).catch(()=>{});await api('stop_caption');await wait(async()=>!await api('caption_running'));
  if(ownMeeting&&(await api('snapshot')).active_id===ownMeeting)await api('stop');
  await main.evaluate(saved=>{if(saved===null)localStorage.removeItem('innote.caption.style');else localStorage.setItem('innote.caption.style',saved);},saved);await emit('caption-style',{});await emit('caption-mode',snapshot.translating??false);await emit('caption-text',{...snapshot.caption,translating:snapshot.translating??false});await emit('caption-status',snapshot.status);
  if(visible)await api('open_caption_window');else await api('close_caption_window');const out=process.env.BILINGUAL_MEETING==='1'?'test-results/bilingual-shared-pairs':'test-results/bilingual-pairs';fs.mkdirSync(out,{recursive:true});fs.writeFileSync(`${out}/results.json`,JSON.stringify(results,null,2));console.log(JSON.stringify(results));await browser.close();
 }
})().catch(e=>{console.error(e);process.exitCode=1});
