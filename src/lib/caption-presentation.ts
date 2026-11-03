export interface CaptionCue {
  text: string;
  original: string;
  final_: boolean;
  translating?: boolean;
}
const empty = (): CaptionCue => ({text:"",original:"",final_:true});
/** A bilingual cue is indivisible. Never combine a new source with an old target. */
export class CaptionPresentation {
  private translating = false;
  private latest = empty();
  private translated = empty();
  private pair: CaptionCue | null = null;
  isTranslating() { return this.translating; }
  clear() { this.translating=false;this.latest=empty();this.translated=empty();this.pair=null; }
  setMode(translating:boolean) { if(this.translating!==translating){this.clear();this.translating=translating;} }
  push(cue:CaptionCue) {
    if(cue.translating!==undefined)this.setMode(cue.translating);
    else if(cue.original.trim())this.setMode(true);
    this.latest={...cue};
    if(this.translating && cue.text.trim())this.translated={...cue};
    if(this.translating && cue.final_ && cue.text.trim() && cue.original.trim())this.pair={...cue};
  }
  restore(snapshot:{caption:CaptionCue;translating?:boolean;paired?:CaptionCue|null}) {
    this.clear();
    const translating=snapshot.translating??!!snapshot.caption.original.trim();
    this.setMode(translating);
    if(snapshot.paired)this.push({...snapshot.paired,translating});
    this.push({...snapshot.caption,translating});
  }
  view(bilingual:boolean):CaptionCue {
    if(!this.translating)return {...this.latest,original:""};
    return {...(bilingual ? this.pair??empty() : this.translated)};
  }
}
