const assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path');const{chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
(async()=>{
 const browser=await chromium.connectOverCDP('http://127.0.0.1:9223');const page=browser.contexts().flatMap(c=>c.pages()).find(p=>p.url()==='http://127.0.0.1:1420/');
 const api=(cmd,args={})=>page.evaluate(({cmd,args})=>window.__TAURI_INTERNALS__.invoke(cmd,args),{cmd,args});
 const first=await api('snapshot');assert(first.data_dir.includes('innote-storage-native-20261107')&&!first.active_id);
 const registryFile=path.join(first.data_dir,'meeting-storage.json'),before=fs.readFileSync(registryFile);const registry=JSON.parse(before);
 const missing=path.join(path.dirname(first.data_dir),'unplugged-library-test');assert(!fs.existsSync(missing));
 const speaker=first.devices.find(d=>d.kind==='system'&&d.default)||first.devices.find(d=>d.kind==='system');
 const options={title:'QA chỉ kiểm tra xóa',system:speaker.id,microphone:null,language:'en',model:'moonshine-en',transcribe:false,provider:'local',captions_enabled:false,translation_language:null};
 try {
   registry.libraries.find(l=>l.id===registry.default_library_id).root=missing;fs.writeFileSync(registryFile,JSON.stringify(registry));
   const status=await api('meeting_storage_status');assert.equal(status.ready,false);assert(status.issues.length);
   await assert.rejects(api('start',{options}));assert(!fs.existsSync(missing));
   assert((await api('snapshot')).sessions.length>0,'Other libraries remain visible');
 }finally{fs.writeFileSync(registryFile,before);}
 const second=JSON.parse(fs.readFileSync('test-results/meeting-storage-native/native-results.json','utf8')).second;
 const info=await api('meeting_files_status',{id:second.id});const moved=info.path+' đổi ngoài app';fs.renameSync(info.path,moved);
 assert(await api('rescan_meeting_libraries')>=1);assert.equal((await api('meeting_files_status',{id:second.id})).path,moved);
 await api('edit_session',{id:second.id,title:'QA thư mục đã liên kết lại',notes:null,segmentId:null,text:null});
 assert(fs.existsSync((await api('meeting_files_status',{id:second.id})).path));
 const fresh=await api('start',{options});await page.waitForTimeout(500);await api('stop');
 const until=Date.now()+30000;while((await api('snapshot')).active_id&&Date.now()<until)await page.waitForTimeout(100);
 const folder=(await api('meeting_files_status',{id:fresh.id})).path;await api('delete_session',{id:fresh.id});
 assert(!fs.existsSync(folder));assert(!(await api('snapshot')).sessions.some(s=>s.id===fresh.id));
 fs.writeFileSync('test-results/meeting-storage-native/offline-results.json',JSON.stringify({missingRootRejected:true,noFallback:true,otherLibraryReadable:true,externalRenameRescanned:true,deleteOwnFixture:true},null,2));
 console.log('PASS offline selected root, no fallback, other library access, Explorer rename/rescan and own-fixture deletion');await browser.close();
})().catch(e=>{console.error(e);process.exit(1)});
