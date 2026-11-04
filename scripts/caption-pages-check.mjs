import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import ts from 'typescript';
const js=ts.transpileModule(readFileSync('src/lib/caption-pages.ts','utf8'),{compilerOptions:{module:ts.ModuleKind.ESNext,target:ts.ScriptTarget.ES2022}}).outputText;
const {CaptionPager,fitCaptionPage}=await import('data:text/javascript;base64,'+Buffer.from(js).toString('base64'));
for(const text of ['Một câu tiếng Việt dài cần chia thành các phần ngắn để đọc được.', '你好世界。这是一段很长的中文字幕。','A_long_word_without_spaces_123456789','🎧😀 Một câu có biểu tượng.']) {
  let start=0,result='';
  while(start<text.length){const page=fitCaptionPage(text,start,s=>Array.from(s).length<=12);assert(page.end>start);assert(Array.from(page.text).length<=12);result+=text.slice(start,page.end);start=page.end;}
  assert.equal(result,text,'All remaining text is retained for following pages');
}
const fit=(text,start)=>fitCaptionPage(text,start,s=>s.length<=12);
const p=new CaptionPager();p.set('one two three four five six','',0);
const first=p.layout(fit,fit,0);assert.equal(first.text,'one two');assert(first.delay>0);
p.set('one two three four five six seven','',100);assert.equal(p.layout(fit,fit,100).text,first.text);
p.advance(2000);assert.notEqual(p.layout(fit,fit,2000).text,first.text);
p.set('new sentence','',2100);assert.equal(p.layout(fit,fit,2100).text,'new sentence');
const pair=new CaptionPager();pair.set('one two three four five','một hai ba bốn năm sáu',0);const a=pair.layout(fit,fit,0);pair.advance(2000);const b=pair.layout(fit,fit,2000);assert.notEqual(a.text,b.text);assert.notEqual(a.original,b.original);
console.log('PASS caption pages: bounded text, word boundaries, CJK, emoji, full continuation, streaming updates, new cue reset, synchronized bilingual advance');
