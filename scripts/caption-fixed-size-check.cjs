// Never inject test text into an active session. Observe it read-only instead.
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict'),fs=require('node:fs');
(async()=>{
 const browser=await chromium.connectOverCDP('http://127.0.0.1:9223');
 try{
  const pages=()=>browser.contexts().flatMap(c=>c.pages());const main=pages().find(p=>p.url().endsWith(':1420/'));assert(main);
  const api=cmd=>main.evaluate(cmd=>window.__TAURI_INTERNALS__.invoke(cmd),cmd);
  const active=!!(await api('snapshot')).active_id||await api('caption_running');const visible=await api('caption_window_visible');
  if(!active)await api('open_caption_window');
  const overlay=pages().filter(p=>p.url().includes('caption.html')).at(-1);assert(overlay,'No caption renderer to observe');
  await overlay.locator('.caption-root').waitFor({state:'attached'});await overlay.waitForTimeout(500);
  const measure=()=>overlay.evaluate(async()=>{const{getCurrentWebviewWindow}=await import('/node_modules/@tauri-apps/api/webviewWindow.js');const win=getCurrentWebviewWindow(),size=await win.innerSize();return {width:size.width,height:size.height,viewport:[innerWidth,innerHeight],root:[document.querySelector('.caption-root').clientWidth,document.querySelector('.caption-root').clientHeight],characters:document.querySelector('.caption-text')?.textContent?.length??0};});
  const before=await measure(),samples=[];
  const saved=await main.evaluate(()=>localStorage.getItem('innote.caption.style'));
  const previous={...await overlay.evaluate(()=>({text:document.querySelector('.caption-text')?.textContent??'',original:document.querySelector('.caption-original')?.textContent??'',final_:true})),translating:(await api('caption_snapshot')).translating??false};
  try{
   if(active){for(let n=0;n<20;n++){samples.push(await measure());await overlay.waitForTimeout(200);}}
   else{
    for(const bilingual of [false,true])for(const length of [40,2000,10000]){
     await main.evaluate(async({bilingual,length})=>{const{emit}=await import('/node_modules/@tauri-apps/api/event.js');const raw=JSON.parse(localStorage.getItem('innote.caption.style')||'{}');localStorage.setItem('innote.caption.style',JSON.stringify({...raw,showOriginal:bilingual}));await emit('caption-style',{});await emit('caption-text',{text:('Câu phụ đề rất dài 12345 中文 English. ').repeat(length).slice(0,length),original:bilingual?'Original sentence with many words. '.repeat(length).slice(0,length):'',final_:true,translating:bilingual});},{bilingual,length});
     await overlay.waitForFunction(length=>{const text=document.querySelector('.caption-lines .caption-text')?.textContent??'';return text.length>0&&text.length<=length},length);await overlay.waitForTimeout(100);samples.push({...await measure(),bilingual});
    }
   }
   for(const s of samples){assert.equal(s.width,before.width);assert.equal(s.height,before.height);assert(s.root[0]<=s.viewport[0]&&s.root[1]<=s.viewport[1]);}
   const result={mode:active?'read-only-live-session':'synthetic-preview-no-audio',before,samples,windowSizeUnchanged:true,passed:true};fs.mkdirSync('test-results/caption-fixed-size',{recursive:true});fs.writeFileSync('test-results/caption-fixed-size/results.json',JSON.stringify(result,null,2));console.log(JSON.stringify(result));
  }finally{
   if(!active){await main.evaluate(async({saved,previous})=>{const{emit}=await import('/node_modules/@tauri-apps/api/event.js');if(saved===null)localStorage.removeItem('innote.caption.style');else localStorage.setItem('innote.caption.style',saved);await emit('caption-style',{});await emit('caption-text',previous);},{saved,previous});if(!visible)await api('close_caption_window');}
  }
 }finally{await browser.close();}
})().catch(e=>{console.error(e);process.exitCode=1});
