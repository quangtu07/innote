// Opt-in: saved Soniox profile, reference audio through Windows speakers, no mic.
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict');
(async()=>{
 const browser=await chromium.connectOverCDP('http://127.0.0.1:9223');
 const errors=[];for(const context of browser.contexts())context.on('page',p=>{p.on('pageerror',e=>errors.push(String(e)));p.on('console',m=>{if(m.type()==='error')errors.push(m.text());});});
 let page;
 for(let attempt=0;attempt<100;attempt++){
  page=browser.contexts().flatMap(c=>c.pages()).find(p=>p.url().endsWith(':1420/'));
  if(page)break;await new Promise(resolve=>setTimeout(resolve,100));
 }
 assert(page,'Development window did not load');
 const api=(cmd,args)=>page.evaluate(({cmd,args})=>window.__TAURI_INTERNALS__.invoke(cmd,args),{cmd,args});
 const wait=async(fn)=>{const deadline=Date.now()+20000;while(Date.now()<deadline){if(await fn())return;await page.waitForTimeout(100);}throw Error('Backend did not stop');};
 assert(!(await api('snapshot')).active_id&&!await api('caption_running'),'Refuse to interrupt a session');
 const profile=(await api('list_ai_profiles')).find(p=>p.provider==='soniox');assert(profile);
 const saved=await page.evaluate(()=>localStorage.getItem('may.caption.standalone'));
 const out=path.resolve('test-results/caption-latency');fs.mkdirSync(out,{recursive:true});const results=[];
 try{
  for(let trial=0;trial<Number(process.env.CAPTION_TRIALS||2);trial++){
   const setupStart=Date.now();
   await page.evaluate(async()=>{const{listen}=await import('/node_modules/@tauri-apps/api/event.js');window.captionAudit={events:[],started:0};window.captionAuditOff=await listen('caption-text',({payload})=>{if(window.captionAudit.started)window.captionAudit.events.push({ms:performance.now()-window.captionAudit.started,...payload});});});
   if(process.env.CAPTION_UI==='1'){
    await page.getByRole('button',{name:'Phụ đề',exact:true}).click();
    await page.getByLabel('Dịch sang',{exact:true}).setChecked(process.env.CAPTION_SOURCE!=='1');
    await page.getByLabel('Ngôn ngữ gốc').selectOption('en');
    await page.getByLabel('Thiết bị loa/tai nghe').selectOption('');
    await page.getByLabel('Cấu hình AI phụ đề').selectOption(profile.id);
    await page.getByRole('button',{name:'Bắt đầu phụ đề',exact:true}).click();
    await wait(async()=>await api('caption_running'));
   }else await api('start_caption',{options:{provider:'',apiKey:'',baseUrl:'',model:'stt-rt-v5',targetLanguage:'vi',translate:process.env.CAPTION_SOURCE!=='1',sourceLanguage:'en',device:null,mode:'standalone',profileId:profile.id,sessionDir:null}});
   await api('open_caption_window');
   // WebView2 CDP can retain detached targets after native close. The latest
   // target must also acknowledge an event before it is used as render evidence.
   let overlay;await wait(()=>{overlay=browser.contexts().flatMap(c=>c.pages()).filter(p=>p.url().includes('caption.html')).at(-1);return !!overlay;});
   await overlay.waitForSelector('.caption-root');
   const probe=`caption-audit-${Date.now()}`;
   await overlay.evaluate(async probe=>{window.captionProbe=false;const{once}=await import('/node_modules/@tauri-apps/api/event.js');await once(probe,()=>window.captionProbe=true)},probe);
   await page.evaluate(async probe=>{const{emit}=await import('/node_modules/@tauri-apps/api/event.js');await emit(probe,{})},probe);
   await wait(()=>overlay.evaluate(()=>window.captionProbe));
   await wait(async()=>{const s=await api('caption_snapshot');return s.status.sending&&s.status.provider==='soniox'});
   const setupMs=Date.now()-setupStart;
   await overlay.evaluate(async()=>{window.auditDom=[];window.auditManual=[];const{listen}=await import('/node_modules/@tauri-apps/api/event.js');window.auditManualOff=await listen('caption-text',e=>window.auditManual.push(e.payload));window.auditObserver=new MutationObserver(()=>{const text=document.querySelector('.caption-text')?.textContent?.trim();if(text)window.auditDom.push({at:Date.now(),text});});window.auditObserver.observe(document.querySelector('.caption-root'),{subtree:true,childList:true,characterData:true});});
   // Let capture connect before reference speech; first-text metric excludes setup.
   await page.waitForTimeout(1600);
   await page.evaluate(async file=>{const{convertFileSrc}=await import('/node_modules/@tauri-apps/api/core.js');window.auditAudio=new Audio(convertFileSrc(file));const ended=new Promise((resolve,reject)=>{window.auditAudio.onended=resolve;window.auditAudio.onerror=reject;});window.captionAudit.started=performance.now();window.captionAudit.epoch=Date.now();await window.auditAudio.play();await ended;},path.resolve('test-results/fixtures/english.wav'));
   await page.waitForTimeout(2200);
   await overlay.screenshot({path:path.join(out,`${process.env.CAPTION_LABEL||'current'}-${trial+1}-overlay.png`)});
   if(process.env.CAPTION_UI==='1')await page.screenshot({path:path.join(out,`${process.env.CAPTION_LABEL}-${trial+1}-app.png`)});
   await api('stop_caption');await wait(async()=>!await api('caption_running'));
   assert.equal((await api('caption_snapshot')).status.state,'idle','A deliberate stop must not report audio-source failure');
   const audit=await page.evaluate(()=>{window.captionAuditOff();return window.captionAudit});
   const firstOriginal=audit.events.find(e=>e.original?.trim()||process.env?.CAPTION_SOURCE==='1');
   const firstText=audit.events.find(e=>e.text?.trim());
   const rendered=await overlay.evaluate(()=>{window.auditObserver.disconnect();return window.auditDom});
   const firstDom=rendered.find(e=>e.at>=audit.epoch);
   if(!firstDom)fs.writeFileSync(path.join(out,'render-failure.json'),JSON.stringify({audit,rendered,errors,url:overlay.url(),manual:await overlay.evaluate(()=>window.auditManual),dom:await overlay.locator('.caption-root').innerText(),metadata:await overlay.evaluate(()=>window.__TAURI_INTERNALS__.metadata),provider:await api('caption_snapshot')},null,2));
   assert(firstText,'No visible caption text');
   assert(firstDom,'No caption text rendered in floating window');
   results.push({trial:trial+1,setupMs,firstOriginalMs:firstOriginal?.ms??null,firstTextMs:firstText.ms,firstDomMs:firstDom.at-audit.epoch,events:audit.events.length,finalTexts:audit.events.filter(e=>e.final_).map(e=>e.text),sourceOnly:process.env.CAPTION_SOURCE==='1'});
   await api('close_caption_window');
  }
 }finally{await page.evaluate(()=>{window.auditAudio?.pause();window.captionAuditOff?.()}).catch(()=>{});await api('stop_caption');await wait(async()=>!await api('caption_running'));await api('close_caption_window');await page.evaluate(saved=>{if(saved===null)localStorage.removeItem('may.caption.standalone');else localStorage.setItem('may.caption.standalone',saved)},saved);fs.writeFileSync(path.join(out,(process.env.CAPTION_LABEL||'current')+'.json'),JSON.stringify(results,null,2));console.log(JSON.stringify(results,null,2));await browser.close();}
})().catch(e=>{console.error(e);process.exitCode=1});
