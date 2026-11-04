// Smoke test an extracted installer build (not Vite). Speaker fixture, no mic/key extraction.
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict');
(async()=>{
 const browser=await chromium.connectOverCDP('http://127.0.0.1:9223');
 const pages=()=>browser.contexts().flatMap(c=>c.pages());
 const page=pages().find(p=>/^https?:\/\/tauri\.localhost\/?$/.test(p.url()));assert(page,'Launch extracted release with CDP');
 const api=(cmd,args)=>page.evaluate(({cmd,args})=>window.__TAURI_INTERNALS__.invoke(cmd,args),{cmd,args});
 const wait=async fn=>{const until=Date.now()+30000;while(Date.now()<until){if(await fn())return;await page.waitForTimeout(100);}throw Error('Packaged caption timeout');};
 const before=await api('snapshot');assert(!before.active_id&&!await api('caption_running'));
 const profiles=await api('list_ai_profiles');let ownsCaption=false;
 try{
  if(process.env.PACKAGE_UPGRADE_FROM){
    await page.evaluate(version=>localStorage.setItem('innote.release-notice.v1',JSON.stringify({version,pending:null})),process.env.PACKAGE_UPGRADE_FROM);
    await page.reload();
  }
  assert(before.models.some(m=>m.id==='moonshine-en'&&m.installed));
  assert(!before.models.some(m=>m.id==='phowhisper-small'));
  const releaseNotice=page.getByRole('dialog',{name:'Có gì mới trong Innote?',exact:true});
  await releaseNotice.waitFor();assert((await releaseNotice.innerText()).includes(process.env.RELEASE_VERSION||'v'+JSON.parse(fs.readFileSync('package.json','utf8')).version));
  await releaseNotice.getByRole('button',{name:'Đóng thông báo cập nhật',exact:true}).click();
  await page.reload();await page.waitForTimeout(500);
  assert.equal(await page.locator('dialog.whats-new[open]').count(),0,'Dismissal must persist in the actual production WebView');
  await page.getByRole('button',{name:'Cài đặt',exact:true}).click();
  const trigger=page.getByRole('button',{name:'Thêm cấu hình',exact:true});await trigger.click();
  const dialog=page.getByRole('dialog',{name:'Thêm cấu hình',exact:true});await dialog.waitFor();
  assert.equal(await dialog.evaluate(e=>e.tagName),'DIALOG','Installer must contain current keyboard-safe dialog');
  await page.screenshot({path:'test-results/final-review-packaged-dialog.png'});
  await page.keyboard.press('Escape');await dialog.waitFor({state:'detached'});
  await api('open_caption_window');
  await wait(async()=>pages().some(p=>p.url().includes('caption.html')));
  const overlay=pages().find(p=>p.url().includes('caption.html'));assert(overlay);
  assert(/^https?:\/\/tauri\.localhost\/caption.html/.test(overlay.url()),'Production caption window must use bundled assets, never Vite');
  await overlay.route('http://127.0.0.1:1420/**',route=>route.abort());
  await api('start_caption',{options:{provider:'',apiKey:'',baseUrl:'',profileId:'__local__',model:'moonshine-en',sourceLanguage:'en',targetLanguage:'vi',translate:false,speechQuality:'balanced',mode:'standalone',device:null,sessionDir:null}});ownsCaption=true;
  await wait(async()=>{const s=(await api('caption_snapshot')).status;assert.notEqual(s.state,'error',s.message);return s.state==='active';});
  await page.evaluate(async file=>{
   const url=window.__TAURI_INTERNALS__.convertFileSrc(file,'asset');
   window.packageFixture=new Audio(url);
   const done=new Promise((resolve,reject)=>{window.packageFixture.onended=resolve;window.packageFixture.onerror=()=>reject(Error('Fixture playback failed'));});
   await window.packageFixture.play();await done;
  },path.resolve('test-results/fixtures/realtime/reference.wav'));
  const caption=await api('caption_snapshot');assert(caption.caption.text.trim());assert.equal(caption.status.provider,'local');assert(!caption.status.sending);
  await wait(async()=>!!(await overlay.locator('.caption-text').textContent().catch(()=>''))?.trim());
  await api('stop_caption');await wait(async()=>!await api('caption_running'));ownsCaption=false;
  assert.equal((await api('caption_snapshot')).status.state,'idle');
  const after=await api('snapshot');assert.deepEqual(after.sessions,before.sessions);assert.deepEqual(await api('list_ai_profiles'),profiles);
  const result={packagedAssets:true,bundledCaptionUrl:overlay.url(),overlayRendered:true,currentDialog:true,upgradeNoticeOnce:true,bundledMoonshine:true,noPhoWhisper:true,speakerCaptions:true,cleanStop:true,existingDataPreserved:true,passed:true};
  fs.writeFileSync('test-results/final-review-package-smoke.json',JSON.stringify(result,null,2));console.log(JSON.stringify(result));
 }finally{
  await page.evaluate(()=>window.packageFixture?.pause()).catch(()=>{});
  if(ownsCaption){await api('stop_caption').catch(()=>{});await wait(async()=>!await api('caption_running')).catch(()=>{});}
  await api('close_caption_window').catch(()=>{});await browser.close();
 }
})().catch(e=>{console.error(e);process.exitCode=1});
