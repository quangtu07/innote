// Optional real API verification. Keys are resolved only inside the isolated backend.
const assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path');
const {execFile}=require('node:child_process'),{promisify}=require('node:util');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
(async()=>{
 const browser=await chromium.connectOverCDP('http://127.0.0.1:9223');const page=browser.contexts().flatMap(c=>c.pages()).find(p=>p.url()==='http://127.0.0.1:1420/');assert(page);
 const api=(cmd,args={})=>page.evaluate(({cmd,args})=>window.__TAURI_INTERNALS__.invoke(cmd,args),{cmd,args});
 const first=await api('snapshot');assert(first.data_dir.includes('innote-storage-native-20261107')&&!first.active_id);
 const results=JSON.parse(fs.readFileSync('test-results/meeting-storage-native/native-results.json','utf8'));
 assert(first.sessions.some(s=>s.id===results.first.id)&&first.sessions.some(s=>s.id===results.second.id),'Restart retains both libraries');
 const profiles=await api('list_ai_profiles');const soniox=profiles.find(p=>p.provider==='soniox');
 if(!soniox){console.log('SKIP Soniox live: no profile in isolated backend');await browser.close();return;}
 const speaker=first.devices.find(d=>d.kind==='system'&&d.default)||first.devices.find(d=>d.kind==='system');
 const play=()=>promisify(execFile)('powershell.exe',['-NoProfile','-File',path.resolve('scripts/meeting-storage-play-fixture.ps1'),'-AudioPath',path.resolve('test-results/fixtures/english.wav')]);
 async function wait(fn,ms=60000){const end=Date.now()+ms;while(Date.now()<end){const v=await fn();if(v)return v;await page.waitForTimeout(150);}throw Error('Timed out');}
 let own;
 try {
   const s=await api('start',{options:{title:'QA Soniox bản gốc và bản dịch',system:speaker.id,microphone:null,language:'en',model:'moonshine-en',transcribe:true,provider:'soniox',profile_id:soniox.id,speech_quality:'fast',captions_enabled:true,translation_language:'vi'}});own=s.id;
   await play();await page.waitForTimeout(3000);await api('stop');await wait(async()=>!(await api('snapshot')).active_id);
   const saved=(await api('snapshot')).sessions.find(x=>x.id===own);assert.equal(saved.status,'completed',saved.error);assert(saved.segments.length&&saved.translations.length,'Both original and translation must persist');assert.equal(saved.speech.connections,1,'Shared single Soniox stream');
   const info=await api('meeting_files_status',{id:own});await api('retry_meeting_bundle',{id:own});
   const bundle=JSON.parse(fs.readFileSync(path.join(info.path,'meeting.json'),'utf8'));assert(bundle.original.segments.length&&bundle.translations.length);assert(!JSON.stringify(bundle).includes('profile_id'));
   const groq=profiles.find(p=>p.provider==='groq');let summary='not tested: no Groq profile';
   if(groq){const result=await api('summarize_transcript',{id:own,provider:'groq',profileId:groq.id,model:'openai/gpt-oss-20b',baseUrl:'',prompt:'',style:'brief',source:'original',key:null,fallback:null});assert(result.summary?.text.trim());assert(fs.existsSync(path.join(info.path,'summary.md')));summary='Groq gpt-oss-20b passed';}
   own=null;const count=(await api('snapshot')).sessions.length;
   await api('start_caption',{options:{provider:'soniox',profileId:soniox.id,apiKey:'',baseUrl:'',model:'stt-rt-v6',targetLanguage:'vi',translate:true,sourceLanguage:'en',device:speaker.id,mode:'standalone',sessionDir:null,speechQuality:'fast'}});
   await api('open_caption_window');await play();await page.waitForTimeout(2500);
   const captions=await api('caption_snapshot');assert(captions.caption.text.trim(),'Standalone cloud captions must display real text');
   await api('stop_caption');await wait(async()=>!await api('caption_running'));await api('close_caption_window');
   assert.equal((await api('snapshot')).sessions.length,count,'Standalone must not create a meeting');
   fs.writeFileSync('test-results/meeting-storage-native/cloud-results.json',JSON.stringify({restart:true,soniox:{originals:saved.segments.length,translations:saved.translations.length,connections:saved.speech.connections},standalone:true,summary},null,2));
   console.log('PASS restart, real Soniox shared original/translation, standalone captions and '+summary);
 } finally {
   const active=(await api('snapshot')).active_id;if(own&&active===own)await api('stop');
   if(await api('caption_running'))await api('stop_caption');await api('close_caption_window');await browser.close();
 }
})().catch(e=>{console.error(e);process.exit(1);});
