// Native shared Settings -> floating WebView style, preview without cloud upload.
const{chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path');
(async()=>{
 const browser=await chromium.connectOverCDP('http://127.0.0.1:9223');
 const pages=()=>browser.contexts().flatMap(c=>c.pages());
 const page=pages().find(p=>p.url().endsWith(':1420/'));
 const api=(cmd,args)=>page.evaluate(({cmd,args})=>window.__TAURI_INTERNALS__.invoke(cmd,args),{cmd,args});
 assert(!(await api('snapshot')).active_id&&!await api('caption_running'),'Do not interrupt a session');
 const saved=await page.evaluate(()=>localStorage.getItem('innote.caption.style'));
 const out=path.resolve('test-results/caption-settings');fs.mkdirSync(out,{recursive:true});
 try{
  await page.getByRole('button',{name:'Cài đặt',exact:true}).click();
  const section=page.locator('#caption-settings');await section.scrollIntoViewIfNeeded();
  await section.getByText('Tùy chỉnh chữ phụ đề',{exact:true}).click();
  await section.getByRole('button',{name:'Hiện hộp phụ đề',exact:true}).click();
  await page.waitForTimeout(700);const overlay=pages().filter(p=>p.url().includes('caption.html')).at(-1);assert(overlay);
  await overlay.locator('.caption-root').waitFor();
  await section.getByLabel('Trong suốt',{exact:true}).uncheck();
  await section.getByLabel('Màu nền hộp phụ đề').evaluate(input=>{input.value='#314253';input.dispatchEvent(new Event('input',{bubbles:true}));});
  await overlay.waitForFunction(()=>getComputedStyle(document.querySelector('.caption-root')).backgroundColor==='rgb(49, 66, 83)');
  await section.getByLabel('Cỡ chữ phụ đề').fill('34');await section.getByLabel('Cỡ chữ phụ đề').dispatchEvent('input');
  await overlay.waitForFunction(()=>document.querySelector('.caption-root').style.getPropertyValue('--font-size')==='34px');
  await section.getByLabel('Trong suốt',{exact:true}).check();
  await overlay.waitForFunction(()=>getComputedStyle(document.querySelector('.caption-root')).backgroundColor==='rgba(0, 0, 0, 0)');
  assert.equal(await api('caption_running'),false,'Preview/style never starts uploading');
  await page.screenshot({path:path.join(out,'shared-settings.png')});
  fs.writeFileSync(path.join(out,'results.json'),JSON.stringify({previewWithoutUpload:true,liveBackground:true,liveFont:true,transparent:true,passed:true},null,2));
  console.log('PASS native shared caption Settings: background, font, transparent, preview without upload');
 }finally{
  await api('close_caption_window');
  await page.evaluate(async saved=>{if(saved===null)localStorage.removeItem('innote.caption.style');else localStorage.setItem('innote.caption.style',saved);const{emit}=await import('/node_modules/@tauri-apps/api/event.js');await emit('caption-style',{});},saved);
  await page.reload();await browser.close();
 }
})().catch(e=>{console.error(e);process.exitCode=1});
