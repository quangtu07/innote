// Opt-in production smoke on a dedicated fixture. No user data, microphone or cloud APIs.
const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
(async()=>{
 const version=require('../package.json').version;
 const root=path.resolve(`test-results/release-${version}-fixture`);
 const browser=await chromium.connectOverCDP('http://127.0.0.1:9224');
 let page;const startupDeadline=Date.now()+15000;
 while(!page&&Date.now()<startupDeadline){page=browser.contexts().flatMap(c=>c.pages()).find(p=>/^https?:\/\/tauri\.localhost\/?$/.test(p.url()));if(!page)await new Promise(resolve=>setTimeout(resolve,100));}
 if(!page){await browser.close();throw Error('Packaged production origin required');}
 const api=(cmd,args={})=>page.evaluate(({cmd,args})=>window.__TAURI_INTERNALS__.invoke(cmd,args),{cmd,args});
 const initial=await api('snapshot');assert.equal(path.resolve(initial.data_dir).toLowerCase(),path.join(root,'private').toLowerCase());
 assert(!initial.active_id&&!await api('caption_running'));assert.equal((await api('list_ai_profiles')).length,0);
 let own=false;
 try{
  await page.evaluate(()=>{localStorage.setItem('may.ui_locale','vi');localStorage.setItem('innote.release-notice.v1',JSON.stringify({version:'0.8.8',pending:null}));});await page.reload();
  const dialog=page.getByRole('dialog',{name:'Có gì mới trong Innote?',exact:true});await dialog.waitFor();assert((await dialog.innerText()).includes('v'+version));
  await page.screenshot({path:path.join(root,'production-whats-new.png')});
  await dialog.getByRole('button',{name:'Đóng thông báo cập nhật',exact:true}).click();await page.reload();await page.waitForTimeout(400);assert.equal(await page.locator('dialog.whats-new[open]').count(),0);
  const speaker=initial.devices.find(d=>d.kind==='system'&&d.default)||initial.devices.find(d=>d.kind==='system');assert(speaker,'System output required');
  const meeting=await api('start',{options:{title:'QA release storage',system:speaker.id,microphone:null,language:'en',model:'moonshine-en',transcribe:false,provider:'local',captions_enabled:false,translation_language:null}});own=true;
  await page.getByRole('button',{name:'Về phiên đang chạy',exact:true}).click();
  const toggle=page.locator('#meeting-caption-style-toggle');await toggle.waitFor();await toggle.click();const panel=page.locator('#meeting-caption-style');await panel.waitFor();
  await panel.getByRole('button',{name:'Đóng cài đặt phụ đề',exact:true}).click();await panel.waitFor({state:'detached'});assert.equal((await api('snapshot')).active_id,meeting.id);
  await api('edit_session',{id:meeting.id,title:'QA bản phát hành đổi tên',notes:null,segmentId:null,text:null});await page.waitForTimeout(1500);await api('stop');
  const deadline=Date.now()+30000;while((await api('snapshot')).active_id&&Date.now()<deadline)await page.waitForTimeout(100);assert(!(await api('snapshot')).active_id);own=false;
  const info=await api('meeting_files_status',{id:meeting.id});assert(info.path.includes('QA bản phát hành đổi tên'));assert(path.resolve(info.path).startsWith(path.join(root,'library')));
  for(const file of ['recording.wav','meeting.json','00 - ĐỌC TÔI TRƯỚC.txt','.innote-identity.json'])assert(fs.existsSync(path.join(info.path,file)),file);
  const denied=await page.evaluate(async file=>{try{return(await fetch(window.__TAURI_INTERNALS__.convertFileSrc(file,'asset'))).status}catch{return 'rejected'}},path.join(root,'private','meeting-storage.json'));assert(denied==='rejected'||denied>=400);
  const audio=await api('audio_path',{id:meeting.id});const media=await page.evaluate(file=>new Promise((resolve,reject)=>{const a=new Audio(window.__TAURI_INTERNALS__.convertFileSrc(file,'asset'));a.onloadedmetadata=()=>resolve({duration:a.duration});a.onerror=()=>reject(Error('WAV unavailable'));}),audio);assert(media.duration>1);
  const bundle=JSON.parse(fs.readFileSync(path.join(info.path,'meeting.json'),'utf8'));assert(!/api_key|base_url|profile_id/i.test(JSON.stringify(bundle)));
  const result={version,productionOrigin:page.url(),upgradeNoticeOnce:true,runtimeSettingsCollapse:true,meetingContinued:true,renamedFolder:true,recordingWav:true,guide:true,privateAssetDenied:denied,mediaDuration:media.duration,noCredentialsInBundle:true,passed:true};
  fs.writeFileSync(path.join(root,'smoke.json'),JSON.stringify(result,null,2));console.log(JSON.stringify(result));
 }finally{if(own)await api('stop').catch(()=>{});await browser.close();}
})().catch(e=>{console.error(e);process.exitCode=1});
