// Controlled public speaker reference; credentials stay in Rust.
const{chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict');
(async()=>{
 const browser=await chromium.connectOverCDP('http://127.0.0.1:9223');
 const pages=()=>browser.contexts().flatMap(c=>c.pages());const main=pages().find(p=>p.url().endsWith(':1420/'));assert(main);
 const api=(cmd,args)=>main.evaluate(({cmd,args})=>window.__TAURI_INTERNALS__.invoke(cmd,args),{cmd,args});
 const wait=async(fn,ms=30000)=>{const end=Date.now()+ms;while(Date.now()<end){if(await fn())return;await main.waitForTimeout(100);}throw Error('Realtime benchmark timed out');};
 const initial=await api('snapshot');assert(!initial.active_id&&!await api('caption_running'),'Do not interrupt a user session');
 const provider=process.env.BENCH_PROVIDER||'soniox',profile=provider==='local'?{id:'__local__'}:(await api('list_ai_profiles')).find(p=>p.provider===provider);assert(profile);
 const model=process.env.BENCH_MODEL||(provider==='groq'?'whisper-large-v3-turbo':provider==='local'?'nemotron-3.5-q8':'stt-rt-v5');
 if(provider==='local')assert(initial.models.some(m=>m.id===model&&m.installed));
 const visible=await api('caption_window_visible'),saved=await main.evaluate(()=>localStorage.getItem('innote.caption.style'));
 const timing=JSON.parse(fs.readFileSync('test-results/fixtures/realtime/reference-timing.json','utf8'));
 const modes=(process.env.BENCH_MODES||(provider==='soniox'?'source,translated,bilingual':'source')).split(',');
 const results=[];let overlay;
 const norm=s=>s.toLowerCase().replace(/15\b/g,'fifteen').replace(/50\b/g,'fifty').replace(/[^\p{L}\p{N}\s]/gu,' ').replace(/\s+/g,' ');
 const anchor=s=>{const n=norm(s);let i=n.includes('meeting')?2:n.includes('fifty million')?1:n.includes('friday')?0:-1;return i<0?null:timing.utterances[i].speechEndMs;};
 try{
  await api('open_caption_window');overlay=pages().filter(p=>p.url().includes('caption.html')).at(-1);assert(overlay);await overlay.locator('.caption-root').waitFor();
  for(const mode of modes)for(let trial=0;trial<Number(process.env.BENCH_TRIALS||2);trial++){
   await main.evaluate(async bilingual=>{const{emit}=await import('/node_modules/@tauri-apps/api/event.js');const old=JSON.parse(localStorage.getItem('innote.caption.style')||'{}');localStorage.setItem('innote.caption.style',JSON.stringify({...old,showOriginal:bilingual}));await emit('caption-style',{});},mode==='bilingual');
   await overlay.evaluate(async()=>{
    const{listen}=await import('/node_modules/@tauri-apps/api/event.js');window.perfAudit={events:[],frames:[],reads:0,writes:0,epoch:0};
    const height=Object.getOwnPropertyDescriptor(Element.prototype,'scrollHeight'),top=Object.getOwnPropertyDescriptor(Element.prototype,'scrollTop');window.perfDescriptors={height,top};
    Object.defineProperty(Element.prototype,'scrollHeight',{...height,get(){if(this.matches('.caption-text,.caption-original'))window.perfAudit.reads++;return height.get.call(this);}});
    Object.defineProperty(Element.prototype,'scrollTop',{...top,set(value){if(this.matches('.caption-text,.caption-original'))window.perfAudit.writes++;top.set.call(this,value);}});
    window.perfOff=await listen('caption-text',({payload})=>{if(window.perfAudit.epoch)window.perfAudit.events.push({at:performance.now(),wall:Date.now(),...payload});});
    window.perfObserver=new MutationObserver(()=>{const text=document.querySelector('.caption-text')?.textContent??'',original=document.querySelector('.caption-original')?.textContent??'';if(!text||!window.perfAudit.epoch)return;const at=performance.now();requestAnimationFrame(()=>window.perfAudit.frames.push({at,frame:performance.now(),wall:Date.now(),text,original}));});window.perfObserver.observe(document.querySelector('.caption-lines'),{childList:true,subtree:true,characterData:true});
   });
   const setup=Date.now();
   await api('start_caption',{options:{provider:'',apiKey:'',baseUrl:'',speechQuality:process.env.BENCH_LEVEL||'balanced',profileId:profile.id,model,sourceLanguage:'en',targetLanguage:'vi',translate:mode!=='source',mode:'standalone',device:null,sessionDir:null}});
   await wait(async()=>{const s=(await api('caption_snapshot')).status;assert.notEqual(s.state,'error',s.message);return provider==='groq'||provider==='local'?s.state==='active':s.sending;});
   const setupMs=Date.now()-setup;await main.waitForTimeout(500);
   const start=Date.now();await overlay.evaluate(()=>{window.perfAudit.epoch=Date.now();window.perfAudit.events=[];window.perfAudit.frames=[];window.perfAudit.reads=0;window.perfAudit.writes=0;});
   await main.evaluate(async file=>{const{convertFileSrc}=await import('/node_modules/@tauri-apps/api/core.js');window.perfAudio=new Audio(convertFileSrc(file));const done=new Promise((resolve,reject)=>{window.perfAudio.onended=resolve;window.perfAudio.onerror=reject;});await window.perfAudio.play();await done;},path.resolve('test-results/fixtures/realtime/reference.wav'));
   await main.waitForTimeout(provider==='groq'?4500:2500);
   const beforeStop=(await api('caption_snapshot')).status;
   const stopStart=Date.now();await api('stop_caption');await wait(async()=>!await api('caption_running'));const stopMs=Date.now()-stopStart;
   const afterStop=(await api('caption_snapshot')).status;
   const audit=await overlay.evaluate(()=>{window.perfObserver.disconnect();window.perfOff();Object.defineProperty(Element.prototype,'scrollHeight',window.perfDescriptors.height);Object.defineProperty(Element.prototype,'scrollTop',window.perfDescriptors.top);return window.perfAudit;});
   const firstOriginal=audit.events.find(e=>e.original.trim()||(mode==='source'&&e.text.trim())),firstTranslation=audit.events.find(e=>e.text.trim()),firstPair=audit.events.find(e=>e.final_&&e.original.trim()&&e.text.trim());
   const originals=audit.events.filter(e=>e.final_).map(e=>mode==='source'?e.text:e.original).filter(Boolean).filter((t,i,a)=>t!==a[i-1]);
   const lastSource=[...audit.events].reverse().find(e=>(mode==='source'?e.text:e.original).trim());
   const pendingFinal=!!lastSource&&!lastSource.final_;
   if(pendingFinal){const tail=mode==='source'?lastSource.text:lastSource.original;if(!originals.includes(tail))originals.push(tail);}
   const n=norm(originals.join(' '));const missing=['engineering','friday','nguyen','fifteen','not','fifty','invoice','meeting'].filter(w=>!n.includes(w));
   const finalFrameLags=[],renderLags=[];
   for(const frame of audit.frames){const event=audit.events.find(e=>e.text===frame.text&&(mode!=='bilingual'||e.original===frame.original));if(event)renderLags.push(Math.max(0,frame.frame-event.at));const final=audit.events.find(e=>e.final_&&e.text===frame.text&&(mode==='source'||e.original===frame.original));const end=final?anchor(mode==='source'?final.text:final.original):null;if(end!==null)finalFrameLags.push(frame.wall-start-end);}
   assert(audit.frames.length,'No actual overlay frames');
   results.push({provider,model,level:process.env.BENCH_LEVEL||"balanced",mode,trial:trial+1,setupMs,stopMs,beforeStop,afterStop,firstOriginalMs:firstOriginal?firstOriginal.wall-start:null,firstTranslationMs:firstTranslation?firstTranslation.wall-start:null,firstPairMs:firstPair?firstPair.wall-start:null,firstFrameMs:audit.frames[0].wall-start,finalFrameLagsMs:finalFrameLags,eventToFrameMs:renderLags,captionEvents:audit.events.length,frames:audit.frames.length,layoutReads:audit.reads,scrollWrites:audit.writes,missingReferenceTerms:missing,sourcePendingFinal:pendingFinal,sourceText:originals.join(' '),translatedText:mode==='source'?'':audit.events.filter(e=>e.final_).map(e=>e.text).filter(Boolean).join(' ')});
   assert.notEqual(beforeStop.state,'error',beforeStop.message);assert.equal(afterStop.state,'idle',afterStop.message);
  }
 }finally{
  await main.evaluate(()=>window.perfAudio?.pause()).catch(()=>{});await api('stop_caption');await wait(async()=>!await api('caption_running'));
  if(overlay)await overlay.evaluate(()=>{window.perfObserver?.disconnect();window.perfOff?.();if(window.perfDescriptors){Object.defineProperty(Element.prototype,'scrollHeight',window.perfDescriptors.height);Object.defineProperty(Element.prototype,'scrollTop',window.perfDescriptors.top);}}).catch(()=>{});
  await main.evaluate(async saved=>{const{emit}=await import('/node_modules/@tauri-apps/api/event.js');if(saved===null)localStorage.removeItem('innote.caption.style');else localStorage.setItem('innote.caption.style',saved);await emit('caption-style',{});},saved);
  if(visible)await api('open_caption_window');else await api('close_caption_window');const out='test-results/realtime-performance';fs.mkdirSync(out,{recursive:true});const label=process.env.BENCH_LABEL||'baseline';fs.writeFileSync(`${out}/${label}.json`,JSON.stringify(results,null,2));console.log(JSON.stringify(results,null,2));await browser.close();
 }
})().catch(e=>{console.error(e);process.exitCode=1});
