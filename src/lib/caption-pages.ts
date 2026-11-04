/** Fit whole words where possible; fall back to code points for CJK and long words. */
export function fitCaptionPage(text: string, start: number, fits: (text: string) => boolean) {
  const rest=text.slice(start);
  const chars=Array.from(rest);
  let lo=0,hi=chars.length;
  while(lo<hi){const mid=Math.ceil((lo+hi)/2);if(fits(chars.slice(0,mid).join("")))lo=mid;else hi=mid-1;}
  if(lo===chars.length)return {text:rest.trim(),end:text.length};
  let part=chars.slice(0,lo).join("");
  // Do not split a word when a usable boundary exists on this page.
  const boundaries=[...part.matchAll(/[\s,.;!?。！？、，；：]+/gu)];
  const boundary=boundaries.at(-1);
  if(boundary && boundary.index!+boundary[0].length>part.length/2)part=part.slice(0,boundary.index!+boundary[0].length);
  return {text:part.trim(),end:start+part.length};
}

export class CaptionPager {
  private text="";
  private original="";
  private start=0;
  private originalStart=0;
  private end=0;
  private originalEnd=0;
  private shownAt=0;
  set(text:string,original:string,now=Date.now()) {
    if(text===this.text && original===this.original)return;
    // Streaming revisions retain the current page while the same utterance grows.
    const same=(old:string,next:string,at:number)=>!old || old.startsWith(next) || next.startsWith(old) || (at>0&&old.slice(0,at)===next.slice(0,at));
    if(!same(this.text,text,this.start)||!same(this.original,original,this.originalStart)) {
      this.start=0;this.originalStart=0;this.shownAt=now;
    }
    if(!this.text&&!this.original)this.shownAt=now;
    this.text=text;this.original=original;
    this.start=Math.min(this.start,text.length);this.originalStart=Math.min(this.originalStart,original.length);
  }
  layout(fitText:(text:string,start:number)=>{text:string;end:number},fitOriginal:typeof fitText,now=Date.now()) {
    const text=fitText(this.text,this.start),original=fitOriginal(this.original,this.originalStart);
    this.end=text.end;this.originalEnd=original.end;
    const more=this.end<this.text.length || this.originalEnd<this.original.length;
    const hold=Math.min(2800,Math.max(1100,Math.max(text.text.length,original.text.length)*45));
    return {text:text.text,original:original.text,delay:more?Math.max(50,this.shownAt+hold-now):null};
  }
  advance(now=Date.now()) {
    if(this.end<this.text.length)this.start=this.end;
    if(this.originalEnd<this.original.length)this.originalStart=this.originalEnd;
    this.shownAt=now;
  }
}
