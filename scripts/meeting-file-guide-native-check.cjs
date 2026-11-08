// Read/write only the isolated storage fixture produced by the storage acceptance tests.
const assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path');
const{chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
(async()=>{
 const browser=await chromium.connectOverCDP('http://127.0.0.1:9224');
 const page=browser.contexts().flatMap(c=>c.pages()).find(p=>p.url().includes('tauri.localhost'));assert(page);
 const api=(cmd,args={})=>page.evaluate(({cmd,args})=>window.__TAURI_INTERNALS__.invoke(cmd,args),{cmd,args});
 const before=await api('snapshot');assert(before.data_dir.includes('innote-storage-native-20261107')&&!before.active_id);
 const samples=[before.sessions.find(s=>s.corrected),before.sessions.find(s=>s.translations.length&&s.summary)];assert(samples.every(Boolean),'Existing synthetic correction/translation fixtures required');
 const records=[];
 for(const s of samples){
   const info=await api('meeting_files_status',{id:s.id});assert(!info.legacy);
   const guide=path.join(info.path,'00 - ĐỌC TÔI TRƯỚC.txt');assert(fs.existsSync(guide),'Startup adds help to existing modern folders');
   await api('retry_meeting_bundle',{id:s.id});
   const text=fs.readFileSync(guide,'utf8');assert(!text.includes('\uFFFD'));
   for(const word of ['BẢN CHÉP GỐC','BẢN HIỆU CHỈNH','BẢN GHI ÂM','meeting.json','.innote/work/'])assert(text.includes(word));
   for(const [name,label] of [['recording.wav','recording.wav'],['corrected.md','corrected.md'],['summary.md','summary.md']])assert(text.includes(`[${fs.existsSync(path.join(info.path,name))?'ĐÃ CÓ':'CHƯA CÓ'}] ${label}`));
   assert(fs.existsSync(path.join(path.dirname(info.path),'00 - HƯỚNG DẪN THƯ VIỆN INNOTE.txt')));
   assert(fs.readFileSync(path.join(info.path,'original.md'),'utf8').startsWith('> BẢN CHÉP GỐC'));
   if(s.corrected)assert(fs.readFileSync(path.join(info.path,'corrected.md'),'utf8').startsWith('> BẢN HIỆU CHỈNH'));
   const after=(await api('snapshot')).sessions.find(x=>x.id===s.id);assert.deepEqual(after,s,'Documentation cannot change the stored meeting');
   records.push({id:s.id,guide,corrected:!!s.corrected,translation:s.translations.length>0,summary:!!s.summary});
 }
 fs.writeFileSync('test-results/meeting-storage-native/file-guide-results.json',JSON.stringify({origin:page.url(),records},null,2));
 console.log('PASS production UTF-8 guides, existing-folder installation, output labels, actual-file statuses and unchanged meeting data');await browser.close();
})().catch(e=>{console.error(e);process.exit(1)});
