// Opt-in test of the actual Windows WebView. Presentation fixtures are not API accuracy evidence.
const assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
async function run(){
 const b=await chromium.connectOverCDP('http://127.0.0.1:9223');
 const main=b.contexts().flatMap(c=>c.pages()).find(p=>p.url().endsWith(':1420/'));
 const invoke=(cmd,args)=>main.evaluate(({cmd,args})=>window.__TAURI_INTERNALS__.invoke(cmd,args),{cmd,args});
 assert(!(await invoke('snapshot')).active_id&&!await invoke('caption_running'),'Never interrupt a user session');
 const saved=await main.evaluate(()=>({style:localStorage.getItem('innote.caption.style'),bounds:localStorage.getItem('innote.caption.bounds')}));
 const out=path.resolve('test-results/caption-direct-drag');fs.mkdirSync(out,{recursive:true});
 const results=[];let cap,nativeBounds;
 try{
  await main.evaluate(()=>{localStorage.removeItem('innote.caption.style');localStorage.removeItem('innote.caption.bounds');localStorage.removeItem('innote.caption.editing')});
  await main.reload();await main.getByRole('button',{name:'Cài đặt',exact:true}).click();
  await main.locator('#caption-settings').scrollIntoViewIfNeeded();
  await main.getByText('Tùy chỉnh chữ phụ đề',{exact:true}).click();
  await main.getByRole('button',{name:'Hiện hộp phụ đề',exact:true}).click();
  for(let i=0;i<40;i++){cap=b.contexts().flatMap(c=>c.pages()).find(p=>p.url().includes('caption.html'));if(cap)break;await main.waitForTimeout(100)}assert(cap);
  nativeBounds=await cap.evaluate(async()=>{const{getCurrentWebviewWindow}=await import('/node_modules/@tauri-apps/api/webviewWindow.js');const win=getCurrentWebviewWindow();return {position:await win.outerPosition(),size:await win.innerSize()};});
  await cap.locator('.caption-root').waitFor();
  const resize=async(w,h)=>{await cap.evaluate(async({w,h})=>{const{getCurrentWebviewWindow}=await import('/node_modules/@tauri-apps/api/webviewWindow.js');const{LogicalSize,LogicalPosition}=await import('/node_modules/@tauri-apps/api/dpi.js');const win=getCurrentWebviewWindow();await win.setPosition(new LogicalPosition(280,240));await win.setSize(new LogicalSize(w,h))},{w,h});await cap.waitForFunction(({w,h})=>Math.abs(innerWidth-w)<2&&Math.abs(innerHeight-h)<2,{w,h});};
  await resize(720,220);
  const emit=payload=>invoke('plugin:event|emit',{event:'caption-text',payload});
  await cap.waitForFunction(()=>document.querySelector(".caption-root")?.dataset.passthrough==="false");
  assert.equal(await cap.locator('button,input,select,.caption-bar,.caption-settings').count(),0);
  assert.equal(await cap.evaluate(()=>getComputedStyle(document.documentElement).backgroundColor),'rgba(0, 0, 0, 0)');
  assert.equal(await cap.locator('.caption-root').evaluate(e=>getComputedStyle(e).backgroundColor),'rgb(23, 45, 41)');
  await main.getByLabel('Màu nền hộp phụ đề').fill('#203040');await main.getByLabel('Màu nền hộp phụ đề').dispatchEvent('input');
  await cap.waitForFunction(()=>getComputedStyle(document.querySelector('.caption-root')).backgroundColor==='rgb(32, 48, 64)');
  await main.getByRole('checkbox',{name:'Trong suốt',exact:true}).check();
  await cap.waitForFunction(()=>getComputedStyle(document.querySelector('.caption-root')).backgroundColor==='rgba(0, 0, 0, 0)');
  assert(await main.getByLabel('Màu nền hộp phụ đề').isDisabled());
  await main.getByRole('checkbox',{name:'Trong suốt',exact:true}).uncheck();
  await cap.waitForFunction(()=>getComputedStyle(document.querySelector('.caption-root')).backgroundColor==='rgb(32, 48, 64)');
  results.push({name:'Default visible background; custom color; transparency toggle restores remembered color',passed:true});
  await emit({text:'',original:'ORIGINAL MUST STAY HIDDEN',final_:false});await cap.waitForTimeout(100);
  assert(!(await cap.locator('.caption-root').innerText()).includes('ORIGINAL MUST STAY HIDDEN'));
  const sample={text:'DỮ LIỆU THỬ: ngân sách là 15 triệu đồng, không phải 50 triệu.',original:'TEST: fifteen million, not fifty million.',final_:true};
  await emit(sample);await cap.locator('.caption-text').waitFor();assert.equal(await cap.locator('.caption-original').count(),0);
  await cap.screenshot({path:path.join(out,'translation-only.png'),omitBackground:true});
  await main.getByRole('checkbox',{name:'Song ngữ',exact:true}).check();await cap.locator('.caption-original').waitFor();
  await main.getByLabel('Cỡ chữ phụ đề').fill('30');await main.getByLabel('Cỡ chữ phụ đề').dispatchEvent('input');
  await main.getByLabel('Căn chữ phụ đề').selectOption('left');
  await main.getByLabel('Màu chữ phụ đề').fill('#ffd166');await main.getByLabel('Màu chữ phụ đề').dispatchEvent('input');
  await cap.waitForFunction(()=>getComputedStyle(document.querySelector('.caption-text')).fontSize==='30px'&&getComputedStyle(document.querySelector('.caption-text')).color==='rgb(255, 209, 102)');
  await cap.screenshot({path:path.join(out,'bilingual.png'),omitBackground:true});
  await main.reload();await main.getByRole('button',{name:'Cài đặt',exact:true}).click();await main.locator('#caption-settings').scrollIntoViewIfNeeded();await main.getByText('Tùy chỉnh chữ phụ đề',{exact:true}).click();
  assert(await main.getByRole('checkbox',{name:'Song ngữ',exact:true}).isChecked());
  assert.equal(await main.getByLabel('Màu nền hộp phụ đề').inputValue(),'#203040');
  assert.equal(await main.getByRole('checkbox',{name:'Trong suốt',exact:true}).isChecked(),false);
  results.push({name:'No overlay controls; native mouse input enabled; main controls and background persist',passed:true});
  await cap.waitForFunction(()=>document.querySelector(".caption-root")?.dataset.passthrough==="false");await cap.locator('.resize').first().waitFor();
  await invoke('plugin:event|emit',{event:'caption-layout-mode',payload:{editing:false}});
  assert.equal(await cap.locator('.resize').count(),4);
  for(const[n,c]of [['nw','.north.west'],['ne','.north:not(.west)'],['sw','.west:not(.north)'],['se',':not(.west):not(.north)']]){
   await resize(720,220);const r=await cap.locator('.resize'+c).boundingBox();const x=r.x+r.width/2,y=r.y+r.height/2;
   await cap.mouse.move(x,y);await cap.mouse.down();await cap.waitForTimeout(50);await cap.mouse.move(x+(n.includes('w')?20:-20),y+(n.includes('n')?14:-14));await cap.mouse.up();await cap.waitForFunction(()=>innerWidth<715&&innerHeight<215,null,{timeout:3000});
   await resize(720,220);const grow=await cap.locator('.resize'+c).boundingBox();const gx=grow.x+grow.width/2,gy=grow.y+grow.height/2;
   await cap.mouse.move(gx,gy);await cap.mouse.down();await cap.waitForTimeout(50);await cap.mouse.move(gx+(n.includes('w')?-20:20),gy+(n.includes('n')?-14:14));await cap.mouse.up();await cap.waitForFunction(()=>innerWidth>725&&innerHeight>225,null,{timeout:3000});
   results.push({name:'Actual WebView resize shrink and grow via CDP pointer '+n,passed:true});
  }
  await resize(720,220);
  const position=()=>cap.evaluate(async()=>{const{getCurrentWebviewWindow}=await import('/node_modules/@tauri-apps/api/webviewWindow.js');return await getCurrentWebviewWindow().outerPosition()});
  const before=await position();await cap.mouse.move(60,50);await cap.mouse.down();await cap.waitForTimeout(50);await cap.mouse.move(90,70);await cap.mouse.up();await cap.waitForTimeout(100);const after=await position();assert(after.x>before.x+5&&after.y>before.y+5);results.push({name:'Move directly without enabling an edit mode',passed:true});
  assert.equal(await cap.locator(".caption-root").getAttribute("data-passthrough"),"false");
  await main.getByRole('checkbox',{name:'Trong suốt',exact:true}).check();
  await cap.waitForFunction(()=>getComputedStyle(document.querySelector('.caption-root')).backgroundColor==='rgba(0, 0, 0, 0)');
  const transparentBefore=await position();await cap.mouse.move(60,50);await cap.mouse.down();await cap.waitForTimeout(50);await cap.mouse.move(80,65);await cap.mouse.up();await cap.waitForTimeout(100);const transparentAfter=await position();assert(transparentAfter.x>transparentBefore.x+5&&transparentAfter.y>transparentBefore.y+5);
  results.push({name:'Direct dragging also works with transparent background',passed:true});
  await emit({...sample,text:sample.text.repeat(12),original:sample.original.repeat(10)});
  for(const[w,h]of [[420,180],[300,140],[180,90]]){await resize(w,h);const r=await cap.locator('.caption-text').boundingBox();assert(r.height>0&&r.y+r.height<=h+1);await cap.screenshot({path:path.join(out,`long-${w}x${h}.png`),omitBackground:true});results.push({size:[w,h],passed:true})}
  await invoke('plugin:event|emit',{event:'caption-status',payload:{state:'idle',sending:false,provider:'',message:'Stopped'}});await cap.waitForFunction(()=>!document.querySelector('.caption-text'));
  results.push({name:'Direct dragging stays enabled; stop removes old translation',passed:true});
  await main.screenshot({path:path.join(out,'controls-in-innote.png')});
  fs.writeFileSync(path.join(out,'results.json'),JSON.stringify(results,null,2));console.log(JSON.stringify(results,null,2));
 }finally{
  if(cap&&nativeBounds)await cap.evaluate(async bounds=>{const{getCurrentWebviewWindow}=await import('/node_modules/@tauri-apps/api/webviewWindow.js');const{PhysicalSize,PhysicalPosition}=await import('/node_modules/@tauri-apps/api/dpi.js');const win=getCurrentWebviewWindow();await win.setSize(new PhysicalSize(bounds.size.width,bounds.size.height));await win.setPosition(new PhysicalPosition(bounds.position.x,bounds.position.y));},nativeBounds).catch(()=>{});
  await invoke('close_caption_window').catch(()=>{});
  await main.evaluate(saved=>{for(const[n,k]of [['style','innote.caption.style'],['bounds','innote.caption.bounds']]){if(saved[n]===null)localStorage.removeItem(k);else localStorage.setItem(k,saved[n]);}localStorage.removeItem('innote.caption.editing');},saved);
  await main.reload();await b.close();
 }
}
module.exports=run;if(require.main===module)run().catch(e=>{console.error(e);process.exitCode=1});
