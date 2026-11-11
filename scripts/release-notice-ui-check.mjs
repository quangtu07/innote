import assert from 'node:assert/strict';
import {mkdirSync,writeFileSync,readFileSync} from 'node:fs';
import {pathToFileURL} from 'node:url';
const {chromium}=await import(process.env.PLAYWRIGHT_MODULE?pathToFileURL(process.env.PLAYWRIGHT_MODULE).href:'playwright');
const browser=await chromium.launch({channel:'chrome',headless:true}),results=[];
const version=JSON.parse(readFileSync('package.json','utf8')).version;
const base=process.env.UI_TEST_URL??'http://127.0.0.1:1420';
const key='innote.release-notice.v1',out='test-results/release-notice';mkdirSync(out,{recursive:true});
async function check(name,options,run){
 const context=await browser.newContext({viewport:options.viewport??{width:900,height:640}});
 if(options.state)await context.addInitScript(({key,state})=>{if(!localStorage.getItem(key))localStorage.setItem(key,JSON.stringify(state));},{key,state:options.state});
 const p=await context.newPage();p.setDefaultTimeout(10000);
 try{await p.goto(base+'/scripts/fixtures/release-notice.html?'+(options.query??''));await p.waitForTimeout(250);await run(p);results.push({name,passed:true});console.log('PASS '+name);}
 catch(e){results.push({name,passed:false,error:String(e)});process.exitCode=1;await p.screenshot({path:out+'/failed-'+results.length+'.png'});console.error('FAIL '+name,e);}
 finally{await context.close();}
}
const absent=async p=>assert.equal(await p.locator('dialog[open]').count(),0);
try{
 await check('Fresh installation does not announce an upgrade',{},async p=>{await absent(p);await p.reload();await p.waitForTimeout(200);await absent(p);});
 await check('Legacy upgrade: X dismisses once and survives reload',{query:'existing=1'},async p=>{
  await p.getByRole('dialog').waitFor();assert.equal(await p.locator('.release-version').innerText(),'v'+version);await p.screenshot({path:out+'/whats-new-900x640.png'});
  await p.getByRole('button',{name:'Đóng thông báo cập nhật',exact:true}).click();await absent(p);await p.reload();await p.waitForTimeout(200);await absent(p);
 });
 await check('Upgrade contains keyboard focus and Escape remembers dismissal',{viewport:{width:1180,height:800},state:{version:'0.8.4',pending:null}},async p=>{
  const dialog=p.getByRole('dialog');await dialog.waitFor();await p.screenshot({path:out+'/whats-new-1180x800.png'});
  for(const keyPress of ['Tab','Shift+Tab'])for(let i=0;i<6;i++){await p.keyboard.press(keyPress);assert(await dialog.evaluate(e=>e.contains(document.activeElement)));}
  await p.keyboard.press('Escape');await absent(p);await p.reload();await p.waitForTimeout(200);await absent(p);
 });
 await check('Busy mode defers without consuming pending notes',{query:'ready=0',state:{version:'0.8.4',pending:null}},async p=>{
  await absent(p);assert.equal(await p.evaluate(key=>JSON.parse(localStorage.getItem(key)).pending,key),version);
  await p.goto(base+'/scripts/fixtures/release-notice.html?ready=1');await p.getByRole('dialog').waitFor();
  await p.getByRole('button',{name:'Đã hiểu',exact:true}).click();await absent(p);await p.reload();await p.waitForTimeout(200);await absent(p);
 });
 await check('Downgrade does not show old release notes',{state:{version:'99.0.0',pending:null}},absent);
 await check('Upgrade from 0.8.9 shows microphone notes and remembers dismissal',{state:{version:'0.8.9',pending:null}},async p=>{
  const dialog=p.getByRole('dialog');await dialog.waitFor();const text=await dialog.innerText();assert(text.includes('Tắt micro'));assert(text.includes('v'+version));
  await p.getByRole('button',{name:'Đóng thông báo cập nhật',exact:true}).click();await absent(p);await p.reload();await p.waitForTimeout(200);await absent(p);
 });
 await check('Upgrade from 0.8.8 shows the current release notes',{state:{version:'0.8.8',pending:null}},async p=>{
  const dialog=p.getByRole('dialog');await dialog.waitFor();assert((await dialog.innerText()).includes('Tắt micro'));
  await p.getByRole('button',{name:'Đóng thông báo cập nhật',exact:true}).click();await absent(p);
 });
 await check('Development preview does not change real upgrade history',{query:'version=0.8.4&existing=1'},async p=>{
  await absent(p);const before=await p.evaluate(key=>localStorage.getItem(key),key);
  await p.evaluate(()=>window.dispatchEvent(new Event('innote:preview-whats-new')));await p.getByRole('dialog').waitFor();
  await p.getByRole('button',{name:'Đã hiểu',exact:true}).click();await absent(p);
  assert.equal(await p.evaluate(key=>localStorage.getItem(key),key),before);
 });
 for(const locale of ['en','ko','zh'])await check('Localized dialog fits '+locale,{query:'existing=1&locale='+locale},async p=>{
  const dialog=p.getByRole('dialog');await dialog.waitFor();const box=await dialog.boundingBox();assert(box.x>=0&&box.y>=0&&box.x+box.width<=901&&box.y+box.height<=641);
  assert(await dialog.locator('footer button').isVisible());await p.screenshot({path:out+'/whats-new-'+locale+'.png'});
 });
}finally{await browser.close();writeFileSync(out+'/results.json',JSON.stringify(results,null,2));}
