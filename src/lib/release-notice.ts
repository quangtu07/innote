export const RELEASE_NOTICE_KEY = "innote.release-notice.v1";
type Store = Pick<Storage, "getItem" | "setItem">;
interface NoticeState { version: string; pending: string | null }

function parts(version: string) {
  const match=/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-([\da-zA-Z.-]+))?(?:\+[\da-zA-Z.-]+)?$/.exec(version);
  return match ? { core:match.slice(1,4).map(Number), pre:match[4]?.split(".") } : null;
}
export function compareVersions(a: string, b: string): number {
  const left=parts(a),right=parts(b);
  if(!left||!right)throw new Error("Invalid release version");
  for(let i=0;i<3;i++)if(left.core[i]!==right.core[i])return Math.sign(left.core[i]-right.core[i]);
  if(!left.pre||!right.pre)return left.pre ? -1 : right.pre ? 1 : 0;
  for(let i=0;i<Math.max(left.pre.length,right.pre.length);i++){
    const x=left.pre[i],y=right.pre[i];if(x===y)continue;
    if(x===undefined)return -1;if(y===undefined)return 1;
    const xn=/^\d+$/.test(x),yn=/^\d+$/.test(y);
    if(xn&&yn)return Math.sign(Number(x)-Number(y));
    if(xn!==yn)return xn ? -1 : 1;
    return x<y ? -1 : 1;
  }
  return 0;
}
function read(store: Store): NoticeState | null {
  try {
    const value=JSON.parse(store.getItem(RELEASE_NOTICE_KEY)??"null");
    return value && typeof value.version==="string" && parts(value.version)
      && (value.pending===null || value.pending===value.version) ? value : null;
  } catch { return null; }
}
/** First install establishes a baseline. Legacy installs are upgraded clients. */
export function prepareReleaseNotice(store: Store, version: string, existingInstall: boolean, hasNotes: boolean): boolean {
  if(!parts(version))return false;
  const previous=read(store);
  if(previous && compareVersions(version,previous.version)<=0)return previous.version===version && previous.pending===version && hasNotes;
  const pending=hasNotes && (!!previous || existingInstall) ? version : null;
  try { store.setItem(RELEASE_NOTICE_KEY,JSON.stringify({version,pending})); }
  catch { return false; } // Do not repeatedly interrupt when storage is unavailable.
  return pending!==null;
}
export function dismissReleaseNotice(store: Store, version: string) {
  const state=read(store);
  if(state?.pending===version)store.setItem(RELEASE_NOTICE_KEY,JSON.stringify({...state,pending:null}));
}
