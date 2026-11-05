import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import ts from 'typescript';
const asURL=source=>'data:text/javascript;base64,'+Buffer.from(ts.transpileModule(source,{compilerOptions:{module:ts.ModuleKind.ESNext,target:ts.ScriptTarget.ES2022}}).outputText).toString('base64');
const styleURL=asURL(readFileSync('src/lib/caption-style.ts','utf8'));
const presetsURL=asURL(readFileSync('src/lib/workflow-presets.ts','utf8').replace('"./caption-style"',JSON.stringify(styleURL)));
const {writePresetPreferences}=await import(presetsURL);
const {SONIOX_RT_MODEL,sonioxV6Preferences}=await import(asURL(readFileSync('src/lib/soniox-model.ts','utf8')));
assert.equal(SONIOX_RT_MODEL,'stt-rt-v6');
const oldChoice={profileId:'soniox',model:'stt-rt-v5',fallback:{profileId:'backup',model:'stt-rt-v4'}};
const oldQuality=JSON.stringify(['meeting','soniox','stt-rt-v5','en',true]),newQuality=JSON.stringify(['meeting','soniox','stt-rt-v6','en',true]);
const map=new Map(Object.entries({
 'may.preferences':JSON.stringify({speechSelection:oldChoice,microphone:'',language:'en',meetingTranslate:true}),
 'may.caption.standalone':JSON.stringify({...oldChoice,translate:false}),
 'may.task_assign.meeting':JSON.stringify({transcription:oldChoice}),
 'may.task_assign.captionsonly':JSON.stringify({caption:oldChoice}),
 'innote.workflow-presets.v1':JSON.stringify({version:1,presets:[{id:'saved',name:'Keep title stt-rt-v5',tasks:{caption:oldChoice},captionStyle:{fontSize:36}}]}),
 'may.speech.processing-levels':JSON.stringify({[oldQuality]:'accurate',[newQuality]:'fast'}),
 'ai-profiles.json':'private-store-must-not-be-read',
 'unrelated':'stt-rt-v5',
}));
const source={getItem:key=>{assert(!key.includes('ai-profiles'));return map.get(key)??null}};
const changes=sonioxV6Preferences(source);assert.equal(Object.keys(changes).length,6);
const presets=JSON.parse(changes['innote.workflow-presets.v1']);assert.equal(presets.presets[0].name,'Keep title stt-rt-v5');assert.equal(presets.presets[0].tasks.caption.model,'stt-rt-v6');assert.equal(presets.presets[0].tasks.caption.fallback.model,'stt-rt-v6');assert.equal(presets.presets[0].captionStyle.fontSize,36);
assert.equal(JSON.parse(changes['may.speech.processing-levels'])[newQuality],'fast');assert(!changes['may.speech.processing-levels'].includes('stt-rt-v5'));
let fail=true;const original=[...map];const store={...source,setItem:(key,value)=>{if(key==='may.task_assign.meeting'&&fail){fail=false;throw Error('disk full')}map.set(key,value)},removeItem:key=>map.delete(key)};
assert.throws(()=>writePresetPreferences(store,changes));assert.deepEqual([...map],original,'Failed upgrade rolls all launch entries back');
writePresetPreferences(store,changes);assert.deepEqual(sonioxV6Preferences(source),{});assert.equal(map.get('unrelated'),'stt-rt-v5');
for(const raw of ['bad','null','[]','{"model":3}'])assert.deepEqual(sonioxV6Preferences({getItem:()=>raw}),{});
console.log('PASS Soniox v6: launch/preset/fallback migration, quality collision, idempotence, atomic rollback, styles/labels and API stores preserved');
