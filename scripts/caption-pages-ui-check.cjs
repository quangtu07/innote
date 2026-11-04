// Isolated browser renderer QA: never changes a user's live caption window.
const assert=require('node:assert/strict'),fs=require('node:fs');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
(async()=>{
 const browser=await chromium.launch({channel:'chrome',headless:true});
 const mocks=fs.readFileSync('node_modules/@tauri-apps/api/mocks.js','utf8').replace(/export \{[^}]+\};/g,'');
 const output='test-results/caption-pages-ui';fs.mkdirSync(output,{recursive:true});
 const results=[];
 try{
  for(const [width,height,fontSize] of [[240,90,48],[420,150,32],[600,220,22]])for(const bilingual of [false,true]){
   const context=await browser.newContext({viewport:{width,height}});
   await context.addInitScript({content:mocks+`
    mockWindows('caption');
    localStorage.setItem('innote.caption.style',JSON.stringify({fontSize:${fontSize},showOriginal:${bilingual},color:'#ffce54',backgroundColor:'#172d29',transparent:false}));
    mockIPC((cmd)=>cmd==='caption_snapshot'?{status:{state:'idle',sending:false,provider:'',message:''},caption:{text:'',original:'',final_:true},translating:false,paired:null}:null,{shouldMockEvents:true});
   `});
   const page=await context.newPage();await page.goto('http://127.0.0.1:1420/caption.html');
   await page.locator('.caption-root').waitFor();await page.waitForTimeout(150);
   await page.evaluate(async bilingual=>{const{emit}=await import('/node_modules/@tauri-apps/api/event.js');await emit('caption-text',{text:'Đây là một câu rất dài cần hiển thị lần lượt trong hộp phụ đề nhỏ. Chúng ta cần đọc được từng phần mà không mất dòng đầu hay tràn ra ngoài hộp. '.repeat(3),original:bilingual?'This original sentence must advance together with its translation. Long bilingual captions remain inside the resized box. '.repeat(3):'',final_:true,translating:bilingual})},bilingual);
   await page.locator('.caption-lines .caption-text').waitFor();
   const measure=()=>page.evaluate(()=>{const root=document.querySelector('.caption-lines').getBoundingClientRect();return [...document.querySelectorAll('.caption-lines .caption-text,.caption-lines .caption-original')].map(el=>{const b=el.getBoundingClientRect();return {text:el.textContent,inside:b.left>=root.left-.5&&b.right<=root.right+.5&&b.top>=root.top-.5&&b.bottom<=root.bottom+.5,unclipped:el.scrollHeight<=el.clientHeight+1&&el.scrollWidth<=el.clientWidth+1}})});
   const before=await measure();assert.equal(before.length,bilingual?2:1);assert(before.every(m=>m.inside&&m.unclipped),JSON.stringify({width,height,bilingual,before}));
   const first=before[before.length-1].text;
   await page.waitForFunction(first=>document.querySelector('.caption-lines .caption-text')?.textContent!==first,first,{timeout:4000});
   const after=await measure();assert(after.every(m=>m.inside&&m.unclipped));
   assert.equal(await page.evaluate(()=>innerWidth),width);assert.equal(await page.evaluate(()=>innerHeight),height);
   await page.screenshot({path:`${output}/${width}x${height}-${bilingual?'bilingual':'translation'}.png`});
   results.push({width,height,fontSize,bilingual,passed:true});await context.close();
  }
  fs.writeFileSync(`${output}/results.json`,JSON.stringify(results,null,2));console.log('PASS 6 caption layouts: small/large fonts, fixed bounds, no clipped lines, continuation and bilingual paging');
 }finally{await browser.close()}
})().catch(e=>{console.error(e);process.exitCode=1});
