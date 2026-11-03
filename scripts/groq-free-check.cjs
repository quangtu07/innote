// Opt-in real Groq test. Reads only public profile metadata; key stays in Rust.
const{chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path');
(async()=>{
 const browser=await chromium.connectOverCDP('http://127.0.0.1:9223');const page=browser.contexts().flatMap(c=>c.pages()).find(p=>p.url().endsWith(':1420/'));assert(page);
 const api=(cmd,args)=>page.evaluate(({cmd,args})=>window.__TAURI_INTERNALS__.invoke(cmd,args),{cmd,args});
 const wait=async(fn,ms=20000)=>{const end=Date.now()+ms;while(Date.now()<end){if(await fn())return;await page.waitForTimeout(100);}throw Error('Timed out waiting for Groq state');};
 assert(!(await api('snapshot')).active_id&&!await api('caption_running'),'Refuse to interrupt an active session');
 const p=(await api('list_ai_profiles')).find(p=>p.provider==='groq');assert(p,'Save a verified Groq profile in Settings first');
 const results=[];const out=path.resolve('test-results/groq-free');fs.mkdirSync(out,{recursive:true});
 try{
  for(const model of (process.env.GROQ_MODEL ? [process.env.GROQ_MODEL] : ['openai/gpt-oss-20b','openai/gpt-oss-120b','qwen/qwen3.8-27b','whisper-large-v3-turbo','whisper-large-v3'])){
   const t=Date.now();try{await api('test_ai_connection',{input:{provider:'groq',apiKey:'',baseUrl:'',model,profileId:p.id}});results.push({test:'model-access',model,ms:Date.now()-t,passed:true});}catch(e){results.push({test:'model-access',model,passed:false,error:String(e)});}
  }
  for(const model of ['whisper-large-v3-turbo','whisper-large-v3']){
   if(!results.find(r=>r.model===model)?.passed)continue;
   await page.evaluate(async()=>{window.groqAudit=[];window.groqWords=[];window.groqErrors=[];const{listen}=await import('/node_modules/@tauri-apps/api/event.js');window.groqWordsOff=await listen('groq-caption-audit',({payload})=>window.groqWords.push(payload));window.groqTextOff=await listen('caption-text',({payload})=>{if(payload.text.trim())window.groqAudit.push({at:Date.now(),...payload})});window.groqStatusOff=await listen('caption-status',({payload})=>{if(payload.state==='error')window.groqErrors.push(payload.message)});});
   await api('start_caption',{options:{provider:'',apiKey:'',baseUrl:'',model,targetLanguage:'vi',translate:false,sourceLanguage:'en',device:null,mode:'standalone',profileId:p.id,sessionDir:null}});await api('open_caption_window');
   const start=Date.now();await page.evaluate(async file=>{const{convertFileSrc}=await import('/node_modules/@tauri-apps/api/core.js');window.groqAudio=new Audio(convertFileSrc(file));const end=new Promise((resolve,reject)=>{window.groqAudio.onended=resolve;window.groqAudio.onerror=reject});await window.groqAudio.play();await end;},path.resolve('test-results/fixtures/english.wav'));
   await page.waitForTimeout(2500);await api('stop_caption');await wait(async()=>!await api('caption_running'));
   const audit=await page.evaluate(()=>{window.groqTextOff();window.groqStatusOff();window.groqWordsOff();return{events:window.groqAudit,words:window.groqWords,errors:window.groqErrors}});
   fs.writeFileSync(path.join(out,`${model}-audit.json`),JSON.stringify(audit,null,2));
   const text=audit.events.map(e=>e.text).join(' ');const normalized=text.toLowerCase();
   const missing=['engineering','friday','million','please','review','recording','send','meeting','tomorrow'].filter(w=>!normalized.includes(w));
   const result={test:'speaker-caption',model,firstTextMs:audit.events[0]?.at-start,text,errors:audit.errors,missing,sourceOnly:audit.events.every(e=>e.original===''),passed:audit.events.length>0&&audit.errors.length===0&&missing.length===0};results.push(result);
   await api('close_caption_window');
  }
 }finally{await page.evaluate(()=>{window.groqAudio?.pause();window.groqTextOff?.();window.groqStatusOff?.()}).catch(()=>{});await api('stop_caption');await wait(async()=>!await api('caption_running'));await api('close_caption_window');fs.writeFileSync(path.join(out,'results.json'),JSON.stringify(results,null,2));console.log(JSON.stringify(results,null,2));await browser.close();}
 assert(results.every(r=>r.passed),'Groq checks have failures; inspect results');
})().catch(e=>{console.error(e);process.exitCode=1});
