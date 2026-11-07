<script lang="ts">
  import {onMount} from 'svelte';
  import {invoke} from '@tauri-apps/api/core';
  import {FolderOpen,FolderInput,RefreshCw} from '@lucide/svelte';
  export let mode:'settings'|'meeting'|'hint'='settings';
  export let id:string|null=null;
  export let desktop=true;
  export let openSettings:()=>void=()=>{};
  let info:any=null, files:any=null, candidate='',busy=false,error='',loadError='',message='';
  let loadedId:string|null=null;
  async function refresh(){
    if(!desktop)return;
    try {
      if(mode==='meeting'&&id){const requested=id;const value=await invoke('meeting_files_status',{id:requested});if(id===requested){files=value;loadedId=id;}}
      else info=await invoke('meeting_storage_status');
      loadError='';
    }catch(e){loadError=String(e);}
  }
  $: if(mode==='meeting'&&id!==loadedId){loadedId=id;files=null;void refresh();}
  onMount(()=>{void refresh();const timer=setInterval(()=>void refresh(),4000);return()=>clearInterval(timer);});
  async function run(fn:()=>Promise<void>){busy=true;error='';message='';try{await fn();await refresh();}catch(e){error=String(e).replace(/^Error:\s*/,'');}finally{busy=false;}}
  function bytes(n:number){return new Intl.NumberFormat('vi',{maximumFractionDigits:1}).format(n/1024**3)+' GB';}
  function displayPath(p:string){return p?.replace(/^\\\\\?\\/,'')??'';}
</script>

{#if mode==='settings'}
<section class="settings-section meeting-storage" aria-labelledby="storage-heading">
  <h2 id="storage-heading">Lưu trữ cuộc họp</h2>
  <p>Bản ghi âm, bản chép gốc, bản dịch, bản hiệu chỉnh và tóm tắt được lưu cùng cuộc họp. Cấu hình và API key nằm riêng trong Innote.</p>
  <div class="storage-destination">
    <div class="storage-path" title={displayPath(info?.path)}>{info?.path?displayPath(info.path):'Chưa chọn nơi lưu cuộc họp mới'}</div>
    {#if info?.available_bytes!=null}<span class="storage-meta">Còn trống {bytes(info.available_bytes)}</span>{/if}
  </div>
  <div class="storage-actions">
    <button class="button secondary" disabled={!desktop||busy} on:click={()=>run(async()=>{const path=await invoke<string|null>('choose_meetings_directory');if(path)candidate=path;})}><FolderInput size={17}/>Chọn thư mục</button>
    <button class="button secondary" disabled={!desktop||busy||!info?.ready} on:click={()=>run(async()=>{await invoke('open_meetings_directory');})}><FolderOpen size={17}/>Mở thư mục</button>
  </div>
  {#if candidate}
  <div class="storage-confirm">
    <label>Thư mục đã chọn<input readonly value={candidate} aria-label="Thư mục cuộc họp đã chọn"/></label>
    <p>Ví dụ: 2026-11-07_09-30-00 - Họp kế hoạch</p>
    <div class="storage-actions"><button class="button primary" disabled={busy} on:click={()=>run(async()=>{await invoke('set_meetings_directory',{path:candidate});candidate='';message='Đã đổi nơi lưu cho cuộc họp mới';})}>Dùng cho cuộc họp mới</button><button class="text-button" disabled={busy} on:click={()=>candidate=''}>Hủy</button></div>
  </div>
  {/if}
  <p class="storage-meta">Chỉ áp dụng cho cuộc họp tạo từ bây giờ. Các cuộc họp đã có không bị chuyển.</p>
  <details><summary>Cấu hình và dữ liệu cũ</summary><p>Cấu hình/API key vẫn ở thư mục riêng: <span class="storage-path">{info?.private_path??''}</span>. Dữ liệu cuộc họp cũ vẫn chiếm dung lượng tại nơi cũ. Model local và bộ cài không chuyển theo cài đặt này.</p><button class="text-button" disabled={busy} on:click={()=>run(async()=>{const count=await invoke<number>('rescan_meeting_libraries');message=`Đã quét thư viện, cập nhật ${count} đường dẫn cuộc họp`;})}><RefreshCw size={16}/>Quét lại thư viện</button></details>
  {#each info?.issues??[] as issue}<p class="test-error" role="status">{issue}</p>{/each}
</section>
{:else if mode==='hint'}
<div class="storage-hint"><span>Lưu tại: <span title={displayPath(info?.path)}>{info?.path?displayPath(info.path):'Chưa chọn thư mục'}</span></span><button class="text-button" on:click={openSettings}>Đổi trong Cài đặt</button></div>
{#each info?.issues??[] as issue}<p class="test-error" role="status">{issue}</p>{/each}
{:else if files}
<div class="storage-hint meeting-files" aria-label="Tệp cuộc họp">
  <div><span class="storage-path" title={displayPath(files.path)}>{displayPath(files.path)}</span><small>{files.legacy?'Nơi lưu cũ · dữ liệu giữ nguyên':files.error??(files.rename_pending?'Tên thư mục sẽ cập nhật khi xử lý xong':files.revision===files.published_revision?'Tệp cuộc họp đã cập nhật':'Đã lưu trong Innote · đang cập nhật tệp')}</small></div>
  <button class="text-button" disabled={busy} on:click={()=>run(async()=>{await invoke('open_meeting_directory',{id});})}><FolderOpen size={16}/>Mở thư mục</button>
  {#if !files.legacy&&files.error}<button class="text-button" disabled={busy} on:click={()=>run(async()=>{await invoke('retry_meeting_bundle',{id});})}><RefreshCw size={16}/>Thử lại</button>{/if}
</div>
{/if}
{#if error||loadError}<p class="test-error" role="alert">{error||loadError}</p>{/if}
{#if message}<p class="storage-success" role="status">{message}</p>{/if}

<style>
  .meeting-storage p{max-width:76ch}.storage-destination{display:flex;align-items:baseline;gap:16px;justify-content:space-between;background:var(--bg-soft,#edf3ed);padding:14px 16px;border-radius:12px;margin:18px 0}.storage-path{overflow-wrap:anywhere;min-width:0}.storage-meta,small{color:var(--muted,#536b61);font-size:.9rem}.storage-actions{display:flex;gap:12px;flex-wrap:wrap;align-items:center}.storage-confirm{margin:20px 0;padding:18px;border:1px solid var(--border,#ccdcd2);border-radius:12px}.storage-confirm label{display:grid;gap:8px}.storage-confirm input{width:100%;box-sizing:border-box}.storage-hint{display:flex;gap:12px;align-items:center;justify-content:space-between;flex-wrap:wrap;font-size:.9rem;margin:12px 0 20px}.storage-hint>span,.storage-hint>div{min-width:0;flex:1}.storage-hint small{display:block;margin-top:4px}.storage-hint button{display:inline-flex;align-items:center;gap:6px;white-space:nowrap}.storage-success{color:#146c55}summary{cursor:pointer}.storage-meta{margin-top:14px}@media(max-width:700px){.storage-destination{align-items:flex-start;flex-direction:column;gap:6px}.storage-hint{align-items:flex-start}}
</style>
