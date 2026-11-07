// Opt-in: run against a freshly built app with isolated MAY_DATA_DIR and WebView2 CDP 9223.
const assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path'),os=require('node:os');
const {execFile}=require('node:child_process');
const {promisify}=require('node:util');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
(async()=>{
 const browser=await chromium.connectOverCDP('http://127.0.0.1:9223');
 const page=browser.contexts().flatMap(c=>c.pages()).find(p=>p.url()==='http://127.0.0.1:1420/');assert(page);
 const api=(cmd,args={})=>page.evaluate(({cmd,args})=>window.__TAURI_INTERNALS__.invoke(cmd,args),{cmd,args});
 const out=path.resolve('test-results/meeting-storage-native');fs.mkdirSync(out,{recursive:true});
 const privateRoot=path.join(os.tmpdir(),'innote-storage-native-20261107','private');
 assert.equal(path.resolve((await api('snapshot')).data_dir).toLowerCase(),privateRoot.toLowerCase(),'Only isolated test app allowed');
 assert(!(await api('snapshot')).active_id,'Never interrupt a running meeting');
 const rootA=path.join(os.tmpdir(),'innote-storage-native-20261107','library-a');const rootB=path.join(out,'library-b');
 fs.mkdirSync(rootA,{recursive:true});fs.mkdirSync(rootB,{recursive:true});
 await page.evaluate(()=>{localStorage.setItem('may.ui_locale','vi');});await page.reload();
 if(await page.getByRole('button',{name:'Đã hiểu',exact:true}).isVisible().catch(()=>false))await page.getByRole('button',{name:'Đã hiểu',exact:true}).click();
 await api('set_meetings_directory',{path:rootA});
 await page.getByRole('button',{name:'Cài đặt',exact:true}).click();
 await page.getByRole('heading',{name:'Lưu trữ cuộc họp',exact:true}).waitFor();
 await page.locator('.meeting-storage').scrollIntoViewIfNeeded();
 await page.locator('.meeting-storage').screenshot({path:path.join(out,'settings.png')});
 const initial=await api('snapshot');const speaker=initial.devices.find(d=>d.kind==='system'&&d.default)||initial.devices.find(d=>d.kind==='system');assert(speaker);
 assert(initial.models.some(m=>m.id==='moonshine-en'&&m.installed),'Bundled English model required');
 async function wait(fn,ms=60000){const deadline=Date.now()+ms;while(Date.now()<deadline){const v=await fn();if(v)return v;await page.waitForTimeout(150);}throw Error('Timed out');}
 async function capture(title,transcribe){
   const s=await api('start',{options:{title,system:speaker.id,microphone:null,language:'en',model:'moonshine-en',transcribe,provider:'local',speech_quality:'fast',captions_enabled:transcribe,translation_language:null}});
   await assert.rejects(api('set_meetings_directory',{path:rootB}),/kết thúc|phiên|ghi/i);
   const playback=promisify(execFile)('powershell.exe',['-NoProfile','-File',path.resolve('scripts/meeting-storage-play-fixture.ps1'),'-AudioPath',path.resolve('test-results/fixtures/english.wav')]);
   await page.waitForTimeout(2000);
   await api('edit_session',{id:s.id,title:title+' đổi tên',notes:null,segmentId:null,text:null});
   await playback;await api('pause',{paused:true});await page.waitForTimeout(250);await api('pause',{paused:false});
   await api('stop');
   await wait(async()=>!(await api('snapshot')).active_id);
   const current=(await api('snapshot')).sessions.find(x=>x.id===s.id);assert(['completed','recorded'].includes(current.status),current.error);
   const files=await api('meeting_files_status',{id:s.id});assert(fs.existsSync(path.join(files.path,'recording.wav')));
   assert(files.path.includes('đổi tên'));assert(!path.resolve(files.path).startsWith(privateRoot));
   await api('retry_meeting_bundle',{id:s.id});const bundle=JSON.parse(fs.readFileSync(path.join(files.path,'meeting.json'),'utf8'));
   assert.equal(bundle.id,s.id);assert.equal(bundle.audio.available,true);assert.equal(bundle.audio.path,'recording.wav');
   const wav=await api('audio_path',{id:s.id});
   const media=await page.evaluate(async wav=>{const {convertFileSrc}=await import('/node_modules/@tauri-apps/api/core.js');const audio=new Audio(convertFileSrc(wav));await audio.play();const until=Date.now()+5000;while(audio.currentTime===0&&Date.now()<until)await new Promise(r=>setTimeout(r,100));const result={duration:audio.duration,time:audio.currentTime};audio.pause();return result;},wav);
   assert(media.duration>5&&media.time>0,'Actual asset playback from selected disk: '+JSON.stringify(media));
   const forbidden=await page.evaluate(async p=>{const {convertFileSrc}=await import('/node_modules/@tauri-apps/api/core.js');const r=await fetch(convertFileSrc(p));return r.status;},path.join(privateRoot,'meeting-storage.json'));
   assert(forbidden>=400,'Private registry must not be available through media scope');
   if(transcribe){assert(current.segments.length||current.live.length,'Speaker ASR must produce text');}
   return {id:s.id,files:files.path,transcribe,segments:current.segments.length,live:current.live.length,media,privateAssetStatus:forbidden};
 }
 const first=await capture('QA lưu tại C',true);
 const original=JSON.stringify((await api('snapshot')).sessions.find(x=>x.id===first.id).segments);
 const corrected=await api('correct_transcript',{id:first.id,profileId:'__local__',model:'moonshine-en',fallback:null});assert(corrected.corrected?.segments.length);
 assert.equal(JSON.stringify(corrected.segments),original,'Correction cannot overwrite original');
 await api('set_meetings_directory',{path:rootB});const second=await capture('QA lưu tại D',false);
 assert((await api('snapshot')).sessions.some(s=>s.id===first.id));
 const registry=JSON.parse(fs.readFileSync(path.join(privateRoot,'meeting-storage.json'),'utf8'));assert.equal(registry.libraries.length,2);
 assert(!fs.existsSync(path.join(privateRoot,'sessions',first.id)));assert(!fs.existsSync(path.join(privateRoot,'sessions',second.id)));
 fs.writeFileSync(path.join(out,'native-results.json'),JSON.stringify({first,second,correctionModel:corrected.corrected.model,registryCount:registry.libraries.length},null,2));
 console.log('PASS native two-volume storage, system-speaker recording, pause/resume, rename, local ASR/correction, bundle and private asset rejection');
 await browser.close();
})().catch(e=>{console.error(e);process.exit(1);});
