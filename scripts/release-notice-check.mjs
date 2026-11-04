import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import ts from 'typescript';
const code=ts.transpileModule(readFileSync('src/lib/release-notice.ts','utf8'),{compilerOptions:{module:ts.ModuleKind.ESNext,target:ts.ScriptTarget.ES2022}}).outputText;
const {prepareReleaseNotice:prepare,dismissReleaseNotice:dismiss,compareVersions,RELEASE_NOTICE_KEY:key}=await import('data:text/javascript;base64,'+Buffer.from(code).toString('base64'));
const storage=()=>{const values=new Map();return {getItem:k=>values.get(k)??null,setItem:(k,v)=>values.set(k,v)};};
const fresh=storage();assert(!prepare(fresh,'0.8.5',false,true));assert(!prepare(fresh,'0.8.5',true,true));
const upgrade=storage();assert(!prepare(upgrade,'0.8.4',false,false));assert(prepare(upgrade,'0.8.5',true,true));assert(prepare(upgrade,'0.8.5',true,true),'Keep pending until acknowledged');
dismiss(upgrade,'0.8.5');assert(!prepare(upgrade,'0.8.5',true,true));assert(prepare(upgrade,'0.8.6',true,true));
assert(!prepare(upgrade,'0.8.5',true,true),'Downgrade must not notify');assert(prepare(upgrade,'0.8.6',true,true),'Downgrade must not consume pending update');
dismiss(upgrade,'0.8.6');assert(!prepare(upgrade,'0.8.6',true,true));
const legacy=storage();assert(prepare(legacy,'0.8.5',true,true));dismiss(legacy,'0.8.5');assert(!prepare(legacy,'0.8.5',true,true));
const corrupt=storage();corrupt.setItem(key,'{');assert(prepare(corrupt,'0.8.5',true,true));
assert(!prepare(storage(),'invalid',true,true));assert(!prepare(storage(),'0.8.5',true,false));
const broken={getItem(){throw Error('blocked')},setItem(){throw Error('blocked')}};assert(!prepare(broken,'0.8.5',true,true));
for(const [a,b] of [['0.8.5','0.8.4'],['0.10.0','0.9.9'],['1.0.0','1.0.0-rc.1'],['1.0.0-rc.10','1.0.0-rc.2'],['1.0.0-beta','1.0.0-alpha']])assert(compareVersions(a,b)>0);
assert.equal(compareVersions('0.8.5+one','0.8.5+two'),0);
console.log('PASS release notice lifecycle: first install, legacy upgrade, pending restart, dismissal, future version, rollback, corruption, unavailable storage and version ordering');

// The next shipped release has its own notes and acknowledgement.
const current=JSON.parse(readFileSync('package.json','utf8')).version;
const upgraded=storage();prepare(upgraded,'0.8.6',false,true);assert(prepare(upgraded,current,true,true));dismiss(upgraded,current);assert(!prepare(upgraded,current,true,true));
const notesJS=ts.transpileModule(readFileSync('src/lib/release-notes.ts','utf8'),{compilerOptions:{module:ts.ModuleKind.ESNext,target:ts.ScriptTarget.ES2022}}).outputText;
const {releaseNotes,previewReleaseVersion}=await import('data:text/javascript;base64,'+Buffer.from(notesJS).toString('base64'));
assert.equal(previewReleaseVersion,current);assert.notEqual(releaseNotes[current],releaseNotes['0.8.6']);
for(const locale of ['vi','en','ko','zh']){const note=releaseNotes[current][locale];assert(note.title&&note.subtitle&&note.close&&note.done&&note.once);assert.equal(note.items.length,4);assert(note.items.every(i=>i.title&&i.detail));}
console.log('PASS current release: distinct localized notes, preview version and upgrade from 0.8.6 dismissed once');
