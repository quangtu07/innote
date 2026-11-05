// Opt-in real Soniox meeting test: public/reference files through Windows speakers; mic off.
const assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path');
const{chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
(async()=>{
 const browser=await chromium.connectOverCDP('http://127.0.0.1:9223');
 const page=browser.contexts().flatMap(c=>c.pages()).find(p=>p.url().endsWith(':1420/'));
 const api=(cmd,args)=>page.evaluate(({cmd,args})=>window.__TAURI_INTERNALS__.invoke(cmd,args),{cmd,args});
 const wait=async(predicate,timeout=30000)=>{const until=Date.now()+timeout;while(Date.now()<until){if(await predicate())return;await page.waitForTimeout(100);}throw Error('Timed out waiting for backend state');};
 const initial=await api('snapshot');assert(!initial.active_id&&!await api('caption_running'),'Do not interrupt user sessions');
 const profile=(await api('list_ai_profiles')).find(p=>p.provider==='soniox');assert(profile);
 const saved=await page.evaluate(()=>({preferences:localStorage.getItem('may.preferences'),standalone:localStorage.getItem('may.caption.standalone')}));
 const out=path.resolve('test-results/soniox-shared-meeting');fs.mkdirSync(out,{recursive:true});
 const results=[];let ownId;
 async function start(translate,rotate=false){
  // A completed scenario can leave the meetings dialog open. Return to the
  // page just as a user would before starting the next independent meeting.
  await page.keyboard.press('Escape');
  await page.getByRole('button',{name:'Cuộc họp mới',exact:true}).click();
  await page.getByLabel('Tên cuộc họp').fill('QA Soniox dùng chung — '+(rotate?'bật-tắt dịch + pause':translate?'gốc + dịch':'chỉ chép lời'));
  await page.getByLabel('Thiết bị phát âm thanh cuộc họp').selectOption(initial.devices.find(d=>d.kind==='system'&&d.default).id);
  await page.getByLabel('Micro của bạn',{exact:true}).selectOption('');
  await page.getByLabel('Ngôn ngữ',{exact:true}).selectOption(translate?'vi':'en');
  await page.getByRole('checkbox',{name:/Chép lời khi/}).check();
  await page.getByLabel('Cách chép lời').selectOption('soniox');
  await page.getByLabel('Cấu hình Soniox',{exact:true}).selectOption(profile.id);
  await page.getByLabel('Dự phòng local khi Soniox lỗi').uncheck();
  if(translate)await page.getByLabel('Cách chép lời').selectOption('local');
  await page.getByLabel('Bật phụ đề khi bắt đầu cuộc họp').setChecked(!rotate);
  if(!rotate)await page.getByLabel('Dịch sang trong cuộc họp').setChecked(translate);
  if(translate)await page.getByLabel('Ngôn ngữ bản dịch',{exact:true}).selectOption('en');
  if(translate){await page.getByLabel('Model dùng chung cho cuộc họp').selectOption(JSON.stringify({profileId:profile.id,model:'stt-rt-v6'}));assert.equal(await page.getByLabel('Cách chép lời').inputValue(),'soniox');assert.equal(await page.getByLabel('Cấu hình Soniox',{exact:true}).inputValue(),profile.id);}
  await page.getByRole('button',{name:'Bắt đầu ghi',exact:true}).click();
  await wait(async()=>!!(await api('snapshot')).active_id,15000);
  ownId=(await api('snapshot')).active_id;
  const audio=path.resolve(translate?'test-results/fixtures/vietnamese-pcm.wav':'test-results/fixtures/english.wav');
  await page.evaluate(async file=>{
   const{convertFileSrc}=await import('/node_modules/@tauri-apps/api/core.js');const a=new Audio(convertFileSrc(file));window.sharedFixture=a;
   window.sharedFixtureEnd=new Promise((resolve,reject)=>{a.onended=resolve;a.onerror=()=>reject(Error('Reference speaker playback failed'))});await a.play();
  },audio);
  if(rotate){
    await page.waitForTimeout(2000);
    await page.getByLabel('Dịch sang khi bật phụ đề').check();
    await page.locator('.record-controls').getByRole('button',{name:'Phụ đề',exact:true}).click();
    await wait(async()=>await api('caption_running'));
    await page.waitForTimeout(1200);
    await page.evaluate(()=>window.sharedFixture.pause());await api('pause',{paused:true});await page.waitForTimeout(800);await api('pause',{paused:false});await page.evaluate(()=>window.sharedFixture.play());
    await page.waitForTimeout(2200);
    await page.locator('.record-controls').getByRole('button',{name:'Tắt phụ đề',exact:true}).click();
    assert((await api('snapshot')).active_id,'Disabling translation must retain the recording');
    await page.waitForTimeout(1600);
  }
  await page.evaluate(()=>window.sharedFixtureEnd);await page.waitForTimeout(2500);
  if(!translate&&!rotate){const caption=await api('caption_snapshot');assert(caption.status.sending,'Meeting source caption is active');assert(caption.caption.text.trim(),'Original must be the displayed text, even without bilingual mode');assert.equal(caption.caption.original,'','Do not duplicate source into two lines');}
  await page.screenshot({path:path.join(out,translate?'meeting-translated-live.png':'meeting-original-live.png')});
  await page.locator('.record-controls').getByRole('button',{name:'Kết thúc',exact:true}).click();
  await page.getByRole('button',{name:'Kết thúc và lưu',exact:true}).click();
  await wait(async()=>{const s=await api('snapshot');return !s.active_id && !['recording','paused','processing'].includes(s.sessions.find(m=>m.id===ownId)?.status);});
  const meeting=(await api('snapshot')).sessions.find(s=>s.id===ownId);fs.writeFileSync(path.join(out,rotate?'rotation.json':translate?'translated.json':'original.json'),JSON.stringify(meeting,null,2));
  await page.getByText('Đã hoàn tất',{exact:true}).waitFor({timeout:15000});
  assert.equal(meeting.status,'completed',meeting.error);assert.equal(meeting.provider,'soniox');assert.equal(meeting.speech.local_fallback,false);
  if(!rotate)assert.equal(meeting.speech.connections,1,'A single shared Soniox connection');else assert(meeting.speech.connections>=2,'Configuration rotates sequentially');assert(meeting.speech.uploaded_samples>16000);assert(meeting.segments.length);
  assert(meeting.speech.uploaded_samples <= meeting.duration*16000+3200,'No replayed audio when toggling');
  for(const s of meeting.segments)assert(s.start>=0&&s.end>s.start&&s.end<=meeting.duration+.1);
  if(translate){
   assert(meeting.translations.length,'Both original and translation must be saved');
   for(const t of meeting.translations){const originals=(t.source_ids?.length?t.source_ids:[t.source_id]).map(id=>meeting.segments.find(s=>s.id===id));assert(originals.every(Boolean));assert.equal(t.original,originals.map(s=>s.text).join(' '));assert.equal(t.language,'en');}
   await page.getByLabel('Nội dung hiển thị').selectOption('bilingual');
   await page.locator('.saved-translation').first().waitFor();
   await page.screenshot({path:path.join(out,'bilingual-saved.png')});
   await page.getByLabel('Nội dung hiển thị').selectOption('translation');
   assert((await page.locator('.transcript').innerText()).includes(meeting.translations[0].text));
   await page.reload();await page.getByRole('button',{name:'Danh sách cuộc họp',exact:true}).click();
   await page.getByRole('dialog',{name:'Cuộc họp của bạn'}).getByRole('button',{name:new RegExp('QA Soniox dùng chung — gốc \\+ dịch')}).first().click();
   await page.getByLabel('Nội dung hiển thị').selectOption('bilingual');await page.locator('.saved-translation').first().waitFor();
  }else if(!rotate){assert.equal(meeting.translations.length,0);assert.equal(meeting.speech.translation_language,null);}
  const first=meeting.segments[0];
  await page.locator('audio.playback').waitFor({timeout:10000});
  await page.locator('.transcript .timestamp').first().click();
  await page.waitForFunction(end=>{const a=document.querySelector('audio.playback');return a.paused&&Math.abs(a.currentTime-end)<.15},first.end,{timeout:Math.ceil((first.end-first.start+6)*1000)});
  const sourceText=meeting.segments.map(s=>s.text).join(' ').toLowerCase();
  const referenceTerms=translate?['voi','hươu cao cổ','quan sát']:['prototype','friday','20 million','tomorrow'];
  const missingReferenceTerms=referenceTerms.filter(term=>!sourceText.includes(term));
  results.push({mode:rotate?'translation-toggle+pause':translate?'original+translation':'transcription-only',id:meeting.id,provider:meeting.provider,model:meeting.speech.model,connections:meeting.speech.connections,originals:meeting.segments.length,translations:meeting.translations.length,segmentPlaybackEnd:first.end,missingReferenceTerms,warnings:meeting.warnings,passed:true});
 }
 try{await start(false);await start(true);await start(false,true);}
 catch(error){fs.writeFileSync(path.join(out,'failure.json'),JSON.stringify({error:String(error),ownId,snapshot:await api('snapshot')},null,2));await page.screenshot({path:path.join(out,'failure.png')});throw error;}
 finally{
  await page.evaluate(()=>window.sharedFixture?.pause()).catch(()=>{});
  const current=(await api('snapshot')).active_id;if(ownId&&current===ownId)await api('stop');
  await api('close_caption_window');
  await page.evaluate(saved=>{for(const[n,k]of [['preferences','may.preferences'],['standalone','may.caption.standalone']]){if(saved[n]===null)localStorage.removeItem(k);else localStorage.setItem(k,saved[n]);}},saved);
  await page.reload();
  fs.writeFileSync(path.join(out,'results.json'),JSON.stringify(results,null,2));console.log(JSON.stringify(results,null,2));await browser.close();
 }
})().catch(e=>{console.error(e);process.exitCode=1});
