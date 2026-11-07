const assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path');
const{execFile}=require('node:child_process'),{promisify}=require('node:util');const{chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
(async()=>{
 const browser=await chromium.connectOverCDP('http://127.0.0.1:9223'),page=browser.contexts().flatMap(c=>c.pages()).find(p=>p.url()==='http://127.0.0.1:1420/');
 const api=(cmd,args={})=>page.evaluate(({cmd,args})=>window.__TAURI_INTERNALS__.invoke(cmd,args),{cmd,args});
 const first=await api('snapshot');assert(first.data_dir.includes('innote-storage-native-20261107')&&!first.active_id);
 const groq=(await api('list_ai_profiles')).find(p=>p.provider==='groq');if(!groq){console.log('SKIP Groq speech: no profile');await browser.close();return;}
 const speaker=first.devices.find(d=>d.kind==='system'&&d.default)||first.devices.find(d=>d.kind==='system');let own;
 try {
   own=(await api('start',{options:{title:'QA Groq Whisper nơi lưu mới',system:speaker.id,microphone:null,language:'en',model:'whisper-large-v3-turbo',transcribe:true,provider:'groq',profile_id:groq.id,speech_quality:'fast',captions_enabled:true,translation_language:null}})).id;
   await promisify(execFile)('powershell.exe',['-NoProfile','-File',path.resolve('scripts/meeting-storage-play-fixture.ps1'),'-AudioPath',path.resolve('test-results/fixtures/english.wav')]);
   await api('stop');const until=Date.now()+90000;while((await api('snapshot')).active_id&&Date.now()<until)await page.waitForTimeout(200);
   const saved=(await api('snapshot')).sessions.find(s=>s.id===own);assert.equal(saved.status,'completed',saved.error);assert(saved.segments.length);assert.equal(saved.provider,'groq');
   const info=await api('meeting_files_status',{id:own});await api('retry_meeting_bundle',{id:own});assert(fs.existsSync(path.join(info.path,'original.md')));assert(fs.existsSync(path.join(info.path,'recording.wav')));
   fs.writeFileSync('test-results/meeting-storage-native/groq-results.json',JSON.stringify({id:own,provider:saved.provider,model:saved.speech.model,segments:saved.segments.length,path:info.path},null,2));console.log('PASS real Groq Whisper shared caption/transcript saved in selected library');own=null;
 }finally{if(own&&(await api('snapshot')).active_id===own)await api('stop');await api('close_caption_window');await browser.close();}
})().catch(e=>{console.error(e);process.exit(1)});
