// Bundled Moonshine meeting + shared source captions, after removing PhoWhisper files.
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path');
(async()=>{
 const browser=await chromium.connectOverCDP('http://127.0.0.1:9223');
 const page=browser.contexts().flatMap(c=>c.pages()).find(p=>p.url().endsWith(':1420/'));assert(page);
 const api=(cmd,args)=>page.evaluate(({cmd,args})=>window.__TAURI_INTERNALS__.invoke(cmd,args),{cmd,args});
 const wait=async(fn,ms=30000)=>{const end=Date.now()+ms;while(Date.now()<end){if(await fn())return;await page.waitForTimeout(100);}throw Error('Moonshine meeting timeout');};
 const initial=await api('snapshot');assert(!initial.active_id&&!await api('caption_running'));
 const preserved=initial.sessions.map(s=>({id:s.id,segments:s.segments,notes:s.notes,model:s.model}));
 const saved=await page.evaluate(()=>Object.fromEntries(['may.preferences','may.meeting.local-models','may.speech.processing-levels'].map(k=>[k,localStorage.getItem(k)])));
 const results=[];let owned;
 try{
  assert(!initial.models.some(m=>m.id==='phowhisper-small'));
  for(const language of ['vi','en']){
   if(process.env.MEETING_LANGUAGE&&process.env.MEETING_LANGUAGE!==language)continue;
   const model=process.env.MEETING_MODEL||'moonshine-'+language;
   const quality=process.env.MEETING_LEVEL||'balanced';
   await page.keyboard.press('Escape');
   await page.getByRole('button',{name:'Cuộc họp mới',exact:true}).click();
   await page.getByLabel('Tên cuộc họp',{exact:true}).fill('QA Moonshine có sẵn — '+language);
   await page.getByLabel('Thiết bị phát âm thanh cuộc họp').selectOption(initial.devices.find(d=>d.kind==='system'&&d.default).id);
   await page.getByLabel('Micro của bạn',{exact:true}).selectOption('');
   await page.getByLabel('Ngôn ngữ',{exact:true}).selectOption(language);
   await page.getByLabel('Cách chép lời',{exact:true}).selectOption('local');
   await page.getByRole('checkbox',{name:/Chép lời khi/}).check();
   await page.getByLabel('Model chép lời local',{exact:true}).selectOption(model);
   await page.getByRole('radio',{name:'Cuộc họp · '+({fast:'Nhanh',balanced:'Cân bằng',accurate:'Chính xác'}[quality]),exact:true}).check();
   await page.getByLabel('Bật phụ đề khi bắt đầu cuộc họp').check();
   await page.getByLabel('Dịch sang trong cuộc họp').uncheck();
   assert.equal(await page.getByRole('button',{name:/Tải ·/}).count(),0);
   await page.getByRole('button',{name:'Bắt đầu ghi',exact:true}).click();
   await wait(async()=>!!(await api('snapshot')).active_id);owned=(await api('snapshot')).active_id;
   await page.getByRole('button',{name:'Kết thúc',exact:true}).waitFor();
   if(process.env.MEETING_LIVE_ONLY==='1')await page.waitForTimeout(10000);
   else await page.evaluate(async file=>{const{convertFileSrc}=await import('/node_modules/@tauri-apps/api/core.js');window.moonMeetingAudio=new Audio(convertFileSrc(file));const done=new Promise((resolve,reject)=>{window.moonMeetingAudio.onended=resolve;window.moonMeetingAudio.onerror=reject;});await window.moonMeetingAudio.play();await done;},path.resolve('test-results/fixtures',language==='vi'?'vietnamese-pcm.wav':'english.wav'));
   await page.waitForTimeout(2000);
   const caption=await api('caption_snapshot');assert.equal(caption.status.provider,'local');assert(caption.caption.text.trim());assert(!caption.status.sending);
   await page.getByRole('button',{name:'Kết thúc',exact:true}).click();
   await page.getByRole('button',{name:'Kết thúc và lưu',exact:true}).click();
   await wait(async()=>{const s=(await api('snapshot')).sessions.find(s=>s.id===owned);return s?.status==='completed';});
   const meeting=(await api('snapshot')).sessions.find(s=>s.id===owned);
   assert.equal(meeting.model,model);assert.equal(meeting.speech_quality,quality);assert.equal(meeting.provider,'local');assert(meeting.segments.length);assert.equal(meeting.translations.length,0);
   if(process.env.MEETING_LIVE_ONLY!=='1'){
    const first=meeting.segments[0];await page.locator('.transcript .timestamp').first().click();
    await page.waitForFunction(end=>{const a=document.querySelector('audio.playback');return a.paused&&Math.abs(a.currentTime-end)<.15;},first.end,{timeout:Math.ceil((first.end-first.start+6)*1000)});
   }
   results.push({language,model:meeting.model,level:meeting.speech_quality,segments:meeting.segments.length,captionShared:true,noCloud:true,segmentPlayback:process.env.MEETING_LIVE_ONLY!=='1',liveSource:process.env.MEETING_LIVE_ONLY==='1',passed:true});
   owned=null;
  }
  const after=await api('snapshot');for(const old of preserved){const now=after.sessions.find(s=>s.id===old.id);assert.deepEqual({id:now.id,segments:now.segments,notes:now.notes,model:now.model},old);}
 }finally{
  await page.evaluate(()=>window.moonMeetingAudio?.pause()).catch(()=>{});
  if(owned&&(await api('snapshot')).active_id===owned)await api('stop');
  await api('close_caption_window');
  await page.evaluate(saved=>{for(const[k,v]of Object.entries(saved)){if(v===null)localStorage.removeItem(k);else localStorage.setItem(k,v);}},saved);await page.reload();
  fs.writeFileSync(process.env.MEETING_RESULT||'test-results/remove-pho-meetings.json',JSON.stringify(results,null,2));console.log(JSON.stringify(results));await browser.close();
 }
})().catch(error=>{console.error(error);process.exitCode=1});
