// Opt-in native QA: uses an already saved Groq profile, public speaker audio, mic off.
const assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
(async()=>{
 const browser=await chromium.connectOverCDP('http://127.0.0.1:9223');
 const main=browser.contexts().flatMap(c=>c.pages()).find(p=>p.url().endsWith(':1420/'));assert(main,'Fresh development backend required');
 main.setDefaultTimeout(20000);
 const api=(cmd,args)=>main.evaluate(({cmd,args})=>window.__TAURI_INTERNALS__.invoke(cmd,args),{cmd,args});
 const wait=async(fn,ms=45000)=>{const until=Date.now()+ms;while(Date.now()<until){if(await fn())return;await main.waitForTimeout(150)}throw Error('Groq QA timed out')};
 const initial=await api('snapshot'),profiles=await api('list_ai_profiles');assert(!initial.active_id&&!await api('caption_running'),'Never interrupt a user session');
 const profile=profiles.find(p=>p.provider==='groq'&&p.has_key);assert(profile,'Saved Groq connection required');
 const keys=['innote.workflow-presets.v1','innote.caption.style','may.preferences','may.caption.standalone','may.task_assign.meeting','may.task_assign.captionsonly','may.speech.processing-levels','may.meeting.local-models'];
 const saved=await main.evaluate(keys=>Object.fromEntries(keys.map(k=>[k,localStorage.getItem(k)])),keys);
 const out=path.resolve('test-results/groq-native');fs.mkdirSync(out,{recursive:true});
 const result={profile:profile.name,meeting:null,standalone:null};let ownId;let ownStandalone=false;
 async function create(mode,name,model){
  await main.getByRole('button',{name:'Mẫu cấu hình',exact:true}).click();
  await main.getByRole('tab',{name:mode==='meeting'?'Cuộc họp':'Phụ đề',exact:true}).click();
  await main.getByRole('button',{name:'Tạo mẫu',exact:true}).click();
  await main.getByLabel('Tên mẫu',{exact:true}).fill(name);await main.getByLabel('Ngôn ngữ gốc của mẫu').selectOption('en');
  await main.getByLabel('Dịch sang trong mẫu').uncheck();
  if(mode==='meeting'){
   await main.getByLabel('Bật phụ đề khi bắt đầu',{exact:true}).check();
   await main.getByLabel('Chép lời (transcription)',{exact:true}).selectOption(profile.id);
   await main.getByLabel('Chép lời · Model',{exact:true}).selectOption(model);
  }else{
   await main.getByLabel('Phụ đề trực tiếp',{exact:true}).selectOption(profile.id);
   await main.getByLabel('Model phụ đề',{exact:true}).selectOption(model);
  }
  const local=main.getByLabel('Dự phòng local khi Soniox lỗi',{exact:true});
  if(await local.count())await local.uncheck();
  await main.getByRole('button',{name:'Lưu mẫu',exact:true}).click();
  const row=main.locator('.preset-row').filter({has:main.getByRole('heading',{name,exact:true})});await row.waitFor();
  const preset=await main.evaluate(name=>JSON.parse(localStorage.getItem('innote.workflow-presets.v1')).presets.find(p=>p.name===name),name);
  assert(preset);assert.equal((mode==='meeting'?preset.tasks.transcription:preset.tasks.caption).model,model);
  await row.getByRole('button',{name:'Sử dụng',exact:true}).click();await main.getByText(`Áp mẫu thành công: ${name}`,{exact:true}).waitFor();
  return preset;
 }
 async function play(){
  await main.evaluate(async file=>{const{convertFileSrc}=await import('/node_modules/@tauri-apps/api/core.js');const a=new Audio(convertFileSrc(file));window.groqQAAudio=a;window.groqQAEnd=new Promise((resolve,reject)=>{a.onended=resolve;a.onerror=()=>reject(Error('Speaker playback failed'))});await a.play()},path.resolve('test-results/fixtures/realtime/reference.wav'));
  await main.evaluate(()=>window.groqQAEnd);
 }
 try{
  const preset=await create('meeting','QA Groq — cuộc họp','whisper-large-v3-turbo');
  await main.reload();assert.equal(await main.getByLabel('Cấu hình Groq').inputValue(),profile.id);assert.equal(await main.getByLabel('Model chép lời Groq').inputValue(),'whisper-large-v3-turbo');
  await main.getByLabel('Mẫu cấu hình cuộc họp').selectOption(preset.id);await main.getByRole('button',{name:'Áp mẫu',exact:true}).click();
  await main.getByLabel('Tên cuộc họp').fill('QA Groq — bản chép dùng chung phụ đề');
  await main.getByLabel('Thiết bị phát âm thanh cuộc họp').selectOption(initial.devices.find(d=>d.kind==='system'&&d.default).id);
  await main.getByLabel('Micro của bạn',{exact:true}).selectOption('');
  await main.getByRole('button',{name:'Bắt đầu ghi',exact:true}).click();await wait(async()=>!!(await api('snapshot')).active_id);ownId=(await api('snapshot')).active_id;
  await play();await wait(async()=>!!(await api('snapshot')).sessions.find(s=>s.id===ownId).segments.length);
  assert(await api('caption_running'));const firstCaption=await api('caption_snapshot');assert.equal(firstCaption.status.provider,'groq');assert(firstCaption.caption.text.trim());
  await main.screenshot({path:path.join(out,'meeting-live.png')});
  // Turning the overlay off and on must attach to the existing uploader.
  await api('stop_caption');assert(!await api('caption_running'));
  await api('start_caption',{options:{provider:'',apiKey:'',baseUrl:'',model:'whisper-large-v3-turbo',targetLanguage:'vi',translate:false,sourceLanguage:'en',mode:'meeting',profileId:profile.id}});
  assert(await api('caption_running'));
  await assert.rejects(()=>api('set_meeting_translation',{target:'vi'}),/Groq|Soniox/);
  await api('pause',{paused:true});await main.waitForTimeout(800);await api('pause',{paused:false});
  await main.locator('.record-controls').getByRole('button',{name:'Kết thúc',exact:true}).click();await main.getByRole('button',{name:'Kết thúc và lưu',exact:true}).click();await wait(async()=>!(await api('snapshot')).active_id);
  const meeting=(await api('snapshot')).sessions.find(s=>s.id===ownId);assert.equal(meeting.status,'completed',meeting.error);assert.equal(meeting.provider,'groq');assert.equal(meeting.model,'whisper-large-v3-turbo');assert.equal(meeting.speech.connections,1);assert.equal(meeting.translations.length,0);assert(meeting.segments.length>0);
  assert(meeting.segments.every(s=>s.end>=s.start&&s.start>=0&&s.end<=meeting.duration+1));
  result.meeting={passed:true,id:ownId,model:meeting.model,segments:meeting.segments.length,connections:meeting.speech.connections,microphone:false,text:meeting.segments.map(s=>s.text).join(' '),warnings:meeting.warnings};
  await create('captionsonly','QA Groq — chỉ phụ đề','whisper-large-v3');
  await main.getByLabel('Thiết bị loa/tai nghe').selectOption(initial.devices.find(d=>d.kind==='system'&&d.default).id);
  const count=(await api('snapshot')).sessions.length;
  await main.getByRole('button',{name:'Bắt đầu phụ đề',exact:true}).click();await wait(async()=>await api('caption_running'));ownStandalone=true;
  await play();await wait(async()=>!!(await api('caption_snapshot')).caption.text.trim());const caption=await api('caption_snapshot');assert.equal(caption.status.provider,'groq');assert.equal(caption.translating,false);
  await main.screenshot({path:path.join(out,'standalone-live.png')});
  await main.getByRole('button',{name:'Dừng phụ đề',exact:true}).click();await wait(async()=>!await api('caption_running'));ownStandalone=false;
  const final=await api('snapshot');assert.equal(final.sessions.length,count);for(const old of initial.sessions)assert.deepEqual(final.sessions.find(s=>s.id===old.id),old);assert.deepEqual(await api('list_ai_profiles'),profiles);
  result.standalone={passed:true,model:'whisper-large-v3',noMeetingCreated:true,microphone:false,lastCaption:caption.caption.text};
  fs.writeFileSync(path.join(out,'results.json'),JSON.stringify(result,null,2));console.log('PASS Groq saved-key native QA: both Whisper models, meeting and standalone presets, shared overlay restart, pause/resume, clean drain; previous records and keys preserved');
 }finally{
  await main.evaluate(()=>window.groqQAAudio?.pause()).catch(()=>{});
  const active=(await api('snapshot')).active_id;
  if(ownId&&active===ownId){await api('stop');await wait(async()=>!(await api('snapshot')).active_id)}
  if(ownStandalone&&await api('caption_running')){await api('stop_caption');await wait(async()=>!await api('caption_running'))}
  await api('hide_caption_window');
  await main.evaluate(async saved=>{for(const[k,v]of Object.entries(saved)){if(v===null)localStorage.removeItem(k);else localStorage.setItem(k,v)}const{emit}=await import('/node_modules/@tauri-apps/api/event.js');await emit('caption-style',{})},saved);
  await main.reload();await browser.close();
 }
})().catch(e=>{console.error(e);process.exitCode=1});
