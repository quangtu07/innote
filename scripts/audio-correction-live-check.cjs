// Opt-in: correct only a labeled QA recording using already saved API profiles.
const assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
(async()=>{
 const browser=await chromium.connectOverCDP('http://127.0.0.1:9223');const page=browser.contexts().flatMap(c=>c.pages()).find(p=>p.url().endsWith(':1420/'));assert(page);
 const api=(cmd,args)=>page.evaluate(({cmd,args})=>window.__TAURI_INTERNALS__.invoke(cmd,args),{cmd,args});
 const initial=await api('snapshot');assert(!initial.active_id&&!await api('caption_running')&&!initial.corrections?.length,'Never interrupt an existing job/session');
 const target=initial.sessions.find(s=>s.title.startsWith('QA Groq — bản chép dùng chung phụ đề'));assert(target,'Need the existing English QA recording; never modify a user meeting');
 const profiles=await api('list_ai_profiles'),groq=profiles.find(p=>p.provider==='groq'&&p.has_key),soniox=profiles.find(p=>p.provider==='soniox'&&p.has_key);assert(groq&&soniox);
 const audio=await api('audio_path',{id:target.id}),hash=()=>crypto.createHash('sha256').update(fs.readFileSync(audio)).digest('hex'),audioHash=hash();
 const keys=['innote.workflow-presets.v1','may.task_assign.meeting','may.preferences','innote.caption.style','may.speech.processing-levels','may.meeting.local-models'];
 const saved=await page.evaluate(keys=>Object.fromEntries(keys.map(k=>[k,localStorage.getItem(k)])),keys);
 const out=path.resolve('test-results/audio-correction-native');fs.mkdirSync(out,{recursive:true});let ownJob=false;const results={target:target.id};
 async function openTarget(){await page.getByRole('button',{name:'Danh sách cuộc họp',exact:true}).click();await page.locator('dialog.meetings-dialog .meeting-card').filter({hasText:target.title}).locator('.meeting-card-main').click();await page.locator('audio.playback').waitFor();}
 try{
  await page.getByRole('button',{name:'Mẫu cấu hình',exact:true}).click();await page.getByRole('tab',{name:'Cuộc họp',exact:true}).click();await page.getByRole('button',{name:'Tạo mẫu',exact:true}).click();
  await page.getByLabel('Tên mẫu',{exact:true}).fill('QA — model hiệu chỉnh riêng');await page.getByLabel('Ngôn ngữ gốc của mẫu').selectOption('en');await page.getByLabel('Dịch sang trong mẫu').uncheck();
  await page.getByLabel('Chép lời (transcription)',{exact:true}).selectOption(soniox.id);
  await page.getByRole('button',{name:'Lưu mẫu',exact:true}).click();await page.reload();await page.getByRole('button',{name:'Mẫu cấu hình',exact:true}).click();
  await page.locator('.preset-row').filter({hasText:'QA — model hiệu chỉnh riêng'}).getByRole('button',{name:'Sử dụng',exact:true}).click();
  const assignments=await page.evaluate(()=>JSON.parse(localStorage.getItem('may.task_assign.meeting')));assert.equal(assignments.transcription.model,'stt-rt-v6');assert.equal(assignments.correction,undefined);assert.equal(assignments.summary,undefined);
  await openTarget();await page.getByRole('button',{name:target.corrected?'Hiệu chỉnh lại':'Hiệu chỉnh',exact:true}).click();const d=page.getByRole('dialog',{name:'Hiệu chỉnh từ file ghi âm'});
  assert.equal(await d.getByLabel('Cấu hình AI hiệu chỉnh').inputValue(),'__local__');await d.getByLabel('Cấu hình AI hiệu chỉnh').selectOption(groq.id);await d.getByLabel('Model hiệu chỉnh',{exact:true}).selectOption('whisper-large-v3');
  await d.screenshot({path:path.join(out,'model-picker.png')});ownJob=true;await d.getByRole('button',{name:target.corrected?'Hiệu chỉnh lại':'Bắt đầu hiệu chỉnh',exact:true}).click();
  await d.waitFor({state:'hidden',timeout:150000});await page.getByRole('tab',{name:'Bản hiệu chỉnh',exact:true}).waitFor({timeout:150000});ownJob=false;
  const first=(await api('snapshot')).sessions.find(s=>s.id===target.id);assert.equal(first.corrected.provider,'groq');assert.equal(first.corrected.model,'whisper-large-v3');assert(first.corrected.segments.length);assert.equal(first.corrected.audio_sha256,audioHash);assert.deepEqual(first.segments,target.segments);assert.equal(first.notes,target.notes);
  await page.screenshot({path:path.join(out,'groq-result.png')});
  await page.locator('.smooth-segment .timestamp').first().click();assert(Math.abs(await page.locator('audio.playback').evaluate(a=>a.currentTime)-first.corrected.segments[0].start)<.5);await page.locator('audio.playback').evaluate(a=>a.pause());
  const exported=await api('export_session',{id:target.id,format:'srt',path:path.join(out,'corrected.srt'),version:'corrected'});assert(fs.readFileSync(exported,'utf8').includes(first.corrected.segments[0].text));
  await page.reload();await openTarget();await page.getByRole('tab',{name:'Bản hiệu chỉnh',exact:true}).click();assert((await page.locator('.smooth-result').innerText()).trim());
  await assert.rejects(()=>api('correct_transcript',{id:target.id,profileId:groq.id,model:'openai/gpt-oss-20b',fallback:null}),/âm thanh|model chat/);assert.deepEqual((await api('snapshot')).sessions.find(s=>s.id===target.id).corrected,first.corrected);
  results.groq={passed:true,model:first.corrected.model,segments:first.corrected.segments.length,text:first.corrected.text};
  if(process.env.CORRECTION_LOCAL_REAL==='1'){
   results.local=[];
   for(const model of ['moonshine-en','nemotron-3.5-q8']){
    await page.getByRole('button',{name:'Hiệu chỉnh lại',exact:true}).click();const d=page.getByRole('dialog',{name:'Hiệu chỉnh từ file ghi âm'});await d.getByLabel('Cấu hình AI hiệu chỉnh').selectOption('__local__');await d.getByLabel('Model hiệu chỉnh',{exact:true}).selectOption(model);const start=Date.now();ownJob=true;await d.getByRole('button',{name:'Hiệu chỉnh lại',exact:true}).click();await d.waitFor({state:'hidden',timeout:150000});ownJob=false;
    const local=(await api('snapshot')).sessions.find(s=>s.id===target.id);assert.equal(local.corrected.provider,'local');assert.equal(local.corrected.model,model);assert.equal(local.corrected.audio_sha256,audioHash);assert(local.corrected.segments.length);assert.deepEqual(local.segments,target.segments);assert(local.corrected.segments.every(s=>s.start>=0&&s.end>=s.start&&s.end<=local.duration+1));
    results.local.push({passed:true,model,wallMs:Date.now()-start,segments:local.corrected.segments.length,text:local.corrected.text});await page.screenshot({path:path.join(out,model+'-result.png')});
   }
   const previous=(await api('snapshot')).sessions.find(s=>s.id===target.id).corrected;
   const pending=api('correct_transcript',{id:target.id,profileId:'__local__',model:'moonshine-en',fallback:null}).then(()=>null,e=>String(e));ownJob=true;
   for(let n=0;n<50;n++){if((await api('snapshot')).corrections.length)break;await page.waitForTimeout(50)}await api('cancel_correction',{id:target.id});const cancelled=await pending;ownJob=false;assert(cancelled&&cancelled.includes('hủy'));assert.deepEqual((await api('snapshot')).sessions.find(s=>s.id===target.id).corrected,previous);results.localCancel={passed:true};
  }
  await page.getByRole('button',{name:'Hiệu chỉnh lại',exact:true}).click();const second=page.getByRole('dialog',{name:'Hiệu chỉnh từ file ghi âm'});await second.getByLabel('Cấu hình AI hiệu chỉnh').selectOption(soniox.id);await second.getByLabel('Model hiệu chỉnh',{exact:true}).selectOption('stt-async-v6');
  ownJob=true;await second.getByRole('button',{name:'Hiệu chỉnh lại',exact:true}).click();await second.waitFor({state:'hidden',timeout:180000});ownJob=false;
  const after=(await api('snapshot')).sessions.find(s=>s.id===target.id);assert.equal(after.corrected.model,'stt-async-v6');assert.equal(after.corrected.provider,'soniox');assert(after.corrected.segments.length);assert.equal(after.corrected.audio_sha256,audioHash);assert.deepEqual(after.segments,target.segments);
  await page.screenshot({path:path.join(out,'soniox-result.png')});results.soniox={passed:true,model:after.corrected.model,segments:after.corrected.segments.length,text:after.corrected.text};
  if(process.env.CORRECTION_SUMMARY_REAL==='1'){
   const summary=await api('summarize_transcript',{id:target.id,provider:'groq',profileId:groq.id,model:'openai/gpt-oss-20b',baseUrl:'',prompt:'',style:'overview',source:'corrected',key:null,fallback:null});assert.equal(summary.summary.source,'corrected');assert(summary.summary.text.trim());results.summary={passed:true,source:'corrected',model:summary.summary.model};
  }
  const final=await api('snapshot');assert(!final.corrections.length);assert.equal(hash(),audioHash);for(const old of initial.sessions){const s=final.sessions.find(s=>s.id===old.id);assert.deepEqual(s.segments,old.segments);assert.deepEqual(s.translations,old.translations);assert.equal(s.notes,old.notes);if(old.id!==target.id)assert.deepEqual(s,old);}assert.deepEqual(await api('list_ai_profiles'),profiles);
  fs.writeFileSync(path.join(out,'results.json'),JSON.stringify(results,null,2));console.log('PASS native audio correction: operation-local model choices, Groq and Soniox file recognition, saved separate timed result, replay/export/reload, rejected chat model; originals/audio/keys unchanged');
 }finally{
  if(ownJob){await api('cancel_correction',{id:target.id});for(let n=0;n<100;n++){if(!(await api('snapshot')).corrections?.length)break;await page.waitForTimeout(100)}}
  await page.evaluate(saved=>{for(const[k,v]of Object.entries(saved)){if(v===null)localStorage.removeItem(k);else localStorage.setItem(k,v)}},saved);await page.reload();await browser.close();
 }
})().catch(e=>{console.error(e);process.exitCode=1});
