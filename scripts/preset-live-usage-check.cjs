// Opt-in saved-key QA: create presets through UI, play public audio via speakers, mic off.
const assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
(async()=>{
 const browser=await chromium.connectOverCDP('http://127.0.0.1:9223');
 const main=browser.contexts().flatMap(c=>c.pages()).find(p=>p.url().endsWith(':1420/'));assert(main);
 const api=(cmd,args)=>main.evaluate(({cmd,args})=>window.__TAURI_INTERNALS__.invoke(cmd,args),{cmd,args});
 const wait=async(fn,ms=30000)=>{const until=Date.now()+ms;while(Date.now()<until){if(await fn())return;await main.waitForTimeout(100)}throw Error('QA timed out')};
 const initial=await api('snapshot'),profiles=await api('list_ai_profiles');assert(!initial.active_id&&!await api('caption_running'),'Never interrupt a user session');
 const profile=profiles.find(p=>p.provider==='soniox'&&p.has_key);assert(profile,'Saved Soniox connection required');
 const keys=['innote.workflow-presets.v1','innote.caption.style','may.preferences','may.caption.standalone','may.task_assign.meeting','may.task_assign.captionsonly','may.speech.processing-levels','may.meeting.local-models'];
 const saved=await main.evaluate(keys=>Object.fromEntries(keys.map(k=>[k,localStorage.getItem(k)])),keys);
 const out=path.resolve('test-results/preset-live-usage');fs.mkdirSync(out,{recursive:true});
 let ownId;const result={meeting:null,standalone:null};
 async function create(mode,name,font){
  await main.getByRole('button',{name:'Mẫu cấu hình',exact:true}).click();
  await main.getByRole('tab',{name:mode==='meeting'?'Cuộc họp':'Phụ đề',exact:true}).click();
  await main.getByRole('button',{name:'Tạo mẫu',exact:true}).click();
  await main.getByLabel('Tên mẫu',{exact:true}).fill(name);
  await main.getByLabel('Ngôn ngữ gốc của mẫu').selectOption('vi');
  await main.getByLabel('Dịch sang trong mẫu').check();
  if(mode==='meeting')await main.getByLabel('Bật phụ đề khi bắt đầu',{exact:true}).check();
  await main.getByLabel('Phụ đề trực tiếp',{exact:true}).selectOption(profile.id);
  await main.getByLabel('Ngôn ngữ dịch đích',{exact:true}).selectOption('en');
  await main.getByLabel('Cỡ chữ phụ đề').fill(String(font));await main.getByLabel('Cỡ chữ phụ đề').dispatchEvent('input');
  await main.getByLabel('Trong suốt',{exact:true}).uncheck();
  await main.getByLabel('Song ngữ',{exact:true}).setChecked(mode!=='meeting');
  await main.getByRole('button',{name:'Lưu mẫu',exact:true}).click();
  const row=main.locator('.preset-row').filter({has:main.getByRole('heading',{name,exact:true})});await row.waitFor();
  const preset=await main.evaluate(name=>JSON.parse(localStorage.getItem('innote.workflow-presets.v1')).presets.find(p=>p.name===name),name);
  assert.equal(preset.captionStyle.fontSize,font);assert.equal(preset.tasks.caption.model,'stt-rt-v5');
  if(mode==='meeting')await main.getByRole('button',{name:'Cuộc họp mới',exact:true}).click();
  else await main.getByRole('button',{name:'Chỉ phụ đề',exact:true}).click();
  await main.getByLabel(mode==='meeting'?'Mẫu cấu hình cuộc họp':'Mẫu cấu hình phụ đề').selectOption(preset.id);
  await main.getByRole('button',{name:'Áp mẫu',exact:true}).click();
  await main.getByText(`Áp mẫu thành công: ${name}`,{exact:true}).waitFor();
  assert.equal(await main.evaluate(()=>JSON.parse(localStorage.getItem('innote.caption.style')).fontSize),font);
  return preset;
 }
 async function play(){
  await main.evaluate(async file=>{const{convertFileSrc}=await import('/node_modules/@tauri-apps/api/core.js');const a=new Audio(convertFileSrc(file));window.presetQAAudio=a;window.presetQAEnd=new Promise((resolve,reject)=>{a.onended=resolve;a.onerror=()=>reject(Error('Speaker playback failed'))});await a.play()},path.resolve('test-results/fixtures/vietnamese-pcm.wav'));
  await main.evaluate(()=>window.presetQAEnd);
 }
 async function inspectOverlay(font){
  await wait(async()=>!!(await api('caption_snapshot')).caption.text.trim());
  await wait(async()=>browser.contexts().flatMap(c=>c.pages()).some(p=>p.url().includes('caption.html')));
  const overlay=browser.contexts().flatMap(c=>c.pages()).find(p=>p.url().includes('caption.html'));
  await overlay.locator('.caption-lines .caption-text').waitFor();
  const geometry=await overlay.evaluate(()=>{const root=document.querySelector('.caption-lines').getBoundingClientRect();return [...document.querySelectorAll('.caption-lines .caption-text,.caption-lines .caption-original')].map(el=>{const b=el.getBoundingClientRect();return {inside:b.left>=root.left-.5&&b.right<=root.right+.5&&b.top>=root.top-.5&&b.bottom<=root.bottom+.5,textLength:el.textContent.length}})});
  assert(geometry.length&&geometry.every(x=>x.inside&&x.textLength));
  assert.equal(await overlay.evaluate(()=>JSON.parse(localStorage.getItem('innote.caption.style')).fontSize),font);
  return overlay;
 }
 try{
  const meetingPreset=await create('meeting','QA Mẫu cuộc họp Soniox',26);
  assert.equal(await main.getByLabel('Mẫu cấu hình cuộc họp').inputValue(),meetingPreset.id);
  await main.getByLabel('Tên cuộc họp').fill('QA dùng mẫu — cuộc họp và phụ đề');
  await main.getByLabel('Thiết bị phát âm thanh cuộc họp').selectOption(initial.devices.find(d=>d.kind==='system'&&d.default).id);
  await main.getByLabel('Micro của bạn',{exact:true}).selectOption('');
  await main.getByRole('button',{name:'Bắt đầu ghi',exact:true}).click();await wait(async()=>!!(await api('snapshot')).active_id);ownId=(await api('snapshot')).active_id;
  await play();await wait(async()=>!!(await api('snapshot')).sessions.find(s=>s.id===ownId).translations.length);
  const overlay=await inspectOverlay(26);await overlay.screenshot({path:path.join(out,'meeting-overlay.png')});await main.getByLabel('Nội dung hiển thị').selectOption('bilingual');await main.screenshot({path:path.join(out,'meeting-live.png')});
  await main.locator('.record-controls').getByRole('button',{name:'Kết thúc',exact:true}).click();await main.getByRole('button',{name:'Kết thúc và lưu',exact:true}).click();await wait(async()=>!(await api('snapshot')).active_id);
  const meeting=(await api('snapshot')).sessions.find(s=>s.id===ownId);assert.equal(meeting.status,'completed');assert.equal(meeting.speech.connections,1);assert.equal(meeting.speech.translation_language,'en');assert(meeting.segments.length&&meeting.translations.length);
  result.meeting={passed:true,id:ownId,presetId:meetingPreset.id,model:meeting.speech.model,connections:meeting.speech.connections,originals:meeting.segments.length,translations:meeting.translations.length,fontSize:26,microphone:false};
  const captionPreset=await create('captionsonly','QA Mẫu chỉ phụ đề Soniox',32);
  assert.equal(await main.getByLabel('Mẫu cấu hình phụ đề').inputValue(),captionPreset.id);
  await main.getByLabel('Thiết bị loa/tai nghe').selectOption(initial.devices.find(d=>d.kind==='system'&&d.default).id);
  const count=(await api('snapshot')).sessions.length;
  await main.getByRole('button',{name:'Bắt đầu phụ đề',exact:true}).click();await wait(async()=>await api('caption_running'));await play();
  const standOverlay=await inspectOverlay(32);assert.equal((await api('caption_snapshot')).translating,true);await standOverlay.screenshot({path:path.join(out,'standalone-overlay.png')});await main.screenshot({path:path.join(out,'standalone-live.png')});
  await main.getByRole('button',{name:'Dừng phụ đề',exact:true}).click();await wait(async()=>!await api('caption_running'));
  const final=await api('snapshot');assert(!final.active_id);assert.equal(final.sessions.length,count);for(const old of initial.sessions)assert.deepEqual(final.sessions.find(s=>s.id===old.id),old);assert.deepEqual(await api('list_ai_profiles'),profiles);
  result.standalone={passed:true,presetId:captionPreset.id,model:'stt-rt-v5',fontSize:32,bilingual:true,noMeetingCreated:true,microphone:false};
  fs.writeFileSync(path.join(out,'results.json'),JSON.stringify(result,null,2));console.log('PASS real saved presets: meeting transcription+translation and standalone translated captions, speaker audio, mic off, styles applied, clean stops, original records preserved');
 }finally{
  await main.evaluate(()=>window.presetQAAudio?.pause()).catch(()=>{});
  const active=(await api('snapshot')).active_id;if(ownId&&active===ownId){await api('stop');await wait(async()=>!(await api('snapshot')).active_id)}
  if(!active&&await api('caption_running')){await api('stop_caption');await wait(async()=>!await api('caption_running'))}
  await api('hide_caption_window');
  await main.evaluate(async saved=>{for(const[k,v]of Object.entries(saved)){if(v===null)localStorage.removeItem(k);else localStorage.setItem(k,v)}const{emit}=await import('/node_modules/@tauri-apps/api/event.js');await emit('caption-style',{})},saved);
  await main.reload();await browser.close();
 }
})().catch(e=>{console.error(e);process.exitCode=1});
