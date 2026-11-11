// Frontend contract/UX tests use the official Tauri mock API; no audio is recorded.
import assert from "node:assert/strict";
import { readFileSync, mkdirSync, writeFileSync } from "node:fs";
import { resolve, join } from "node:path";
import { pathToFileURL } from "node:url";
const modulePath = process.env.PLAYWRIGHT_MODULE;
const { chromium } = await import(
  modulePath ? pathToFileURL(modulePath).href : "playwright"
);
const output = resolve(process.env.UI_TEST_OUTPUT ?? "test-results/ui-check");
mkdirSync(output, { recursive: true });
const mocks = readFileSync(
  "node_modules/@tauri-apps/api/mocks.js",
  "utf8",
).replace(/export \{[^}]+\};/g, "");
const browser = await chromium.launch({
  channel: process.env.PLAYWRIGHT_CHANNEL ?? "chrome",
  headless: true,
});
const results = [];
const ids = [
  "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
  "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
  "cccccccc-cccc-4ccc-8ccc-cccccccccccc",
];
const mockProfiles = [
  { id: "prof-soniox", name: "Soniox công việc", provider: "soniox", base_url: "", has_key: true, key_tail: "abcd", created: "2026-10-30T00:00:00Z" },
  { id: "prof-openai", name: "OpenAI chính", provider: "openai", base_url: "", has_key: true, key_tail: "wxyz", created: "2026-10-30T00:00:00Z" },
  { id: "prof-gemini", name: "Gemini cá nhân", provider: "gemini", base_url: "", has_key: true, key_tail: "efgh", created: "2026-10-30T00:00:00Z" },
];
async function setup(options = {}) {
  const context = await browser.newContext({
    viewport: options.viewport ?? { width: 1180, height: 800 },
  });
  const init = ({ ids, options }) => {
    const segment = {
      id: "segment-one",
      start: 1,
      end: 4,
      text: "Nội dung thử tiếng Việt.",
      edited: false,
    };
    const make = (id, title, notes) => ({
      id,
      title,
      notes,
      created: "2026-10-25T12:00:00+07:00",
      duration: 10,
      processed: 10,
      status: "completed",
      language: options.recordingLanguage ?? "vi",
      model: "base-q5_1",
      segments: [{ ...segment }],
      warnings: [],
      error: null,
    });
    window.qa = {
      profiles: [],
      calls: [],
      snapshot: {
        sessions: [
          make(ids[0], "Cuộc họp Alpha", "Ghi chú Alpha"),
          make(ids[1], "Cuộc họp Beta", "Ghi chú Beta"),
        ],
        devices: [
          { id: "speaker", name: "Loa thử", kind: "system", default: true },
          { id: "mic", name: "Micro thử", kind: "microphone", default: true },
        ],
        models: [
          {
            id: "base-q5_1",
            name: "Tiết kiệm tài nguyên",
            bytes: 59707625,
            installed: !options.missingModels,
          },
          {
            id: "small-q5_1",
            name: "Cân bằng",
            bytes: 190085487,
            installed: !options.missingModels,
          },
          {
            id: "moonshine-vi",
            name: "Moonshine · Tiếng Việt · Có sẵn",
            bytes: 32309045,
            installed: !options.missingBundled,
          },
        ],
        active_id: null,
        has_microphone: false,
        mic_muted: false,
        data_dir: "C:\\QA\\Innote",
      },
      exports: [],
      delayAudio: !!options.delayAudio,
    };
    if(options.captionStatus)window.qa.caption={running:!["idle","error"].includes(options.captionStatus.state)};
    window.qa.snapshot.models.push({id:"moonshine-en",name:"Moonshine · English · Có sẵn",bytes:32309045,installed:!options.missingBundled});
    window.qa.snapshot.models.push({id:"nemotron-3.5-q8",name:"Nemotron · Chép lời local đa ngôn ngữ",bytes:741548352,installed:!options.missingModels&&!options.missingMultilingual});
    if(options.manyMeetings){
      const template=window.qa.snapshot.sessions[0];
      window.qa.snapshot.sessions=Array.from({length:60},(_,i)=>({...structuredClone(template),id:crypto.randomUUID(),title:`Cuộc họp ${i}`}));
    }
    if(options.bilingual){
      window.qa.snapshot.sessions[0].speech={model:'stt-rt-v6',profile_id:'prof-soniox',translation_language:'en',connections:1,uploaded_samples:160000,local_fallback:false};
      window.qa.snapshot.sessions[0].translations=[{id:'translated-one',source_id:'segment-one',start:1,end:4,original:segment.text,text:'English translated reference.',language:'en',provider:'soniox'}];
    }
    if (options.active) {
      window.qa.snapshot.active_id = ids[2];
      window.qa.snapshot.has_microphone = true;
      window.qa.snapshot.mic_muted = false;
      window.qa.snapshot.sessions.push({
        ...make(ids[2], "Phiên đang ghi", ""),
        status: "recording",
        processed: 0,
        duration: 40,
      });
    }
    if (options.fallback) {
      window.qa.snapshot.sessions[0].provider = "mixed";
      window.qa.snapshot.sessions[0].warnings = [
        "Soniox: đã hết số dư. Innote đã chuyển sang local miễn phí đến hết phiên; bản thu và phần đã chép vẫn được giữ.",
      ];
    }
    if (options.interrupted) {
      window.qa.snapshot.sessions[0].status = "interrupted";
      window.qa.snapshot.sessions[0].processed = 4;
    }
    if(options.corrected)window.qa.snapshot.sessions[0].corrected=options.corrected;
    if(options.standalone)localStorage.setItem("may.caption.standalone",JSON.stringify(options.standalone));
    if (!localStorage.getItem("may.preferences") && !options.naturalMeetingDefaults) {
      const defaults={system:window.qa.snapshot.devices.find(d=>d.kind==='system'&&d.default)?.id??"",microphone:window.qa.snapshot.devices.find(d=>d.kind==='microphone'&&d.default)?.id??"",language:"vi",transcribe:true,meetingCaptions:false,meetingTranslate:false};
      localStorage.setItem("may.preferences",JSON.stringify({...defaults,...options.preferences}));
    } else if(!localStorage.getItem("may.preferences") && options.preferences)localStorage.setItem("may.preferences",JSON.stringify(options.preferences));
    if (options.taskAssignMeeting)
      localStorage.setItem(
        "may.task_assign.meeting",
        JSON.stringify(options.taskAssignMeeting),
      );
    if (options.profiles) window.qa.profiles = options.profiles;
    if (!options.noUiLocale) localStorage.setItem("may.ui_locale", "vi");
    mockConvertFileSrc("windows");
    mockIPC(
      async (cmd, args) => {
        const qa = window.qa;
        qa.calls.push({ cmd, args });
        if (cmd === "snapshot") return structuredClone(qa.snapshot);
        if(cmd==='meeting_storage_status')return {path:localStorage.getItem('qa.storagePath')||'C:\\Meetings',ready:true,available_bytes:100*1024**3,private_path:'C:\\Private Innote',issues:options.storageOffline?['Ổ lưu chưa kết nối: D:\\Meetings']:[]};
        if(cmd==='choose_meetings_directory')return options.cancelStoragePicker?null:'D:\\Meetings';
        if(cmd==='set_meetings_directory'){if(options.failStorage||qa.snapshot.active_id)throw Error('Hãy kết thúc phiên trước khi đổi nơi lưu');localStorage.setItem('qa.storagePath',args.path);return;}
        if(cmd==='meeting_files_status'&&qa.storageReadFailed)throw Error('Ổ thư viện tạm thời chưa kết nối');
        if(cmd==='meeting_files_status')return {path:'D:\\Meetings\\2026-11-07 - Họp',legacy:!!options.legacyStorage,revision:2,published_revision:options.bundleFailed?1:2,error:options.bundleFailed?'Đã lưu trong Innote, chưa cập nhật xong tệp cuộc họp':null};
        if(['open_meetings_directory','open_meeting_directory','retry_meeting_bundle'].includes(cmd))return;
        if(cmd==='rescan_meeting_libraries')return 0;

        if (cmd === "plugin:updater|check") {
          if (!options.updateAvailable) return null;
          return {
            rid: 9001,
            currentVersion: "0.6.2",
            version: "0.6.3",
            date: "2026-10-30T00:00:00Z",
            body: "Bản cập nhật thử nghiệm",
            rawJson: {},
          };
        }
        if (cmd === "plugin:resources|close") return;
        if (cmd === "report_live_paint") return;
        if (cmd === "set_soniox_key") {
          qa.snapshot.soniox_configured = !!args.key;
          return;
        }
        if (cmd === "set_llm_key") {
          qa.snapshot.llm_configured = !!args.key;
          return;
        }
        if (cmd === "test_llm_connection") return "ok";
        if (cmd === "test_soniox_connection") return "ok";
        if (cmd === "list_ai_profiles") {
          qa.profiles = qa.profiles ?? [];
          return structuredClone(qa.profiles);
        }
        if (cmd === "save_ai_profile") {
          qa.profiles = qa.profiles ?? [];
          const input = args.input;
          if (input.id) {
            const p = qa.profiles.find((x) => x.id === input.id);
            if (p) {
              p.name = input.name;
              p.provider = input.provider;
              p.base_url = input.baseUrl;
            }
            return structuredClone(p);
          }
          const created = { id: "prof-new", name: input.name, provider: input.provider, base_url: input.baseUrl, has_key: !!input.apiKey, key_tail: (input.apiKey || "").slice(-4), created: "2026-10-30T00:00:00Z" };
          qa.profiles.push(created);
          return structuredClone(created);
        }
        if (cmd === "delete_ai_profile") {
          qa.profiles = (qa.profiles ?? []).filter((x) => x.id !== args.id);
          return;
        }
        if (cmd === "test_ai_connection") {
          if (!args.input.apiKey && !args.input.profileId) throw new Error("Cần nhập API key");
          if (args.input.apiKey === "invalid-key") throw new Error("Soniox 401: key không đúng hoặc đã hết hạn");
          if (options.delayAiTest) await new Promise((r) => setTimeout(r, 500));
          return "ok";
        }
        if (cmd === "start_caption") {
          qa.caption = { running: true, options: args.options };
          return;
        }
        if (cmd === "set_meeting_translation") {
          const s=qa.snapshot.sessions.find(s=>s.id===qa.snapshot.active_id);
          s.speech.translation_language=args.target;
          qa.snapshot.translation_preview=null;
          return;
        }
        if (cmd === "stop_caption") {
          if (options.delayCaptionStop) setTimeout(() => { qa.caption = { running: false }; }, 600);
          else qa.caption = { running: false };
          return;
        }
        if (cmd === "caption_running") return !!(qa.caption && qa.caption.running);
        if (cmd === "caption_snapshot") {
          if (options.delayCaptionSnapshot) await new Promise(r=>setTimeout(r,options.delayCaptionSnapshot));
          return {status:options.captionStatus ?? {state:"idle",provider:"",sending:false,message:""},translating:false,caption:{text:"",original:"",final_:true},paired:null};
        }
        if (cmd === "open_caption_window") { qa.captionVisible = true; return; }
        if (cmd === "hide_caption_window" || cmd === "close_caption_window") { qa.captionVisible = false; return; }
        if (cmd === "caption_window_visible") return !!qa.captionVisible;

        if (cmd === "fetch_llm_models") return ["gpt-4o-mini", "gpt-4o"];
        if(cmd === "cancel_correction"){qa.correctionCancelled=true;return;}
        if (cmd === "correct_transcript") {
          qa.correctionCancelled=false;
          const s=qa.snapshot.sessions.find(s=>s.id===args.id);
          qa.snapshot.corrections=[{id:args.id,stage:"Đang nghe lại file ghi âm",model:args.model,processed:0,total:10,cancelling:false}];
          await new Promise(r=>setTimeout(r,options.correctionDelay??50));
          qa.snapshot.corrections=[];
          if(qa.correctionCancelled)throw Error('Đã hủy hiệu chỉnh; giữ bản đã có');
          if(options.correctionError)throw Error(options.correctionError);
          s.corrected={text:'Nội dung từ file ghi âm đã được nhận lại.',segments:[{id:'corrected-one',start:2,end:5,text:'Nội dung từ file ghi âm đã được nhận lại.',edited:false}],provider:args.profileId==='__local__'?'local':qa.profiles.find(p=>p.id===args.profileId)?.provider,profile_id:args.profileId,model:args.model,created:'2026-11-05T00:00:00Z',audio_sha256:'fixture-sha'};
          return structuredClone(s);
        }
        if (cmd === "summarize_transcript") {
          if(options.summaryError)throw Error(options.summaryError);
          const s = qa.snapshot.sessions.find((s) => s.id === args.id);
          s.summary = {
            text: "## Tóm tắt ngắn\n- Ý chính cuộc họp 1\n- Ý chính cuộc họp 2\n\n## Quyết định\n- Đã thông qua ngân sách",
            provider: args.provider,
            model: args.model,
            source: args.source,
            prompt: args.prompt,
            created: "2026-10-30T00:00:00Z",
            source_hash: "12345678",
          };
          return structuredClone(s);
        }
        if (cmd === "delete_summary") {
          const s = qa.snapshot.sessions.find((s) => s.id === args.id);
          s.summary = null;
          return structuredClone(s);
        }

        if (cmd === "audio_path") {
          if (qa.delayAudio)
            await new Promise((r) =>
              setTimeout(r, args.id === ids[0] ? 550 : 20),
            );
          return args.id + ".wav";
        }
        if (cmd === "download_model") {
          await new Promise((r) => setTimeout(r, 30));
          if (options.failDownload)
            throw new Error("Không tải được model thử nghiệm");
          qa.snapshot.models.find((m) => m.id === args.id).installed = true;
          return;
        }
        if (cmd === "edit_session") {
          await new Promise((r) => setTimeout(r, options.slowSave ? 250 : 10));
          const s = qa.snapshot.sessions.find((s) => s.id === args.id);
          if (args.title !== null) s.title = args.title.trim() || s.title;
          if (args.notes !== null) s.notes = args.notes;
          if (args.segmentId) {
            const segment = s.segments.find((x) => x.id === args.segmentId);
            segment.text = args.text;
            segment.edited = true;
          }
          return structuredClone(s);
        }
        if (cmd === "choose_export_path") return "C:\\QA\\export." + args.format;
        if (cmd === "open_author_link") return;
        if (cmd === "export_session") {
          qa.exports.push(
            structuredClone(qa.snapshot.sessions.find((s) => s.id === args.id)),
          );
          return "C:\\QA\\export." + args.format;
        }
        if (cmd === "test_audio") {
          await new Promise((r) => setTimeout(r, options.slowTest ? 8000 : 15));
          if (options.failTest) throw new Error("Không mở được nguồn thử");
          return {
            path: "C:\\QA\\test.wav",
            peaks: {
              system: options.signal ? 0.1 : 0,
              microphone: options.signal ? 0.1 : 0,
            },
            warnings: options.testWarning
              ? Array(3).fill("Nguồn system báo gián đoạn dữ liệu âm thanh.")
              : [],
          };
        }
        if (cmd === "start") {
          if (options.slowStart) await new Promise(r=>setTimeout(r,1500));
          if(args.options.transcribe && args.options.provider==='soniox' && !qa.snapshot.soniox_configured && !qa.profiles.some(p=>p.id===args.options.profileId&&p.provider==='soniox'))throw Error('Chưa chọn cấu hình Soniox hợp lệ');
          if (options.failStart)
            throw new Error("Không mở được thiết bị âm thanh thử nghiệm");
          qa.snapshot.has_microphone = !!args.options.microphone;
          qa.snapshot.mic_muted = false;
          const s = {
            ...make(ids[2], args.options.title || "Cuộc họp mới", ""),
            language: args.options.language,
            model: args.options.model,
            status: "recording",
            duration: 0,
            processed: 0,
            segments: [],
            provider:
              ["soniox","groq"].includes(args.options.provider) &&
              (qa.snapshot.soniox_configured || qa.profiles.some(p=>p.id===args.options.profileId && p.provider===args.options.provider))
                ? args.options.provider
                : "local",
            warnings:
              args.options.provider === "soniox" &&
              !qa.snapshot.soniox_configured && !qa.profiles.some(p=>p.id===args.options.profileId && p.provider==="soniox")
                ? [
                    "Chưa có key Soniox. Phiên này dùng model local miễn phí; không gửi audio lên Soniox.",
                  ]
                : [],
          };
          if(['soniox','groq'].includes(args.options.provider))s.speech={model:args.options.provider==='groq'?args.options.model:'stt-rt-v6',profile_id:args.options.profileId,translation_language:args.options.translationLanguage??null,connections:1,uploaded_samples:0,local_fallback:false};
          if(args.options.captionsEnabled)qa.caption={running:true,options:{mode:'meeting'}};
          qa.snapshot.sessions.push(s);
          qa.snapshot.active_id = s.id;
          return structuredClone(s);
        }
        if (cmd === "pause") {
          const s = qa.snapshot.sessions.find(
            (s) => s.id === qa.snapshot.active_id,
          );
          s.status = args.paused ? "paused" : "recording";
          return;
        }
        if (cmd === "set_mic_muted") {
          if (!qa.snapshot.has_microphone) throw Error("Phiên này không thu micro của bạn");
          qa.snapshot.mic_muted = !!args.muted;
          return;
        }
        if (cmd === "stop") {
          qa.snapshot.sessions.find(
            (s) => s.id === qa.snapshot.active_id,
          ).status = "processing";
          return;
        }
        if (cmd === "refresh_devices") return;
        if (cmd === "delete_session") {
          qa.snapshot.sessions = qa.snapshot.sessions.filter(
            (s) => s.id !== args.id,
          );
          return;
        }
        if (cmd === "resume_transcript") {
          const s = qa.snapshot.sessions.find((s) => s.id === args.id);
          s.status = "completed";
          s.processed = s.duration;
          s.provider = "local";
          qa.snapshot.active_id = null;
          return;
        }
        throw new Error("Unexpected mock command: " + cmd);
      },
      { shouldMockEvents: true },
    );
  };
  await context.addInitScript({
    content: `localStorage.setItem('innote.release-notice.v1', ${JSON.stringify(JSON.stringify({version:JSON.parse(readFileSync('package.json','utf8')).version,pending:null}))});\n${mocks}\n(${init.toString()})(${JSON.stringify({ ids, options })});`,
  });
  const page = await context.newPage();
  page.setDefaultTimeout(15000);
  await page.route("http://asset.localhost/**", (r) =>
    r.fulfill({
      status: 200,
      body: (() => {
        const b = Buffer.alloc(320044);
        b.write("RIFF", 0);
        b.writeUInt32LE(b.length - 8, 4);
        b.write("WAVEfmt ", 8);
        b.writeUInt32LE(16, 16);
        b.writeUInt16LE(1, 20);
        b.writeUInt16LE(1, 22);
        b.writeUInt32LE(16000, 24);
        b.writeUInt32LE(32000, 28);
        b.writeUInt16LE(2, 32);
        b.writeUInt16LE(16, 34);
        b.write("data", 36);
        b.writeUInt32LE(320000, 40);
        return b;
      })(),
      contentType: "audio/wav",
    }),
  );
  await page.goto(process.env.UI_TEST_URL ?? "http://127.0.0.1:1420", {
    timeout: 30000,
  });
  if (options.noUiLocale) {
    await page
      .getByRole("heading", { name: "Chọn ngôn ngữ giao diện", exact: true })
      .waitFor();
    return { page, context };
  }
  await page.getByRole("button", { name: "Danh sách cuộc họp", exact: true }).waitFor();
  return { page, context };
}
async function openMeeting(p, name) {
  const list = p.locator("dialog.meetings-dialog");
  if (!(await list.isVisible().catch(() => false))) {
    await p
      .getByRole("button", { name: "Danh sách cuộc họp", exact: true })
      .click();
  }
  await list
    .locator(".meeting-card")
    .filter({ hasText: name })
    .locator(".meeting-card-main")
    .click();
}
async function check(name, options, fn) {
  if (process.env.UI_TEST_FILTER && !name.includes(process.env.UI_TEST_FILTER)) return;
  const { page, context } = await setup(options);
  try {
    await fn(page);
    results.push({ name, passed: true });
    console.log("PASS " + name);
  } catch (error) {
    results.push({ name, passed: false, error: String(error) });
    await page.screenshot({
      path: join(output, `${results.length}-failed.png`),
    });
    console.error("FAIL " + name + ": " + error);
    process.exitCode = 1;
  } finally {
    await context.close();
    writeFileSync(
      join(output, "results.json"),
      JSON.stringify(results, null, 2),
    );
  }
}
try {
  await check('Meeting storage legacy guide explains database and safe export',{legacyStorage:true},async p=>{
    await openMeeting(p,'Cuộc họp Alpha');const guide=p.locator('.meeting-files + .file-guide');await guide.locator('summary').click();
    assert((await guide.innerText()).includes('chưa phải các file Markdown riêng'));
    assert((await guide.innerText()).includes('Không cấp quyền toàn bộ nơi lưu cũ'));
    assert.equal(await guide.locator('table').count(),0);
  });
  for(const viewport of [{width:900,height:640},{width:1180,height:800}]) {
    await check(`Meeting storage file guide explains every output ${viewport.width}`,{viewport},async p=>{
      await p.getByRole('button',{name:'Cài đặt',exact:true}).click();
      const panel=p.locator('.meeting-storage');
      assert.equal(await panel.locator('details').count(),0);
      assert.equal(await panel.evaluate(el=>el.scrollWidth>el.clientWidth),false);
      await panel.screenshot({path:join(output,`storage-simple-${viewport.width}.png`)});
      await openMeeting(p,'Cuộc họp Alpha');
      const meeting=p.locator('.meeting-files + .file-guide');await meeting.locator('summary').click();
      for(const name of ['recording.wav','original.md','corrected.md / corrected.srt','meeting.json','.innote/work/','.innote-identity.json'])assert((await meeting.innerText()).includes(name));
      assert((await meeting.innerText()).includes('Sửa file bên ngoài không cập nhật ngược vào Innote'));
      assert.equal(await meeting.evaluate(el=>el.scrollWidth>el.clientWidth),false);
    });
  }
  for(const viewport of [{width:900,height:640},{width:1180,height:800}]) {
    await check(`Meeting storage explicit confirmation and restart ${viewport.width}`,{viewport},async p=>{
      await p.getByRole('button',{name:'Cài đặt',exact:true}).click();
      const panel=p.locator('.meeting-storage');await panel.scrollIntoViewIfNeeded();
      await p.getByRole('button',{name:'Chọn thư mục',exact:true}).click();
      await p.getByLabel('Thư mục cuộc họp đã chọn').waitFor();
      assert.equal(await p.evaluate(()=>qa.calls.filter(x=>x.cmd==='set_meetings_directory').length),0);
      await p.getByRole('button',{name:'Hủy',exact:true}).click();
      assert.equal(await p.getByLabel('Thư mục cuộc họp đã chọn').count(),0);
      await p.getByRole('button',{name:'Chọn thư mục',exact:true}).click();
      await p.getByRole('button',{name:'Dùng cho cuộc họp mới',exact:true}).click();
      await p.getByText('Đã đổi nơi lưu cho cuộc họp mới',{exact:true}).waitFor();
      assert.equal(await p.evaluate(()=>localStorage.getItem('qa.storagePath')),'D:\\Meetings');
      assert.equal(await panel.evaluate(el=>el.scrollWidth>el.clientWidth),false);
      await panel.screenshot({path:join(output,`storage-${viewport.width}.png`)});
      await p.reload();await p.getByRole('button',{name:'Cài đặt',exact:true}).click();
      await p.locator('.meeting-storage .storage-destination').getByText('D:\\Meetings',{exact:true}).waitFor();
    });
  }
  await check('Meeting storage picker cancellation keeps current destination',{cancelStoragePicker:true},async p=>{
    await p.getByRole('button',{name:'Cài đặt',exact:true}).click();await p.getByRole('button',{name:'Chọn thư mục',exact:true}).click();
    assert.equal(await p.getByRole('button',{name:'Dùng cho cuộc họp mới',exact:true}).count(),0);
    assert.equal(await p.evaluate(()=>qa.calls.filter(x=>x.cmd==='set_meetings_directory').length),0);
  });
  await check('Meeting storage clears stale read error after library reconnects',{},async p=>{
    await p.evaluate(()=>qa.storageReadFailed=true);await openMeeting(p,'Cuộc họp Alpha');
    await p.getByRole('alert').filter({hasText:'Ổ thư viện tạm thời chưa kết nối'}).waitFor();
    await p.evaluate(()=>qa.storageReadFailed=false);
    await p.locator('.meeting-files').waitFor();
    assert.equal(await p.getByRole('alert').filter({hasText:'Ổ thư viện tạm thời chưa kết nối'}).count(),0);
  });
  await check('Meeting storage failed commit preserves old destination',{failStorage:true},async p=>{
    await p.getByRole('button',{name:'Cài đặt',exact:true}).click();await p.getByRole('button',{name:'Chọn thư mục',exact:true}).click();
    await p.getByRole('button',{name:'Dùng cho cuộc họp mới',exact:true}).click();
    await p.getByText('Hãy kết thúc phiên trước khi đổi nơi lưu',{exact:true}).waitFor();
    assert.equal(await p.evaluate(()=>localStorage.getItem('qa.storagePath')),null);
    assert.equal(await p.getByText('Đã đổi nơi lưu cho cuộc họp mới',{exact:true}).count(),0);
  });
  for(const viewport of [{width:900,height:640},{width:1180,height:800}]) {
    await check(`Compact workflow layout ${viewport.width}`,{viewport,profiles:mockProfiles,naturalMeetingDefaults:true},async p=>{
      const metrics={};
      const measure=async (selector,name)=>{
        const node=p.locator(selector);await node.evaluate(el=>el.scrollTop=0);
        metrics[name]=await node.evaluate(el=>({height:el.scrollHeight,viewport:el.clientHeight,width:el.clientWidth,overflow:el.scrollWidth>el.clientWidth}));
        assert(!metrics[name].overflow,`${name} horizontal overflow`);
        if(name==='meeting')assert(await p.locator('.source-row').count()===2,'Both audio inputs remain available above the always-open appearance settings');
        else assert(await p.locator('.caption-setup').evaluate(el=>el.getBoundingClientRect().height<650),'Primary caption controls stay compact above the requested expanded appearance settings');
        await p.screenshot({path:join(output,`compact-${name}-${viewport.width}.png`)});
      };
      await measure('.meeting-page','meeting');
      await p.getByRole('button',{name:'Chọn model Soniox để bật dịch',exact:true}).click();
      assert(await p.getByLabel('Model dùng chung cho cuộc họp').evaluate(el=>document.activeElement===el),'Disabled start has a direct path to the missing model');
      assert(!await p.locator('.preparing > .workspace').isVisible(),'No empty transcript panel before recording');
      metrics.sections=await p.locator('.setup-panel').evaluate(el=>[...el.children].map(c=>({class:c.className,height:Math.round(c.getBoundingClientRect().height),margin:getComputedStyle(c).margin})));
      await p.locator('.meeting-page').evaluate(el=>el.scrollTop=el.scrollHeight);
      const stickyStart=await p.getByRole('button',{name:'Bắt đầu ghi',exact:true}).boundingBox();assert(stickyStart.y>=59&&stickyStart.y+stickyStart.height<=viewport.height,'Start recording stays reachable after scrolling');
      const appearance=p.locator('#meeting-caption-style');assert(await appearance.getByLabel('Song ngữ',{exact:true}).isVisible(),'Meeting appearance is always expanded');assert.equal(await appearance.locator('summary').count(),0);
      assert.equal(await appearance.locator('.caption-customize-fields').evaluate(el=>getComputedStyle(el).gridTemplateColumns.split(' ').length),2,'Optional typography uses at most two columns');
      await p.screenshot({path:join(output,`compact-meeting-bottom-${viewport.width}.png`)});
      await p.getByRole('button',{name:'Chỉ phụ đề',exact:true}).click();
      await p.getByLabel('Cấu hình AI phụ đề').selectOption('prof-soniox');
      await measure('.caption-page','captions');
      const captionStart=await p.getByRole('button',{name:'Bắt đầu phụ đề',exact:true}).boundingBox();assert(captionStart.y>=59&&captionStart.y+captionStart.height<=viewport.height,'Caption start stays visible without scrolling through the form');
      await p.getByText('Giải thích mức xử lý',{exact:true}).focus();await p.keyboard.press('Enter');assert(await p.locator('.processing-description').isVisible());
      await p.getByText('Giải thích mức xử lý',{exact:true}).focus();await p.keyboard.press('Enter');
      const radio=p.getByRole('radio',{name:'Phụ đề · Nhanh',exact:true});await radio.check();assert(await radio.isChecked(),'Sticky actions do not block processing controls');
      writeFileSync(join(output,`compact-metrics-${viewport.width}.json`),JSON.stringify(metrics,null,2));
    });
  }
  const groqSelectionProfiles=[...mockProfiles,
    {id:'groq-one',name:'groq1',provider:'groq',has_key:true,base_url:'',key_tail:'one',created:'now'},
    {id:'groq-two',name:'groq2',provider:'groq',has_key:true,base_url:'',key_tail:'two',created:'now'}];
  await check("Soniox v6 upgrades old meeting and standalone presets before applying or starting",{profiles:mockProfiles},async p=>{
    await p.evaluate(()=>{
      const c={profileId:'prof-soniox',model:'stt-rt-v5'},style={fontSize:34,color:'#123456',align:'left',showOriginal:true,transparent:false,backgroundColor:'#112233'};
      localStorage.setItem('innote.workflow-presets.v1',JSON.stringify({version:1,presets:[
        {id:'legacy-meeting',name:'Mẫu họp cũ',mode:'meeting',tasks:{transcription:c,caption:{...c,translate:true,targetLang:'vi'}},source:'en',quality:'accurate',captions:true,translate:true,target:'vi',transcribe:true,captionStyle:style},
        {id:'legacy-caption',name:'Mẫu phụ đề cũ',mode:'captionsonly',tasks:{caption:{...c,translate:false,targetLang:'vi'}},source:'en',quality:'fast',translate:false,target:'vi',captionStyle:style}
      ]}));
      const prefs=JSON.parse(localStorage.getItem('may.preferences'));localStorage.setItem('may.preferences',JSON.stringify({...prefs,speechSelection:c,language:'en',meetingCaptions:true,meetingTranslate:true}));
      localStorage.setItem('may.task_assign.meeting',JSON.stringify({transcription:c,caption:{...c,translate:true,targetLang:'vi'}}));
      localStorage.setItem('may.task_assign.captionsonly',JSON.stringify({caption:{...c,translate:false,targetLang:'vi'}}));
      localStorage.setItem('may.caption.standalone',JSON.stringify({...c,translate:false,targetLang:'vi',sourceLang:'en',device:'speaker'}));
      localStorage.setItem('may.speech.processing-levels',JSON.stringify({[JSON.stringify(['meeting','prof-soniox','stt-rt-v5','en',true])]:'accurate'}));
    });
    await p.reload();await p.getByText('Đã nâng cấu hình và mẫu Soniox lên stt-rt-v6. Bản chép đã lưu và API key được giữ nguyên.',{exact:true}).waitFor();
    await p.getByLabel('Mẫu cấu hình cuộc họp').selectOption('legacy-meeting');await p.getByRole('button',{name:'Áp mẫu',exact:true}).click();
    assert((await p.getByLabel('Model chép lời Soniox').innerText()).includes('stt-rt-v6'));assert(await p.getByRole('radio',{name:'Cuộc họp · Chính xác',exact:true}).isChecked());
    assert.equal(await p.getByLabel('Cỡ chữ phụ đề').inputValue(),'34');
    await p.getByRole('button',{name:'Chỉ phụ đề',exact:true}).click();await p.getByLabel('Mẫu cấu hình phụ đề').selectOption('legacy-caption');await p.getByRole('button',{name:'Áp mẫu',exact:true}).click();
    assert.equal(await p.getByLabel('Model phụ đề').inputValue(),'stt-rt-v6');assert(!await p.getByLabel('Dịch sang',{exact:true}).isChecked());
    assert(!(await p.getByLabel('Model phụ đề').locator('option').allTextContents()).some(s=>s.includes('async')));
    await p.getByRole('button',{name:'Bắt đầu phụ đề',exact:true}).click();const o=await p.evaluate(()=>window.qa.calls.find(c=>c.cmd==='start_caption').args.options);assert.equal(o.model,'stt-rt-v6');assert.equal(o.profileId,'prof-soniox');assert.equal(o.translate,false);
    const saved=await p.evaluate(()=>JSON.parse(localStorage.getItem('innote.workflow-presets.v1')));assert(saved.presets.every(p=>(p.tasks.caption?.model??p.tasks.transcription?.model)==='stt-rt-v6'));
  });
  await check("Groq visibility keeps its models visible and explains translation in captions",{profiles:groqSelectionProfiles},async p=>{
    await p.getByRole('button',{name:'Chỉ phụ đề',exact:true}).click();await p.getByLabel('Dịch sang',{exact:true}).check();
    for(const id of ['groq-one','groq-two']){await p.getByLabel('Cấu hình AI phụ đề').selectOption(id);assert((await p.getByLabel('Model phụ đề').locator('option').allTextContents()).some(x=>x.includes(id==='groq-one'?'groq1/whisper-large-v3':'groq2/whisper-large-v3')));}
    assert(await p.getByRole('button',{name:'Bắt đầu phụ đề',exact:true}).isDisabled());await p.getByRole('button',{name:'Chỉ chép lời gốc',exact:true}).click();assert(!await p.getByLabel('Dịch sang',{exact:true}).isChecked());
    await p.getByLabel('Model phụ đề').selectOption('whisper-large-v3');await p.reload();await p.getByRole('button',{name:'Chỉ phụ đề',exact:true}).click();assert.equal(await p.getByLabel('Cấu hình AI phụ đề').inputValue(),'groq-two');assert.equal(await p.getByLabel('Model phụ đề').inputValue(),'whisper-large-v3');
    await p.getByRole('button',{name:'Bắt đầu phụ đề',exact:true}).click();const o=await p.evaluate(()=>window.qa.calls.find(c=>c.cmd==='start_caption').args.options);assert.equal(o.profileId,'groq-two');assert.equal(o.model,'whisper-large-v3');assert.equal(o.translate,false);
    await p.getByRole('button',{name:'Dừng phụ đề',exact:true}).click();
  });
  await check("Groq visibility meeting preserves profile model and shares captions without local models",{profiles:groqSelectionProfiles,missingModels:true,missingBundled:true},async p=>{
    await p.getByRole('button',{name:'Cuộc họp mới',exact:true}).click();await p.getByLabel('Cách chép lời',{exact:true}).selectOption('groq');await p.getByLabel('Cấu hình Groq',{exact:true}).selectOption('groq-two');await p.getByLabel('Model chép lời Groq').selectOption('whisper-large-v3');
    assert(await p.getByRole('button',{name:'Bắt đầu ghi',exact:true}).isDisabled());await p.getByRole('button',{name:'Chỉ chép lời gốc',exact:true}).click();await p.getByRole('radio',{name:'Cuộc họp · Nhanh',exact:true}).check();await p.reload();
    assert.equal(await p.getByLabel('Cách chép lời',{exact:true}).inputValue(),'groq');assert.equal(await p.getByLabel('Cấu hình Groq').inputValue(),'groq-two');assert.equal(await p.getByLabel('Model chép lời Groq').inputValue(),'whisper-large-v3');assert(await p.getByRole('radio',{name:'Cuộc họp · Nhanh',exact:true}).isChecked());
    await p.getByRole('button',{name:'Bắt đầu ghi',exact:true}).click();await p.getByRole('button',{name:'Kết thúc',exact:true}).waitFor();
    const o=await p.evaluate(()=>window.qa.calls.find(c=>c.cmd==='start').args.options);assert.equal(o.provider,'groq');assert.equal(o.model,'whisper-large-v3');assert.equal(o.profileId,'groq-two');assert.equal(o.translationLanguage,null);assert.equal(o.captionsEnabled,true);assert.equal(o.localFallback,false);
    assert(!await p.evaluate(()=>window.qa.calls.some(c=>c.cmd==='start_caption')),'Starting Groq meeting captions never opens a second uploader');assert(await p.getByLabel('Dịch sang khi bật phụ đề').isDisabled());
  });
  await check("Groq visibility presets save and apply both speech models with an explicit backup",{profiles:groqSelectionProfiles},async p=>{
    await p.getByRole('button',{name:'Mẫu cấu hình',exact:true}).click();await p.getByRole('button',{name:'Tạo mẫu',exact:true}).click();
    await p.getByLabel('Tên mẫu',{exact:true}).fill('Groq meeting');await p.getByLabel('Bật phụ đề khi bắt đầu',{exact:true}).check();await p.getByLabel('Chép lời (transcription)',{exact:true}).selectOption('groq-one');await p.getByLabel('Dịch sang trong mẫu').uncheck();await p.getByLabel('Chép lời · Model',{exact:true}).selectOption('whisper-large-v3');
    assert.equal(await p.getByLabel('Model phụ đề',{exact:true}).inputValue(),'whisper-large-v3');
    assert.equal(await p.getByLabel('Hiệu chỉnh từ bản thu',{exact:true}).count(),0);assert.equal(await p.getByLabel('Tóm tắt',{exact:true}).count(),0);
    await p.getByLabel('Dự phòng · Chép lời (transcription)',{exact:true}).selectOption(JSON.stringify({profileId:'groq-two',model:'whisper-large-v3-turbo'}));
    await p.getByRole('button',{name:'Lưu mẫu',exact:true}).click();const row=p.locator('.preset-row').filter({hasText:'Groq meeting'});await row.getByRole('button',{name:'Sử dụng',exact:true}).click();
    assert.equal(await p.getByLabel('Cấu hình Groq').inputValue(),'groq-one');assert.equal(await p.getByLabel('Model chép lời Groq').inputValue(),'whisper-large-v3');
    await p.getByRole('button',{name:'Bắt đầu ghi',exact:true}).click();const o=await p.evaluate(()=>window.qa.calls.find(c=>c.cmd==='start').args.options);assert.deepEqual(o.transcriptionFallback,{profileId:'groq-two',model:'whisper-large-v3-turbo'});
  });
  await check("New meetings default captions and translation on while applied presets keep explicit choices",{profiles:mockProfiles,naturalMeetingDefaults:true},async p=>{
    assert(await p.getByLabel('Bật phụ đề khi bắt đầu cuộc họp').isChecked());assert(await p.getByLabel('Dịch sang trong cuộc họp').isChecked());
    await p.getByLabel('Dịch sang trong cuộc họp').uncheck();await p.getByLabel('Bật phụ đề khi bắt đầu cuộc họp').uncheck();
    await p.getByRole('button',{name:'Cuộc họp mới',exact:true}).click();assert(await p.getByLabel('Bật phụ đề khi bắt đầu cuộc họp').isChecked());assert(await p.getByLabel('Dịch sang trong cuộc họp').isChecked());
    await p.evaluate(()=>{const key='innote.workflow-presets.v1',root=JSON.parse(localStorage.getItem(key));root.presets.push({id:'quiet-preset',name:'Không phụ đề',mode:'meeting',tasks:{transcription:{profileId:'__local__',model:'moonshine-vi'}},source:'vi',quality:'balanced',captions:false,translate:false,target:'vi',transcribe:true});localStorage.setItem(key,JSON.stringify(root))});await p.reload();await p.getByLabel('Mẫu cấu hình cuộc họp').selectOption('quiet-preset');await p.getByRole('button',{name:'Áp mẫu',exact:true}).click();
    assert(!await p.getByLabel('Bật phụ đề khi bắt đầu cuộc họp').isChecked());assert(!await p.getByLabel('Dịch sang trong cuộc họp').isChecked());assert(await p.getByRole('button',{name:'Bắt đầu ghi',exact:true}).isEnabled());
  });
  await check("Meeting shows applied subtitle appearance and edits only the session copy",{profiles:mockProfiles},async p=>{
    await p.evaluate(()=>{const key='innote.workflow-presets.v1',root=JSON.parse(localStorage.getItem(key)),base=root.presets.find(p=>p.mode==='meeting');root.presets.push({...base,id:'visible-style',name:'Chữ lớn',captionStyle:{fontSize:36,align:'left',color:'#123456',backgroundColor:'#223344',showOriginal:true,transparent:false}});localStorage.setItem(key,JSON.stringify(root))});await p.reload();
    const panel=p.locator('#meeting-caption-style');await p.getByLabel('Mẫu cấu hình cuộc họp').selectOption('visible-style');await p.getByRole('button',{name:'Áp mẫu',exact:true}).click();
    assert(await panel.evaluate(el=>!!el.closest('.meeting-caption-setup')),'Appearance belongs inside the subtitle setup group');
    const captionToggle=await p.getByLabel('Bật phụ đề khi bắt đầu cuộc họp').boundingBox(),appearance=await panel.boundingBox();assert(appearance.y>captionToggle.y);
    await p.waitForFunction(()=>document.querySelector('#meeting-caption-style [aria-label="Kiểu phụ đề hiện tại"]')?.textContent?.includes('36px'));
    assert(await panel.getByLabel('Cỡ chữ phụ đề').isVisible());assert.equal(await panel.getByLabel('Cỡ chữ phụ đề').inputValue(),'36');assert.equal(await panel.getByLabel('Căn chữ phụ đề').inputValue(),'left');assert(await panel.getByLabel('Song ngữ',{exact:true}).isChecked());
    await p.screenshot({path:join(output,'meeting-applied-subtitle-style.png')});
    await panel.getByLabel('Cỡ chữ phụ đề').fill('28');await panel.getByLabel('Cỡ chữ phụ đề').dispatchEvent('input');assert.equal(await p.getByLabel('Mẫu cấu hình cuộc họp').inputValue(),'');
    assert.equal(await p.evaluate(()=>JSON.parse(localStorage.getItem('innote.workflow-presets.v1')).presets.find(p=>p.id==='visible-style').captionStyle.fontSize),36);
    await p.getByRole('button',{name:'Bắt đầu ghi',exact:true}).click();await p.getByRole('button',{name:'Cài đặt phụ đề',exact:true}).click();
    assert.equal(await p.locator('#meeting-caption-style').getByLabel('Cỡ chữ phụ đề').inputValue(),'28');assert.equal(await p.locator('.preset-page').count(),0,'Runtime appearance controls stay on the meeting screen');
    const activeStyle=p.locator('#meeting-caption-style').getByLabel('Cỡ chữ phụ đề');assert(await activeStyle.isVisible());await activeStyle.fill('30');await activeStyle.dispatchEvent('input');
    assert.equal(await p.evaluate(()=>JSON.parse(localStorage.getItem('innote.caption.style')).fontSize),30);
    const styleToggle=p.locator('#meeting-caption-style-toggle');
    assert.equal(await styleToggle.getAttribute('aria-expanded'),'true');
    await styleToggle.click();await panel.waitFor({state:'detached'});
    assert.equal(await styleToggle.getAttribute('aria-expanded'),'false');
    await styleToggle.focus();await p.keyboard.press('Enter');await panel.waitFor({state:'visible'});
    assert.equal(await panel.getByLabel('Cỡ chữ phụ đề').inputValue(),'30');
    await p.setViewportSize({width:900,height:640});
    await panel.getByRole('button',{name:'Đóng cài đặt phụ đề',exact:true}).scrollIntoViewIfNeeded();
    await p.screenshot({path:join(output,'meeting-runtime-caption-settings-close.png')});
    await panel.getByRole('button',{name:'Đóng cài đặt phụ đề',exact:true}).click();await panel.waitFor({state:'detached'});
    assert(await styleToggle.evaluate(el=>el===document.activeElement),'Closing restores focus to the opener');
    await p.keyboard.press('Space');await panel.waitFor({state:'visible'});
    assert.equal(await panel.getByLabel('Cỡ chữ phụ đề').inputValue(),'30');
    assert.equal(await p.evaluate(()=>JSON.parse(localStorage.getItem('innote.workflow-presets.v1')).presets.find(p=>p.id==='visible-style').captionStyle.fontSize),36);
    assert.equal(await p.evaluate(()=>window.qa.calls.filter(c=>c.cmd==='start').length),1);assert(!await p.evaluate(()=>window.qa.calls.some(c=>c.cmd==='stop'||c.cmd==='stop_caption')));
  });
  await check("Preset audit standalone reload, fallback and switching never launch a pending template",{profiles:[...mockProfiles,{id:'prof-groq',name:'Groq audit',provider:'groq',has_key:true,base_url:'',key_tail:'mock',created:'now'}]},async p=>{
    await p.evaluate(()=>{const key='innote.workflow-presets.v1',root=JSON.parse(localStorage.getItem(key)),style=root.presets[0].captionStyle;root.presets.push(
      {id:'audit-groq',name:'Audit Groq',mode:'captionsonly',tasks:{caption:{profileId:'prof-groq',model:'whisper-large-v3',targetLang:'vi',translate:false,fallback:{profileId:'prof-soniox',model:'stt-rt-v6'}}},source:'en',quality:'accurate',translate:false,target:'vi',transcribe:true,captionStyle:{...style,fontSize:40}},
      {id:'audit-soniox',name:'Audit Soniox',mode:'captionsonly',tasks:{caption:{profileId:'prof-soniox',model:'stt-rt-v6',targetLang:'vi',translate:true}},source:'en',quality:'balanced',translate:true,target:'vi',transcribe:true,captionStyle:{...style,fontSize:24}});localStorage.setItem(key,JSON.stringify(root))});
    await p.reload();await p.getByRole('button',{name:'Chỉ phụ đề',exact:true}).click();await p.getByLabel('Mẫu cấu hình phụ đề').selectOption('audit-groq');await p.getByRole('button',{name:'Áp mẫu',exact:true}).click();await p.reload();await p.getByRole('button',{name:'Chỉ phụ đề',exact:true}).click();
    assert.equal(await p.getByLabel('Cấu hình AI phụ đề').inputValue(),'prof-groq');assert.equal(await p.getByLabel('Model phụ đề',{exact:true}).inputValue(),'whisper-large-v3');
    await p.getByRole('button',{name:'Bắt đầu phụ đề',exact:true}).click();const first=await p.evaluate(()=>window.qa.calls.find(c=>c.cmd==='start_caption').args.options);assert.equal(first.speechQuality,'accurate');assert.equal(first.translate,false);assert.deepEqual(first.fallback,{profileId:'prof-soniox',model:'stt-rt-v6'});
    await p.getByRole('button',{name:'Dừng phụ đề',exact:true}).click();await p.getByRole('button',{name:'Bắt đầu phụ đề',exact:true}).waitFor();
    await p.getByLabel('Mẫu cấu hình phụ đề').selectOption('audit-soniox');assert(await p.getByRole('button',{name:'Bắt đầu phụ đề',exact:true}).isDisabled());assert.equal(await p.getByLabel('Cấu hình AI phụ đề').inputValue(),'prof-groq');
    await p.getByLabel('Mẫu cấu hình phụ đề').selectOption('');assert(await p.getByRole('button',{name:'Bắt đầu phụ đề',exact:true}).isEnabled());
    await p.getByLabel('Mẫu cấu hình phụ đề').selectOption('audit-soniox');await p.getByRole('button',{name:'Áp mẫu',exact:true}).click();await p.getByRole('button',{name:'Áp mẫu',exact:true}).click();
    assert.equal(await p.evaluate(()=>window.qa.calls.filter(c=>c.cmd==='start_caption').length),1);
    await p.getByRole('button',{name:'Bắt đầu phụ đề',exact:true}).click();const second=await p.evaluate(()=>window.qa.calls.filter(c=>c.cmd==='start_caption').at(-1).args.options);assert.equal(second.profileId,'prof-soniox');assert.equal(second.model,'stt-rt-v6');assert.equal(second.translate,true);assert.equal(second.targetLanguage,'vi');assert.equal(second.fallback,null);
  });
  await check("Preset audit rejects unsupported languages and conflicting shared meeting models",{profiles:mockProfiles},async p=>{
    await p.evaluate(()=>{const key='innote.workflow-presets.v1',root=JSON.parse(localStorage.getItem(key)),base=root.presets.find(p=>p.mode==='meeting');root.presets.push({...base,id:'audit-language',name:'Bad language',source:'ja'},{...base,id:'audit-shared',name:'Conflicting shared',captions:true,translate:true,tasks:{transcription:{profileId:'__local__',model:'moonshine-vi'},caption:{profileId:'prof-soniox',model:'stt-rt-v6',targetLang:'vi',translate:true}}});localStorage.setItem(key,JSON.stringify(root))});await p.reload();
    for(const id of ['audit-language','audit-shared']){await p.getByLabel('Mẫu cấu hình cuộc họp').selectOption(id);await p.getByRole('button',{name:'Áp mẫu',exact:true}).click();assert.equal(await p.getByText(/Áp mẫu thành công:/).count(),0);assert(await p.getByRole('button',{name:'Bắt đầu ghi',exact:true}).isDisabled());}
    assert(!await p.evaluate(()=>window.qa.calls.some(c=>c.cmd==='start')));
  });
  await check("Preset audit keeps Soniox translation, captions, quality and fallback after reload",{profiles:[...mockProfiles,{...mockProfiles[0],id:'prof-soniox-backup',name:'Soniox backup'}]},async p=>{
    await p.evaluate(()=>{const key='innote.workflow-presets.v1',root=JSON.parse(localStorage.getItem(key));root.presets.push({id:'audit-meeting',name:'Audit meeting',mode:'meeting',tasks:{transcription:{profileId:'prof-soniox',model:'stt-rt-v6',fallback:{profileId:'prof-soniox-backup',model:'stt-rt-v6'}},caption:{profileId:'prof-soniox',model:'stt-rt-v6',translate:true,targetLang:'en'}},source:'zh',quality:'fast',captions:true,translate:true,target:'en',transcribe:true,localFallback:false,captionStyle:{fontSize:34,color:'#123456',align:'left',showOriginal:true,transparent:false,backgroundColor:'#112233'}});localStorage.setItem(key,JSON.stringify(root))});
    await p.reload();const captionAssign=await p.evaluate(()=>localStorage.getItem('may.task_assign.captionsonly'));
    await p.getByLabel('Mẫu cấu hình cuộc họp').selectOption('audit-meeting');await p.getByRole('button',{name:'Áp mẫu',exact:true}).click();
    await p.getByText('Áp mẫu thành công: Audit meeting',{exact:true}).waitFor();await p.reload();
    assert.equal(await p.getByLabel('Cách chép lời',{exact:true}).inputValue(),'soniox');assert.equal(await p.getByLabel('Ngôn ngữ',{exact:true}).inputValue(),'zh');
    assert(await p.getByLabel('Bật phụ đề khi bắt đầu cuộc họp').isChecked());assert(await p.getByLabel('Dịch sang trong cuộc họp').isChecked());assert.equal(await p.getByLabel('Ngôn ngữ bản dịch',{exact:true}).inputValue(),'en');
    assert.equal(await p.evaluate(()=>localStorage.getItem('may.task_assign.captionsonly')),captionAssign);
    await p.getByRole('button',{name:'Bắt đầu ghi',exact:true}).click();const o=await p.evaluate(()=>window.qa.calls.find(c=>c.cmd==='start').args.options);
    assert.equal(o.provider,'soniox');assert.equal(o.language,'zh');assert.equal(o.speechQuality,'fast');assert.equal(o.translationLanguage,'en');assert.equal(o.captionsEnabled,true);assert.deepEqual(o.transcriptionFallback,{profileId:'prof-soniox-backup',model:'stt-rt-v6'});
  });
  await check("Preset audit blocks false success and restores storage when applying fails",{profiles:mockProfiles},async p=>{
    await p.evaluate(()=>{const key='innote.workflow-presets.v1',root=JSON.parse(localStorage.getItem(key));const base=root.presets.find(p=>p.mode==='meeting');root.presets.push({...base,id:'audit-failure',name:'Audit write failure',source:'en',tasks:{transcription:{profileId:'__local__',model:'moonshine-en'}},captionStyle:{...base.captionStyle,fontSize:58}});localStorage.setItem(key,JSON.stringify(root));});await p.reload();
    const before=await p.evaluate(()=>({assignment:localStorage.getItem('may.task_assign.meeting'),levels:localStorage.getItem('may.speech.processing-levels'),style:localStorage.getItem('innote.caption.style')}));
    const language=await p.getByLabel('Ngôn ngữ',{exact:true}).inputValue();
    await p.evaluate(()=>{window.originalSetItem=Storage.prototype.setItem;Storage.prototype.setItem=function(k,v){if(k==='innote.caption.style'&&JSON.parse(v).fontSize===58)throw Error('quota');return window.originalSetItem.call(this,k,v)}});
    await p.getByLabel('Mẫu cấu hình cuộc họp').selectOption('audit-failure');await p.getByRole('button',{name:'Áp mẫu',exact:true}).click();
    await p.getByText(/Không lưu được cấu hình mẫu trên máy/).waitFor();assert.equal(await p.getByLabel('Ngôn ngữ',{exact:true}).inputValue(),language);
    assert.deepEqual(await p.evaluate(()=>({assignment:localStorage.getItem('may.task_assign.meeting'),levels:localStorage.getItem('may.speech.processing-levels'),style:localStorage.getItem('innote.caption.style')})),before);
    assert.equal(await p.getByText('Áp mẫu thành công: Audit write failure',{exact:true}).count(),0);assert(await p.getByRole('button',{name:'Bắt đầu ghi',exact:true}).isDisabled());
    await p.evaluate(()=>Storage.prototype.setItem=window.originalSetItem);await p.getByRole('button',{name:'Áp mẫu',exact:true}).click();await p.getByText('Áp mẫu thành công: Audit write failure',{exact:true}).waitFor();
  });
  await check("Meeting preset selection waits for Apply and confirms the applied model and style",{profiles:mockProfiles},async p=>{
    await p.getByRole('button',{name:'Mẫu cấu hình',exact:true}).click();await p.getByRole('button',{name:'Tạo mẫu',exact:true}).click();
    await p.getByLabel('Tên mẫu',{exact:true}).fill('Áp họp tiếng Anh');await p.getByLabel('Ngôn ngữ gốc của mẫu').selectOption('en');await p.getByLabel('Model chép lời local của mẫu').selectOption('moonshine-en');await p.getByLabel('Mức xử lý của mẫu').selectOption('fast');await p.getByLabel('Cỡ chữ phụ đề').fill('38');await p.getByLabel('Cỡ chữ phụ đề').dispatchEvent('input');await p.getByRole('button',{name:'Lưu mẫu',exact:true}).click();
    const id=await p.evaluate(()=>JSON.parse(localStorage.getItem('innote.workflow-presets.v1')).presets.find(x=>x.name==='Áp họp tiếng Anh').id);
    await p.getByRole('button',{name:'Cuộc họp mới',exact:true}).click();const original=await p.getByLabel('Ngôn ngữ',{exact:true}).inputValue();const style=await p.evaluate(()=>localStorage.getItem('innote.caption.style'));
    await p.getByLabel('Mẫu cấu hình cuộc họp').selectOption(id);
    assert(await p.getByRole('button',{name:'Bắt đầu ghi',exact:true}).isDisabled(),'Unapplied choice cannot launch old configuration');
    assert.equal(await p.getByLabel('Ngôn ngữ',{exact:true}).inputValue(),original);assert.equal(await p.evaluate(()=>localStorage.getItem('innote.caption.style')),style);
    await p.getByRole('button',{name:'Áp mẫu',exact:true}).click();
    await p.getByText('Áp mẫu thành công: Áp họp tiếng Anh',{exact:true}).waitFor();assert.equal(await p.getByLabel('Ngôn ngữ',{exact:true}).inputValue(),'en');assert.equal(await p.evaluate(()=>JSON.parse(localStorage.getItem('innote.caption.style')).fontSize),38);
    await p.screenshot({path:join(output,'meeting-preset-applied.png')});
    await p.getByRole('button',{name:'Bắt đầu ghi',exact:true}).click();const o=await p.evaluate(()=>window.qa.calls.find(c=>c.cmd==='start').args.options);assert.equal(o.model,'moonshine-en');assert.equal(o.speechQuality,'fast');
  });
  await check("Standalone preset selection waits for Apply and confirms its provider",{profiles:mockProfiles},async p=>{
    await p.getByRole('button',{name:'Mẫu cấu hình',exact:true}).click();await p.getByRole('tab',{name:'Phụ đề',exact:true}).click();await p.getByRole('button',{name:'Tạo mẫu',exact:true}).click();await p.getByLabel('Tên mẫu',{exact:true}).fill('Áp phụ đề Soniox');await p.getByLabel('Phụ đề trực tiếp',{exact:true}).selectOption('prof-soniox');await p.getByRole('button',{name:'Lưu mẫu',exact:true}).click();
    const id=await p.evaluate(()=>JSON.parse(localStorage.getItem('innote.workflow-presets.v1')).presets.find(x=>x.name==='Áp phụ đề Soniox').id);
    await p.getByRole('button',{name:'Chỉ phụ đề',exact:true}).click();const original=await p.getByLabel('Cấu hình AI phụ đề').inputValue();await p.getByLabel('Mẫu cấu hình phụ đề').selectOption(id);assert.equal(await p.getByLabel('Cấu hình AI phụ đề').inputValue(),original);
    await p.getByRole('button',{name:'Áp mẫu',exact:true}).click();await p.getByText('Áp mẫu thành công: Áp phụ đề Soniox',{exact:true}).waitFor();assert.equal(await p.getByLabel('Cấu hình AI phụ đề').inputValue(),'prof-soniox');
    assert(!await p.evaluate(()=>window.qa.calls.some(c=>c.cmd==='start_caption')),'Applying a preset must not start audio upload');
  });
  await check("Legacy presets inherit existing subtitle style without losing template choices",{profiles:mockProfiles},async p=>{
    await p.evaluate(()=>{
      const key='innote.workflow-presets.v1',data=JSON.parse(localStorage.getItem(key));for(const preset of data.presets)delete preset.captionStyle;localStorage.setItem(key,JSON.stringify(data));
      localStorage.setItem('innote.caption.style',JSON.stringify({fontSize:28,align:'left',color:'#123456',backgroundColor:'#654321',showOriginal:true,transparent:true}));
    });
    const before=await p.evaluate(()=>JSON.parse(localStorage.getItem('innote.workflow-presets.v1')).presets);
    await p.reload();await p.getByRole('button',{name:'Mẫu cấu hình',exact:true}).click();
    const after=await p.evaluate(()=>JSON.parse(localStorage.getItem('innote.workflow-presets.v1')).presets);
    assert.deepEqual(after.map(p=>p.id),before.map(p=>p.id));assert.deepEqual(after.map(p=>p.tasks),before.map(p=>p.tasks));assert(after.every(p=>p.captionStyle.fontSize===28&&p.captionStyle.color==='#123456'));
  });
  await check("Preset subtitle styles save per mode, cancel safely and apply with selected template",{profiles:mockProfiles},async p=>{
    const key='innote.caption.style';const initial=await p.evaluate(key=>localStorage.getItem(key),key);
    await p.getByRole('button',{name:'Mẫu cấu hình',exact:true}).click();
    assert.equal(await p.locator('#caption-settings').count(),0,'No detached global caption settings');
    await p.getByRole('button',{name:'Tạo mẫu',exact:true}).click();await p.getByLabel('Tên mẫu',{exact:true}).fill('Họp chữ 30');
    await p.getByLabel('Cỡ chữ phụ đề').fill('30');await p.getByLabel('Cỡ chữ phụ đề').dispatchEvent('input');
    await p.getByLabel('Màu chữ phụ đề').fill('#ff0000');await p.getByLabel('Màu chữ phụ đề').dispatchEvent('input');
    assert.equal(await p.evaluate(key=>localStorage.getItem(key),key),initial,'Draft changes must not affect live styles');
    await p.getByRole('button',{name:'Lưu mẫu',exact:true}).click();
    const meeting=p.locator('.preset-row').filter({hasText:'Họp chữ 30'});await meeting.getByRole('button',{name:'Sửa',exact:true}).click();
    await p.getByLabel('Cỡ chữ phụ đề').fill('50');await p.getByLabel('Cỡ chữ phụ đề').dispatchEvent('input');await p.getByRole('button',{name:'Hủy chỉnh sửa',exact:true}).click();
    const templates=await p.evaluate(()=>JSON.parse(localStorage.getItem('innote.workflow-presets.v1')).presets);assert.equal(templates.find(x=>x.name==='Họp chữ 30').captionStyle.fontSize,30);
    await meeting.getByRole('button',{name:'Sử dụng',exact:true}).click();assert.equal(await p.evaluate(key=>JSON.parse(localStorage.getItem(key)).fontSize,key),30);
    await p.getByRole('button',{name:'Mẫu cấu hình',exact:true}).click();await p.getByRole('tab',{name:'Phụ đề',exact:true}).click();await p.getByRole('button',{name:'Tạo mẫu',exact:true}).click();
    await p.getByLabel('Tên mẫu',{exact:true}).fill('Phụ đề chữ 42');await p.getByLabel('Phụ đề trực tiếp',{exact:true}).selectOption('prof-soniox');
    await p.getByLabel('Cỡ chữ phụ đề').fill('42');await p.getByLabel('Cỡ chữ phụ đề').dispatchEvent('input');
    await p.getByLabel('Song ngữ',{exact:true}).check();await p.getByLabel('Trong suốt',{exact:true}).check();await p.getByLabel('Căn chữ phụ đề').selectOption('left');
    await p.screenshot({path:join(output,'subtitle-style-in-template.png')});
    await p.getByRole('button',{name:'Lưu mẫu',exact:true}).click();await p.reload();await p.getByRole('button',{name:'Mẫu cấu hình',exact:true}).click();await p.getByRole('tab',{name:'Phụ đề',exact:true}).click();
    await p.locator('.preset-row').filter({hasText:'Phụ đề chữ 42'}).getByRole('button',{name:'Sử dụng',exact:true}).click();
    const style=await p.evaluate(key=>JSON.parse(localStorage.getItem(key)),key);assert.equal(style.fontSize,42);assert.equal(style.align,'left');assert(style.showOriginal&&style.transparent);
    await p.getByRole('button',{name:'Mẫu cấu hình',exact:true}).click();await p.getByRole('tab',{name:'Cuộc họp',exact:true}).click();await p.locator('.preset-row').filter({hasText:'Họp chữ 30'}).getByRole('button',{name:'Sử dụng',exact:true}).click();
    assert.equal(await p.evaluate(key=>JSON.parse(localStorage.getItem(key)).fontSize,key),30);
    assert(!await p.evaluate(()=>window.qa.calls.some(c=>['start','stop','start_caption','stop_caption'].includes(c.cmd))));
  });
  await check("Preset sidebar opens its catalog at the top with separate meeting and caption lists",{profiles:mockProfiles},async p=>{
    await p.getByRole('button',{name:'Mẫu cấu hình',exact:true}).click();
    await p.locator('#local-model-settings').scrollIntoViewIfNeeded();
    await p.getByRole('button',{name:'Cài đặt',exact:true}).click();
    await p.getByRole('heading',{name:'Ngôn ngữ giao diện',exact:true}).waitFor();
    await p.getByRole('heading',{name:'Cập nhật',exact:true}).waitFor();
    await p.getByRole('heading',{name:'Lưu trữ cuộc họp',exact:true}).waitFor();
    await p.getByRole('button',{name:'Mẫu cấu hình',exact:true}).click();
    await p.waitForFunction(()=>document.querySelector('.preset-page')?.scrollTop===0);
    assert.equal(await p.getByRole('tab',{name:'Cuộc họp',exact:true}).getAttribute('aria-selected'),'true');
    await p.locator('.preset-row').filter({hasText:'Cuộc họp hiện tại'}).waitFor();
    await p.getByRole('tab',{name:'Phụ đề',exact:true}).click();
    await p.locator('.preset-row').filter({hasText:'Phụ đề hiện tại'}).waitFor();
    assert.equal(await p.locator('.preset-row').filter({hasText:'Cuộc họp hiện tại'}).count(),0);
    assert.equal(await p.getByRole('heading',{name:'Ngôn ngữ giao diện',exact:true}).count(),0);
  });
  for(const viewport of [{width:900,height:640},{width:1180,height:800}])await check(`Sidebar collapse persists and navigation remains usable ${viewport.width}x${viewport.height}`,{profiles:mockProfiles,viewport},async p=>{
    const side=p.getByRole('complementary');
    const expanded=await side.boundingBox();
    await p.screenshot({path:join(output,`sidebar-expanded-${viewport.width}.png`)});
    const toggle=p.getByRole('button',{name:'Thu gọn thanh bên',exact:true});
    await toggle.focus();await p.keyboard.press('Enter');
    await p.waitForFunction(()=>document.querySelector('.sidebar')?.getBoundingClientRect().width<90);
    assert.equal(await p.getByRole('button',{name:'Mở rộng thanh bên',exact:true}).getAttribute('aria-expanded'),'false');
    assert(await p.getByRole('button',{name:'Mở rộng thanh bên',exact:true}).evaluate(el=>el===document.activeElement));
    assert((await side.boundingBox()).width<expanded.width/2);
    for(const name of ['Cuộc họp mới','Chỉ phụ đề','Mẫu cấu hình','Danh sách cuộc họp','Cài đặt']){
      const button=side.getByRole('button',{name,exact:true});assert(await button.isVisible());assert(await button.getAttribute('title'));
    }
    await side.getByRole('button',{name:'Mẫu cấu hình',exact:true}).click();await p.getByRole('heading',{name:'Mẫu cấu hình',exact:true}).waitFor();
    await p.screenshot({path:join(output,`sidebar-collapsed-${viewport.width}.png`)});
    assert(await side.evaluate(el=>el.scrollWidth<=el.clientWidth),'Collapsed sidebar has no horizontal overflow');
    await p.reload();await p.getByRole('button',{name:'Mở rộng thanh bên',exact:true}).waitFor();
    await side.getByRole('button',{name:'Danh sách cuộc họp',exact:true}).click();await p.getByRole('dialog',{name:'Cuộc họp của bạn'}).waitFor();
    await p.keyboard.press('Escape');await p.getByRole('button',{name:'Mở rộng thanh bên',exact:true}).click();
    await p.waitForFunction(()=>document.querySelector('.sidebar')?.getBoundingClientRect().width>200);
    assert.equal(await p.evaluate(()=>localStorage.getItem('innote.sidebar.collapsed')),'false');
    assert(!await p.evaluate(()=>window.qa.calls.some(c=>['start','stop','start_caption','stop_caption'].includes(c.cmd))));
  });
  await check("General settings stay separate from inline caption appearance and preset drafts",{profiles:mockProfiles},async p=>{
    await p.getByRole('button',{name:'Cài đặt',exact:true}).click();assert.equal(await p.locator('#caption-settings,#standalone-caption-style').count(),0);
    const presets=await p.evaluate(()=>localStorage.getItem('innote.workflow-presets.v1'));
    await p.getByRole('button',{name:'Chỉ phụ đề',exact:true}).click();const panel=p.locator('#standalone-caption-style');assert(await panel.getByLabel('Cỡ chữ phụ đề').isVisible());assert.equal(await p.locator('.preset-page').count(),0);
    await panel.getByLabel('Cỡ chữ phụ đề').fill('30');await panel.getByLabel('Cỡ chữ phụ đề').dispatchEvent('input');assert.equal(await p.evaluate(()=>localStorage.getItem('innote.workflow-presets.v1')),presets);
    await p.getByRole('button',{name:'Mẫu cấu hình',exact:true}).click();await p.getByRole('tab',{name:'Phụ đề',exact:true}).click();await p.getByRole('button',{name:'Tạo mẫu',exact:true}).click();
    await p.getByLabel('Cỡ chữ phụ đề').fill('42');await p.getByLabel('Cỡ chữ phụ đề').dispatchEvent('input');assert.equal(await p.evaluate(()=>JSON.parse(localStorage.getItem('innote.caption.style')).fontSize),30,'Template drafts do not edit the live appearance');
    await p.getByRole('button',{name:'Hủy chỉnh sửa',exact:true}).click();assert.equal(await p.locator('#caption-settings').count(),0);
    await p.getByRole('button',{name:'Cài đặt',exact:true}).click();await p.getByRole('button',{name:'Kiểm tra cập nhật',exact:true}).waitFor();assert(!await p.evaluate(()=>window.qa.calls.some(c=>['start','stop','start_caption','stop_caption'].includes(c.cmd))));
  });
  await check("Workflow presets migrate, edit independently, apply meeting models and persist",{profiles:mockProfiles},async p=>{
    await p.getByRole('button',{name:'Cài đặt',exact:true}).click();
    assert.equal(await p.getByText('Gán cấu hình cho tác vụ',{exact:true}).count(),0);
    await p.getByRole('button',{name:'Mẫu cấu hình',exact:true}).click();
    await p.getByRole('button',{name:'Tạo mẫu',exact:true}).click();
    await p.getByLabel('Tên mẫu',{exact:true}).fill('Họp tiếng Anh');
    await p.getByLabel('Ngôn ngữ gốc của mẫu').selectOption('en');
    await p.getByLabel('Model chép lời local của mẫu').selectOption('moonshine-en');
    await p.getByLabel('Mức xử lý của mẫu').selectOption('fast');
    await p.getByRole('button',{name:'Lưu mẫu',exact:true}).click();
    const row=p.locator('.preset-row').filter({hasText:'Họp tiếng Anh'});await row.waitFor();
    const saved=await p.evaluate(()=>localStorage.getItem('innote.workflow-presets.v1'));
    await row.getByRole('button',{name:'Sửa',exact:true}).click();await p.getByLabel('Tên mẫu',{exact:true}).fill('Không được lưu');
    await p.getByRole('button',{name:'Hủy chỉnh sửa',exact:true}).click();assert.equal(await p.evaluate(()=>localStorage.getItem('innote.workflow-presets.v1')),saved);
    await p.reload();await p.getByRole('button',{name:'Mẫu cấu hình',exact:true}).click();
    await p.locator('.preset-row').filter({hasText:'Họp tiếng Anh'}).getByRole('button',{name:'Sử dụng',exact:true}).click();
    assert.equal(await p.getByLabel('Ngôn ngữ',{exact:true}).inputValue(),'en');
    await p.getByLabel('Ngôn ngữ',{exact:true}).selectOption('vi');
    assert.equal(await p.getByLabel('Mẫu cấu hình cuộc họp').inputValue(),'','Editing a session switches to Custom and clears pending selection');
    assert(await p.getByRole('button',{name:'Bắt đầu ghi',exact:true}).isEnabled());
    assert.equal(await p.evaluate(()=>localStorage.getItem('innote.workflow-presets.v1')),saved,'Session edits cannot change saved presets');
    const id=JSON.parse(saved).presets.find(p=>p.name==='Họp tiếng Anh').id;
    await p.getByLabel('Mẫu cấu hình cuộc họp').selectOption(id);await p.getByRole('button',{name:'Áp mẫu',exact:true}).click();
    await p.getByRole('button',{name:'Bắt đầu ghi',exact:true}).click();
    const options=await p.evaluate(()=>window.qa.calls.find(c=>c.cmd==='start').args.options);
    assert.equal(options.language,'en');assert.equal(options.model,'moonshine-en');assert.equal(options.speechQuality,'fast');
    await p.getByRole('button',{name:'Mẫu cấu hình',exact:true}).click();
    assert(await p.locator('.preset-row').filter({hasText:'Họp tiếng Anh'}).getByRole('button',{name:'Sử dụng',exact:true}).isDisabled());
    await p.locator('.preset-row').filter({hasText:'Họp tiếng Anh'}).getByRole('button',{name:'Xóa',exact:true}).click();
    await p.getByRole('button',{name:'Xóa mẫu',exact:true}).click();
    assert.equal(await p.locator('.preset-row').filter({hasText:'Họp tiếng Anh'}).count(),0);
    assert(!await p.evaluate(()=>window.qa.calls.some(c=>c.cmd==='stop'||c.cmd==='delete_ai_profile')));
  });
  for(const viewport of [{width:900,height:640},{width:1180,height:800}])await check(`Workflow presets editor layout ${viewport.width}x${viewport.height}`,{profiles:mockProfiles,viewport},async p=>{
    await p.getByRole('button',{name:'Mẫu cấu hình',exact:true}).click();
    await p.screenshot({path:join(output,`preset-library-${viewport.width}.png`)});
    await p.getByRole('button',{name:'Tạo mẫu',exact:true}).click();
    await p.getByLabel('Tên mẫu',{exact:true}).fill('Họp có phụ đề tiếng Việt');
    if(viewport.width===1180){await p.getByLabel('Ngôn ngữ gốc của mẫu').selectOption('auto');await p.getByLabel('Mức xử lý của mẫu').selectOption('accurate');}
    await p.getByLabel('Dịch sang trong mẫu').check();
    await p.getByLabel('Phụ đề trực tiếp',{exact:true}).selectOption('prof-soniox');
    await p.screenshot({path:join(output,`preset-editor-${viewport.width}.png`)});
    assert(await p.locator('.preset-page').evaluate(el=>el.scrollWidth<=el.clientWidth),'Editor must not overflow horizontally');
    await p.getByRole('button',{name:'Lưu mẫu',exact:true}).click();
    await p.locator('.preset-row').filter({hasText:'Họp có phụ đề tiếng Việt'}).getByRole('button',{name:'Sử dụng',exact:true}).click();
    assert.equal(await p.getByLabel('Cách chép lời',{exact:true}).inputValue(),'soniox');
    await p.screenshot({path:join(output,`preset-launch-${viewport.width}.png`)});
    await p.getByRole('button',{name:'Bắt đầu ghi',exact:true}).click();
    const options=await p.evaluate(()=>window.qa.calls.find(c=>c.cmd==='start').args.options);
    assert.equal(options.provider,'soniox');assert.equal(options.profileId,'prof-soniox');assert.equal(options.translationLanguage,'vi');
    if(viewport.width===1180){assert.equal(options.language,'multi');assert.equal(options.speechQuality,'accurate');}
  });
  await check("Workflow presets can be edited during a meeting without changing its engine",{profiles:mockProfiles,active:true},async p=>{
    const before=await p.evaluate(()=>structuredClone(window.qa.snapshot));
    await p.getByRole('button',{name:'Mẫu cấu hình',exact:true}).click();
    await p.getByRole('button',{name:'Tạo mẫu',exact:true}).click();
    await p.getByLabel('Tên mẫu',{exact:true}).fill('Cho lần họp sau');
    await p.getByRole('button',{name:'Lưu mẫu',exact:true}).click();
    await p.locator('.preset-row').filter({hasText:'Cho lần họp sau'}).waitFor();
    assert.deepEqual(await p.evaluate(()=>window.qa.snapshot),before);
    assert(!await p.evaluate(()=>window.qa.calls.some(c=>['start','stop','start_caption','stop_caption','set_meeting_translation'].includes(c.cmd))));
    assert(await p.locator('.preset-row').filter({hasText:'Cho lần họp sau'}).getByRole('button',{name:'Sử dụng',exact:true}).isDisabled());
  });
  await check("Workflow presets reject deleted profiles without switching providers",{profiles:mockProfiles},async p=>{
    await p.evaluate(()=>localStorage.setItem('innote.workflow-presets.v1',JSON.stringify({version:1,presets:[{id:'missing',name:'API đã xóa',mode:'captionsonly',tasks:{caption:{profileId:'deleted-profile',model:'stt-rt-v6',targetLang:'vi',translate:true}},source:'en',quality:'balanced',translate:true,target:'vi'}]})));
    await p.reload();await p.getByRole('button',{name:'Mẫu cấu hình',exact:true}).click();await p.getByRole('tab',{name:'Phụ đề',exact:true}).click();
    await p.locator('.preset-row').getByRole('button',{name:'Sử dụng',exact:true}).click();
    await p.getByText(/Cấu hình API cho .* không còn tồn tại/).waitFor();
    assert(!await p.evaluate(()=>window.qa.calls.some(c=>c.cmd==='start'||c.cmd==='start_caption')));
  });
  await check("Workflow presets captions keep named provider model and standalone options",{profiles:[...mockProfiles,{id:'prof-groq',name:'Groq Free',provider:'groq',base_url:'',has_key:true,key_tail:'mock',created:'2026-11-03'}]},async p=>{
    await p.getByRole('button',{name:'Mẫu cấu hình',exact:true}).click();
    await p.getByRole('tab',{name:'Phụ đề',exact:true}).click();await p.getByRole('button',{name:'Tạo mẫu',exact:true}).click();
    await p.getByLabel('Tên mẫu',{exact:true}).fill('Luyện nghe Groq');
    await p.getByLabel('Ngôn ngữ gốc của mẫu').selectOption('en');
    await p.getByLabel('Dịch sang trong mẫu').uncheck();
    await p.locator('.task-assign-grid').first().locator('select').first().selectOption('prof-groq');
    await p.getByLabel('Model phụ đề',{exact:true}).selectOption('whisper-large-v3-turbo');
    await p.getByRole('button',{name:'Lưu mẫu',exact:true}).click();
    await p.locator('.preset-row').filter({hasText:'Luyện nghe Groq'}).getByRole('button',{name:'Sử dụng',exact:true}).click();
    assert.equal(await p.getByLabel('Cấu hình AI phụ đề').inputValue(),'prof-groq');
    assert.equal(await p.getByLabel('Model phụ đề',{exact:true}).inputValue(),'whisper-large-v3-turbo');
    await p.getByRole('button',{name:'Bắt đầu phụ đề',exact:true}).click();
    const o=await p.evaluate(()=>window.qa.calls.find(c=>c.cmd==='start_caption').args.options);assert.equal(o.model,'whisper-large-v3-turbo');assert.equal(o.profileId,'prof-groq');assert.equal(o.translate,false);assert.equal(o.sourceLanguage,'en');
  });
  await check("Recording transcript flows as one paragraph until ending, preserving segment timestamps", {active:true},async p=>{
    await openMeeting(p,'Phiên đang ghi');
    const stored=[{id:'flow-one',start:0,end:2,text:'Xin chào mọi người.',edited:false},{id:'flow-two',start:2,end:4,text:'Ngân sách là 15 triệu.',edited:false}];
    await p.evaluate(({id,stored})=>{const s=window.qa.snapshot.sessions.find(s=>s.id===id);s.segments=stored;s.processed=4;s.duration=5;},{id:ids[2],stored});
    await p.locator('.recording-transcript').getByText('Xin chào mọi người. Ngân sách là 15 triệu.',{exact:true}).waitFor();
    assert.equal(await p.locator('.transcript .segment').count(),0);
    assert.equal(await p.locator('.transcript .timestamp').count(),0);
    await p.evaluate(async id=>{const{emit}=await import('/node_modules/@tauri-apps/api/event.js');await emit('live-transcript',{id,update:{text:'Không phải 50 triệu.',start:4,end:6,final_text:false,words:[]}});},ids[2]);
    await p.locator('[data-live-caption]').getByText('Không phải 50 triệu.',{exact:true}).waitFor();
    assert.equal(await p.locator('.recording-transcript p').count(),1);
    await p.evaluate(id=>{const s=window.qa.snapshot.sessions.find(s=>s.id===id);s.translations=[
      {id:'translation-one',source_id:'flow-one',start:0,end:2,original:'Xin chào mọi người.',text:'Hello everyone.',language:'en',provider:'soniox'},
      {id:'translation-two',source_id:'flow-two',start:2,end:4,original:'Ngân sách là 15 triệu.',text:'The budget is 15 million.',language:'en',provider:'soniox'}
    ];},ids[2]);
    const view=p.getByLabel('Nội dung hiển thị',{exact:true});await view.selectOption('bilingual');
    assert.equal(await p.locator('.translation-track .saved-translation').count(),1);
    assert.equal(await p.locator('.translation-track .saved-translation').innerText(),'Hello everyone. The budget is 15 million.');
    await p.screenshot({path:join(output,'recording-continuous-bilingual.png')});
    await view.selectOption('translation');
    assert.equal(await p.locator('.recording-transcript p').innerText(),'Hello everyone. The budget is 15 million.');
    assert.equal(await p.locator('[data-live-caption]').count(),0,'Original partial must not leak into translation-only view');
    await view.selectOption('original');
    await p.getByRole('button',{name:'Tạm dừng',exact:true}).click();
    await p.getByRole('button',{name:'Tiếp tục',exact:true}).waitFor();
    assert.equal(await p.locator('.recording-transcript').count(),1);
    await p.screenshot({path:join(output,'recording-continuous-paragraph.png')});
    await p.getByRole('button',{name:'Kết thúc',exact:true}).click();
    await p.getByRole('button',{name:'Kết thúc và lưu',exact:true}).click();
    await p.getByRole('button',{name:'Sửa đoạn 00:00',exact:true}).waitFor();
    assert.equal(await p.locator('.recording-transcript').count(),0);
    assert.equal(await p.locator('.transcript .segment').count(),2);
    assert.deepEqual(await p.evaluate(id=>window.qa.snapshot.sessions.find(s=>s.id===id).segments,ids[2]),stored);
    await p.screenshot({path:join(output,'ended-segmented-transcript.png')});
  });
  await check("Microphone can be muted during a meeting without stopping capture",{active:true},async p=>{
    await openMeeting(p,'Phiên đang ghi');
    const mute=p.getByRole('button',{name:'Tắt mic',exact:true});
    assert(await mute.isVisible());
    assert(await mute.getAttribute('aria-pressed')==='false');
    await mute.click();
    await p.getByText('Micro đang tắt',{exact:true}).waitFor();
    assert.equal(await p.evaluate(()=>window.qa.snapshot.mic_muted),true);
    const call=await p.evaluate(()=>window.qa.calls.filter(c=>c.cmd==='set_mic_muted').at(-1));
    assert.equal(call.args.muted,true);
    assert(!await p.evaluate(()=>window.qa.calls.some(c=>c.cmd==='stop'||c.cmd==='pause')),'Muting must not stop or pause the meeting');
    assert(await p.getByRole('button',{name:'Bật mic',exact:true}).isVisible());
    await p.screenshot({path:join(output,'meeting-mic-muted.png')});
    await p.getByRole('button',{name:'Bật mic',exact:true}).click();
    await p.getByText('Micro đang tắt',{exact:true}).waitFor({state:'detached'});
    assert.equal(await p.evaluate(()=>window.qa.snapshot.mic_muted),false);
    const again=await p.evaluate(()=>window.qa.calls.filter(c=>c.cmd==='set_mic_muted').at(-1));
    assert.equal(again.args.muted,false);
  });
  await check("Microphone can be turned off before a meeting starts",{},async p=>{
    const mic=p.getByLabel('Micro của bạn',{exact:true});
    assert.equal(await mic.inputValue(),'mic');
    await p.getByRole('button',{name:'Tắt mic',exact:true}).click();
    assert.equal(await mic.inputValue(),'');
    await p.getByRole('button',{name:'Bật mic',exact:true}).click();
    assert.equal(await mic.inputValue(),'mic');
    await p.getByRole('button',{name:'Tắt mic',exact:true}).click();
    assert.equal(await mic.inputValue(),'');
    assert(await p.getByRole('button',{name:'Bắt đầu ghi',exact:true}).isEnabled());
    await p.getByRole('button',{name:'Bắt đầu ghi',exact:true}).click();
    const options=await p.evaluate(()=>window.qa.calls.find(c=>c.cmd==='start').args.options);
    assert.equal(options.microphone,null);
    assert.equal(options.system,'speaker');
  });
  await check("Recording header keeps the mic mute control visible at 900x640",{active:true,viewport:{width:900,height:640}},async p=>{
    await openMeeting(p,'Phiên đang ghi');
    for (const name of ['Tắt mic','Tạm dừng','Kết thúc']) {
      const box=await p.getByRole('button',{name,exact:true}).boundingBox();
      assert(box && box.x>=0 && box.x+box.width<=900 && box.y+box.height<=640,`${name} must stay inside 900x640`);
    }
    assert(await p.evaluate(()=>document.documentElement.scrollWidth<=innerWidth));
    await p.screenshot({path:join(output,'meeting-mic-mute-900x640.png')});
  });
  await check("API profile dialog contains keyboard focus and Escape restores its trigger", {viewport:{width:900,height:640}},async p=>{
    await p.getByRole('button',{name:'Cài đặt',exact:true}).click();
    const trigger=p.getByRole('button',{name:'Thêm cấu hình',exact:true});await trigger.click();
    const dialog=p.getByRole('dialog',{name:'Thêm cấu hình',exact:true});
    await dialog.getByLabel('Tên cấu hình').waitFor();
    assert(await dialog.getByLabel('Tên cấu hình').evaluate(e=>e===document.activeElement));
    for(let i=0;i<12;i++){
      await p.keyboard.press('Tab');
      assert(await dialog.evaluate(e=>e.contains(document.activeElement)),'Tab escaped the modal');
    }
    for(let i=0;i<12;i++){
      await p.keyboard.press('Shift+Tab');
      assert(await dialog.evaluate(e=>e.contains(document.activeElement)),'Shift+Tab escaped the modal');
    }
    await dialog.getByLabel('Nhà cung cấp',{exact:true}).selectOption('groq');
    await dialog.screenshot({path:join(output,'groq-dialog-900x640.png')});
    await p.keyboard.press('Escape');await dialog.waitFor({state:'detached'});
    assert(await trigger.evaluate(e=>e===document.activeElement));
  });
  await check("Cloud caption disclosure restores provider on reload and tracks fallback and stop", {
    profiles:mockProfiles,captionStatus:{provider:"groq",state:"active",sending:false,message:"Đang nghe đoạn tiếp theo"}
  },async p=>{
    const status=p.locator('.topbar .local-label');
    await p.getByRole('button',{name:'Chỉ phụ đề',exact:true}).click();
    await status.filter({hasText:'Groq'}).waitFor();
    assert(!(await status.innerText()).includes('Xử lý trên máy'));
    await p.reload();await status.filter({hasText:'Groq'}).waitFor();
    const emitStatus=payload=>p.evaluate(async payload=>{const{emit}=await import('/node_modules/@tauri-apps/api/event.js');await emit('caption-status',payload);},payload);
    await emitStatus({provider:'soniox',state:'fallback',sending:true,message:'Đang chuyển dự phòng đã chọn'});
    await status.filter({hasText:'Soniox'}).waitFor();
    await emitStatus({provider:'local',state:'active',sending:false,message:'Đang chép local'});
    await status.filter({hasText:'Xử lý trên máy'}).waitFor();
    await emitStatus({provider:'groq',state:'idle',sending:false,message:'Đã dừng'});
    assert(!(await status.innerText()).includes('Groq'));
  });
  await check("Late caption startup snapshot cannot overwrite a live fallback", {delayCaptionSnapshot:1200},async p=>{
    await p.evaluate(async()=>{const{emit}=await import('/node_modules/@tauri-apps/api/event.js');await emit('caption-status',{provider:'soniox',state:'active',sending:true,message:'Đang nghe'});});
    await p.waitForTimeout(1400);
    assert((await p.locator('.topbar .local-label').innerText()).includes('Soniox'));
  });
  await check("New caption text during startup preserves both the live cue and restored provider", {
    profiles:mockProfiles,delayCaptionSnapshot:1200,captionStatus:{provider:"groq",state:"active",sending:false,message:"Đang nghe"}
  },async p=>{
    await p.getByRole('button',{name:'Chỉ phụ đề',exact:true}).click();
    await p.evaluate(async()=>{const{emit}=await import('/node_modules/@tauri-apps/api/event.js');await emit('caption-text',{text:'New confirmed cue',original:'',final_:true,translating:false});});
    await p.waitForTimeout(1400);
    assert((await p.locator('.topbar .local-label').innerText()).includes('Groq'));
    assert.equal(await p.locator('.caption-preview-text').innerText(),'New confirmed cue');
  });
  await check("First launch asks for an interface language and switches to English", {noUiLocale:true, viewport:{width:900,height:640}}, async p=>{
    await p.getByRole("heading",{name:"Chọn ngôn ngữ giao diện",exact:true}).waitFor();
    for (const name of ["Tiếng Việt","English","한국어","中文（简体）"]) {
      assert(await p.getByRole("button",{name:new RegExp(name)}).isVisible(), name);
    }
    await p.screenshot({path:join(output,"language-gate-900x640.png")});
    await p.getByRole("button",{name:/English/}).click();
    await p.getByRole("button",{name:"New meeting",exact:true}).waitFor();
    assert.equal(await p.evaluate(()=>localStorage.getItem("may.ui_locale")),"en");
    assert(await p.getByRole("button",{name:"Meeting list",exact:true}).isVisible());
    await p.screenshot({path:join(output,"language-english-900x640.png")});
  });
  await check("Settings switches the interface between Korean and Chinese", {viewport:{width:900,height:640}}, async p=>{
    await p.getByRole("button",{name:"Cài đặt",exact:true}).click();
    const localeSelect = p.locator(".settings-page select").first();
    await localeSelect.selectOption("ko");
    await p.getByRole("button",{name:"새 회의",exact:true}).waitFor();
    assert.equal(await localeSelect.inputValue(),'ko');
    await p.screenshot({path:join(output,"language-korean-900x640.png")});
    await localeSelect.selectOption("zh");
    await p.getByRole("button",{name:"新建会议",exact:true}).waitFor();
    assert.equal(await localeSelect.inputValue(),'zh');
    await p.screenshot({path:join(output,"language-chinese-900x640.png")});
  });
  await check("Updater banner offers a blocked-safe update action", {updateAvailable:true, viewport:{width:900,height:640}}, async p=>{
    await p.getByText("Đã có bản Innote 0.6.3",{exact:true}).waitFor({timeout:10000});
    assert(await p.getByRole("button",{name:"Cập nhật ngay",exact:true}).isVisible());
    await p.screenshot({path:join(output,"update-banner-900x640.png")});
    await p.getByRole("button",{name:"Để sau",exact:true}).click();
    assert.equal(await p.getByText("Đã có bản Innote 0.6.3",{exact:true}).count(),0);
  });
  await check("Author card exposes Zalo LinkedIn and website links", {viewport:{width:900,height:640}}, async p=>{
    assert(await p.getByText("Alex Linh",{exact:true}).isVisible());
    assert(await p.getByText("Tác giả",{exact:true}).isVisible());
    for (const [name, kind] of [
      ["Liên hệ Zalo Alex Linh", "zalo"],
      ["Liên hệ LinkedIn Alex Linh", "linkedin"],
      ["Mở website Alex Linh", "website"],
    ]) {
      await p.getByRole("button", { name, exact: true }).click();
      await p.waitForFunction(
        (k) => window.qa.calls.some((c) => c.cmd === "open_author_link" && c.args.kind === k),
        kind,
      );
    }
    await p
      .getByRole("button", { name: "Báo lỗi & Yêu cầu tính năng", exact: true })
      .click();
    await p.waitForFunction(
      (k) => window.qa.calls.some((c) => c.cmd === "open_author_link" && c.args.kind === k),
      "feedback",
    );
    await p.screenshot({ path: join(output, "author-900x640.png") });
  });
  await check("Starting capture locks language provider and transcription choices", {slowStart:true}, async p=>{
    await p.getByRole('button',{name:'Bắt đầu ghi',exact:true}).click();
    for (const label of ['Ngôn ngữ','Cách chép lời'])
      assert(await p.getByLabel(label,{exact:true}).isDisabled(),label);
    assert.equal(await p.getByLabel('Chất lượng',{exact:true}).count(),0);
    assert.equal(await p.getByLabel('Model',{exact:true}).count(),0);
    const options=await p.evaluate(()=>window.qa.calls.find(c=>c.cmd==='start').args.options);
    assert.equal(options.model,'moonshine-vi');
    assert(await p.getByRole('checkbox',{name:/Chép lời khi/}).isDisabled());
    await p.getByRole('button',{name:'Tạm dừng',exact:true}).waitFor();
  });
  await check("Mixed meeting keeps VI EN KO ZH text and commits editable turns without translation", {}, async p=>{
    await p.getByLabel('Ngôn ngữ',{exact:true}).selectOption('multi');
    assert(await p.getByLabel('Cách chép lời',{exact:true}).isEnabled());
    await p.getByRole('button',{name:'Bắt đầu ghi',exact:true}).click();
    const options=await p.evaluate(()=>window.qa.calls.find(c=>c.cmd==='start').args.options);
    assert.equal(options.language,'multi');assert.equal(options.model,'nemotron-3.5-q8');assert.equal(options.provider,'local');
    await p.getByRole('button',{name:'Kết thúc',exact:true}).waitFor();
    const text='Xin chào. Hello team. 안녕하세요. 大家好。';
    await p.evaluate(async ({id,text})=>{const {emit}=await import('/node_modules/@tauri-apps/api/event.js');await emit('live-transcript',{id,update:{text,start:0,end:5,final_text:false,words:[]}});},{id:ids[2],text});
    await p.locator('[data-live-caption]').getByText(text,{exact:true}).waitFor();
    await p.evaluate(({id,text})=>{const s=window.qa.snapshot.sessions.find(s=>s.id===id);s.duration=5;s.processed=5;s.segments=[{id:'multi-final',start:0,end:5,text,edited:false}];},{id:ids[2],text});
    await p.waitForTimeout(1500);
    await p.locator('.recording-transcript').getByText(text,{exact:true}).waitFor();
    assert.equal(await p.getByRole('button',{name:'Sửa đoạn 00:00',exact:true}).count(),0);
    assert((await p.locator('main').innerText()).includes(text));
    await p.evaluate(({id})=>{window.qa.snapshot.sessions.find(s=>s.id===id).duration=120;},{id:ids[2]});
    await p.waitForTimeout(1500);
    assert(!(await p.locator('main').innerText()).includes('chờ hiệu chỉnh'));
    assert((await p.locator('main').innerText()).includes('Âm thanh đang được lưu'));
    await p.screenshot({path:join(output,'multilingual-four-scripts.png')});
  });
  await check("Missing multilingual model blocks recording until its own download, without changing VI quality", {missingMultilingual:true}, async p=>{
    await p.getByLabel('Ngôn ngữ',{exact:true}).selectOption('multi');
    assert(await p.getByRole('button',{name:'Bắt đầu ghi',exact:true}).isDisabled());
    await p.getByRole('button',{name:'Tải · 742 MB',exact:true}).click();
    await p.getByRole('button',{name:'Bắt đầu ghi',exact:true}).waitFor({state:'visible'});
    await p.waitForFunction(()=>window.qa.snapshot.models.find(m=>m.id==='nemotron-3.5-q8').installed);
    assert(!(await p.getByRole('button',{name:'Bắt đầu ghi',exact:true}).isDisabled()));
    await p.getByLabel('Ngôn ngữ',{exact:true}).selectOption('vi');
    assert.equal(await p.getByLabel('Chất lượng',{exact:true}).count(),0);
    assert.equal(await p.getByLabel('Model',{exact:true}).count(),0);
    assert(await p.getByLabel('Cách chép lời',{exact:true}).isEnabled());
    await p.getByRole('button',{name:'Bắt đầu ghi',exact:true}).click();
    const options=await p.evaluate(()=>window.qa.calls.find(c=>c.cmd==='start').args.options);
    assert.equal(options.model,'moonshine-vi');
  });
  await check("Korean Chinese and saved mixed preferences select the matching local model", {preferences:{system:'speaker',microphone:'mic',language:'multi',model:'moonshine-vi',transcribe:true}}, async p=>{
    assert.equal(await p.getByLabel('Ngôn ngữ',{exact:true}).inputValue(),'multi');
    await p.getByText('Thông tin xử lý âm thanh',{exact:true}).click();
    for(const lang of ['ko','zh']) {await p.getByLabel('Ngôn ngữ',{exact:true}).selectOption(lang);assert.equal(await p.getByLabel('Model',{exact:true}).count(),0);assert((await p.locator('main').innerText()).includes(lang==='ko'?'Chép tiếng Hàn':'Chép tiếng Trung'));}
    await p.getByRole('button',{name:'Bắt đầu ghi',exact:true}).click();
    const o=await p.evaluate(()=>window.qa.calls.find(c=>c.cmd==='start').args.options);assert.equal(o.language,'zh');assert.equal(o.model,'nemotron-3.5-q8');
  });
  await check("Streaming events display immediately, survive snapshots, isolate sessions and retire refined words", {active:true, viewport:{width:900,height:640}}, async p => {
    await openMeeting(p, "Phiên đang ghi");
    const send = payload => p.evaluate(async payload => {
      const {emit} = await import('/node_modules/@tauri-apps/api/event.js');
      await emit('live-transcript',payload);
    },payload);
    const word = (id,start,end,text) => ({id,start,end,text,edited:false});
    await send({id:ids[2],update:{text:"Đang nói trực tiếp",start:0,end:2,final_text:false,words:[]}});
    await p.getByRole('article',{name:'Bản chép trực tiếp'}).getByText('Đang nói trực tiếp',{exact:true}).waitFor({timeout:500});
    assert.equal(await p.getByText('Đang nhận dạng',{exact:true}).count(),0);
    assert.equal(await p.getByText('Đang hiệu chỉnh…',{exact:true}).count(),0);
    await p.getByText('Đang chép trực tiếp… Kết thúc phiên để xuất file.',{exact:true}).waitFor();
    assert(await p.getByRole('button',{name:'Tải .md',exact:true}).isDisabled());
    assert.equal(await p.evaluate(()=>window.qa.calls.filter(c=>c.cmd==='report_live_paint').length),0);
    await send({id:ids[2],audit_epoch_unix_ms:Date.now()-2000,update:{text:"Đang nói trực tiếp",start:0,end:2,final_text:false,words:[]}});
    await p.waitForFunction(()=>window.qa.calls.some(c=>c.cmd==='report_live_paint'));
    const audit=await p.evaluate(()=>window.qa.calls.find(c=>c.cmd==='report_live_paint').args);
    assert.equal(audit.id,ids[2]);assert.equal(audit.text,'Đang nói trực tiếp');assert(audit.elapsedMs>=2000);assert(audit.receivedMs<=audit.elapsedMs);assert.equal(audit.audioMs,2000);
    assert(!(await p.getByText('Innote đang chờ lời nói',{exact:true}).count()));
    await send({id:ids[0],update:{text:"Không hiện sai phiên",start:0,end:3,final_text:false,words:[]}});
    assert(!(await p.getByText('Không hiện sai phiên',{exact:true}).count()));
    const words=[word('live-1',0,1,'Đã'),word('live-2',1,2,'nói'),word('live-3',2,3,'tiếp')];
    await send({id:ids[2],update:{text:'Đã nói tiếp',start:0,end:3,final_text:true,words}});
    await p.waitForTimeout(1500);
    assert((await p.getByRole('article',{name:'Bản chép trực tiếp'}).innerText()).includes('Đã nói tiếp'));
    await p.evaluate(({id,words})=>{const s=window.qa.snapshot.sessions.find(s=>s.id===id);s.live=words;s.processed=2;s.segments=[];}, {id:ids[2],words});
    await p.waitForTimeout(1500);
    const panel=p.getByRole('article',{name:'Bản chép trực tiếp'});
    assert((await panel.innerText()).includes('tiếp'));
    assert(!(await panel.innerText()).includes('Đã nói tiếp'));
    await send({id:ids[2],update:{text:'Đã nói tiếp nữa',start:0,end:4,final_text:false,words}});
    assert((await panel.innerText()).includes('tiếp nữa'));
    assert(!(await panel.innerText()).includes('Đã nói'));
    await p.screenshot({path:join(output,'streaming-900x640.png')});
    await p.evaluate(id=>{const s=window.qa.snapshot.sessions.find(s=>s.id===id);s.processed=5;s.live=[];s.status='completed';window.qa.snapshot.active_id=null;},ids[2]);
    await p.waitForTimeout(1500);
    assert.equal(await p.getByRole('article',{name:'Bản chép trực tiếp'}).count(),0);
  });
  await check(
    "Audio check buttons are discoverable without horizontal overflow at 900x640",
    { viewport: { width: 900, height: 640 } },
    async (p) => {
      for (const name of ["Kiểm tra âm thanh", "Kiểm tra micro"]) {
        const box = await p
          .getByRole("button", { name, exact: true })
          .boundingBox();
        assert(
          box &&
            box.x >= 0 &&
            box.x + box.width <= 900 &&
            box.y + box.height <= 640,
        );
      }
      assert(
        await p.evaluate(
          () => document.documentElement.scrollWidth <= innerWidth,
        ),
      );
      await p.screenshot({ path: join(output, "setup-audio-900x640.png") });
    },
  );
  await check(
    "Timestamp starts playback at the matching transcript position",
    {},
    async (p) => {
      await openMeeting(p, "Cuộc họp Alpha");
      await p.getByLabel("Nghe lại cuộc họp").waitFor();
      await p.waitForFunction(
        () =>
          document.querySelector('audio[aria-label="Nghe lại cuộc họp"]')
            .readyState >= 1,
      );
      await p.getByRole("button", { name: "00:01", exact: true }).click();
      await p.waitForFunction(
        () =>
          document.querySelector('audio[aria-label="Nghe lại cuộc họp"]')
            .currentTime >= 1,
      );
      assert(
        await p.getByLabel("Nghe lại cuộc họp").evaluate((a) => !a.paused),
      );
    },
  );
  await check(
    "User can edit Vietnamese, filter transcript, and export all three formats",
    {},
    async (p) => {
      await openMeeting(p, "Cuộc họp Alpha");
      await p.getByRole("button", { name: "Sửa đoạn 00:01" }).click();
      await p
        .getByRole("textbox", { name: "Sửa đoạn chép lời" })
        .fill("Đã chốt ngân sách và thời hạn.");
      await p.getByRole("button", { name: "Lưu sửa", exact: true }).click();
      await p.getByText("Đã chỉnh sửa", { exact: true }).waitFor();
      await p
        .getByRole("textbox", { name: "Tìm trong bản chép lời" })
        .fill("ngân sách");
      assert(
        await p
          .getByText("Đã chốt ngân sách và thời hạn.", { exact: true })
          .isVisible(),
      );
      assert(
        await p.getByRole("button", { name: "Sao chép", exact: true }).isVisible(),
      );
      await p.getByRole("button", { name: "Sao chép", exact: true }).click();
      await p.getByText("Đã sao chép bản chép lời.", { exact: true }).waitFor();
      for (const [format, name] of [["md", "Tải .md"], ["txt", "Tải .txt"], ["srt", "Tải .srt"]]) {
        await p.getByRole("button", { name, exact: true }).click();
        await p.waitForFunction(
          (n) => window.qa.exports.length === n,
          ["md", "txt", "srt"].indexOf(format) + 1,
        );
      }
      assert.deepEqual(
        await p.evaluate(() =>
          window.qa.calls
            .filter((c) => c.cmd === "export_session")
            .map((c) => c.args.format),
        ),
        ["md", "txt", "srt"],
      );
      assert(
        await p.evaluate(() =>
          window.qa.exports.every(
            (s) =>
              s.segments[0].edited && s.segments[0].text.includes("ngân sách"),
          ),
        ),
      );
    },
  );
  await check("Audio correction uses an independent audio model and preserves original playback export and summary source",{profiles:[...mockProfiles,{id:'groq-correct',name:'Groq correction',provider:'groq',has_key:true,base_url:'',key_tail:'test',created:'now'}],taskAssignMeeting:{correction:{profileId:'groq-correct',model:'whisper-large-v3'},summary:{profileId:'prof-openai',model:'gpt-4o-mini'}}},async p=>{
    await openMeeting(p,'Cuộc họp Alpha');await p.getByRole('button',{name:'Hiệu chỉnh',exact:true}).click();
    const d=p.getByRole('dialog',{name:'Hiệu chỉnh từ file ghi âm'});assert.equal(await d.getByLabel('Cấu hình AI hiệu chỉnh').inputValue(),'__local__');assert.equal(await d.getByLabel('Model hiệu chỉnh',{exact:true}).inputValue(),'moonshine-vi');assert(await d.getByRole('button',{name:'Bắt đầu hiệu chỉnh',exact:true}).isEnabled());await d.getByLabel('Cấu hình AI hiệu chỉnh').selectOption('groq-correct');await d.getByLabel('Model hiệu chỉnh',{exact:true}).selectOption('whisper-large-v3');
    assert(!(await d.getByLabel('Cấu hình AI hiệu chỉnh').locator('option').allTextContents()).some(s=>s.includes('OpenAI')));
    assert.equal(await d.getByRole('textbox').count(),0);await d.getByRole('button',{name:'Bắt đầu hiệu chỉnh',exact:true}).click();
    await p.getByRole('tab',{name:'Bản hiệu chỉnh',exact:true}).waitFor();assert.equal(await p.getByRole('tab',{name:'Bản hiệu chỉnh',exact:true}).getAttribute('aria-selected'),'true');
    assert(await p.getByText('Nội dung từ file ghi âm đã được nhận lại.',{exact:true}).isVisible());
    const call=await p.evaluate(()=>window.qa.calls.find(c=>c.cmd==='correct_transcript').args);assert.equal(call.profileId,'groq-correct');assert.equal(call.model,'whisper-large-v3');assert(!('prompt' in call)&&!('text' in call)&&!('apiKey' in call));
    const original=await p.evaluate(()=>window.qa.snapshot.sessions[0].segments);assert.equal(original[0].text,'Nội dung thử tiếng Việt.');
    await p.getByRole('button',{name:'Tải .srt',exact:true}).click();assert.equal(await p.evaluate(()=>window.qa.calls.find(c=>c.cmd==='export_session').args.version),'corrected');
    await p.getByRole('button',{name:'Về Bản gốc',exact:true}).click();assert(await p.getByText('Nội dung thử tiếng Việt.',{exact:true}).isVisible());
    await p.getByRole('button',{name:'Tóm tắt',exact:true}).click();const summary=p.locator('dialog.summary-dialog');await summary.getByLabel('Nguồn tóm tắt').selectOption('corrected');
    assert.equal(await summary.getByLabel('Cấu hình AI tóm tắt').inputValue(),'');assert(await summary.getByRole('button',{name:'Tạo tóm tắt',exact:true}).isDisabled());await summary.getByLabel('Cấu hình AI tóm tắt').selectOption('prof-openai');await summary.getByLabel('Model tóm tắt',{exact:true}).selectOption('gpt-4o-mini');await summary.getByRole('button',{name:'Tạo tóm tắt',exact:true}).click();assert.equal(await p.evaluate(()=>window.qa.calls.find(c=>c.cmd==='summarize_transcript').args.source),'corrected');
  });
  await check("Audio correction legacy preset ignores deleted post-session profiles and chooses backup only inside dialog",{profiles:[...mockProfiles,{id:'groq-correct',name:'Groq file',provider:'groq',has_key:true,base_url:'',key_tail:'test',created:'now'}]},async p=>{
    await p.evaluate(()=>localStorage.setItem('innote.workflow-presets.v1',JSON.stringify({version:1,presets:[{id:'legacy-post',name:'Legacy post tasks',mode:'meeting',source:'vi',quality:'balanced',captions:false,translate:false,target:'vi',transcribe:true,localFallback:false,tasks:{transcription:{profileId:'prof-soniox',model:'stt-rt-v6'},correction:{profileId:'deleted-audio',model:'unavailable',fallback:{profileId:'deleted-backup',model:'old'}},summary:{profileId:'deleted-chat',model:'old'}}}]})));
    await p.reload();await p.getByRole('button',{name:'Mẫu cấu hình',exact:true}).click();await p.locator('.preset-row').filter({hasText:'Legacy post tasks'}).getByRole('button',{name:'Sử dụng',exact:true}).click();
    const c=await p.evaluate(()=>JSON.parse(localStorage.getItem('may.task_assign.meeting')));assert.equal(c.transcription.model,'stt-rt-v6');assert.equal(c.correction,undefined);assert.equal(c.summary,undefined);
    await openMeeting(p,'Cuộc họp Alpha');await p.getByRole('button',{name:'Hiệu chỉnh',exact:true}).click();const d=p.getByRole('dialog',{name:'Hiệu chỉnh từ file ghi âm'});assert.equal(await d.getByLabel('Cấu hình AI hiệu chỉnh').inputValue(),'__local__');assert(!await p.evaluate(()=>window.qa.calls.some(c=>c.cmd==='correct_transcript'||c.cmd==='summarize_transcript')));
    await d.getByLabel('Cấu hình AI hiệu chỉnh').selectOption('groq-correct');await d.getByLabel('Model hiệu chỉnh',{exact:true}).selectOption('whisper-large-v3');await d.locator('summary').click();const backup=d.getByLabel('Model hiệu chỉnh dự phòng');await backup.selectOption(JSON.stringify({profileId:'prof-soniox',model:'stt-async-v6'}));await d.getByLabel('Model hiệu chỉnh',{exact:true}).selectOption('whisper-large-v3-turbo');assert.equal(await backup.inputValue(),'');await backup.selectOption(JSON.stringify({profileId:'prof-soniox',model:'stt-async-v6'}));
    await d.getByRole('button',{name:'Bắt đầu hiệu chỉnh',exact:true}).click();await p.getByRole('tab',{name:'Bản hiệu chỉnh',exact:true}).waitFor();const call=await p.evaluate(()=>window.qa.calls.find(c=>c.cmd==='correct_transcript').args);assert.equal(call.model,'whisper-large-v3-turbo');assert.deepEqual(call.fallback,{profileId:'prof-soniox',model:'stt-async-v6'});
    await p.getByRole('button',{name:'Hiệu chỉnh lại',exact:true}).click();assert.equal(await d.getByLabel('Cấu hình AI hiệu chỉnh').inputValue(),'__local__');assert.equal(await d.getByLabel('Model hiệu chỉnh',{exact:true}).inputValue(),'moonshine-vi');await d.getByRole('button',{name:'Đóng',exact:true}).click();
    await p.getByRole('button',{name:'Mẫu cấu hình',exact:true}).click();await p.locator('.preset-row').filter({hasText:'Legacy post tasks'}).getByRole('button',{name:'Sửa',exact:true}).click();assert.equal(await p.getByLabel('Hiệu chỉnh từ bản thu',{exact:true}).count(),0);assert.equal(await p.getByLabel('Tóm tắt',{exact:true}).count(),0);await p.getByRole('button',{name:'Lưu mẫu',exact:true}).click();const saved=await p.evaluate(()=>JSON.parse(localStorage.getItem('innote.workflow-presets.v1')).presets[0]);assert.deepEqual(Object.keys(saved.tasks),['transcription']);
  });
  await check("Audio correction API failure keeps the previous corrected and original versions",{profiles:mockProfiles,correctionError:'Groq 429: chạm hạn mức',corrected:{text:'keep previous correction',segments:[],provider:'groq',profile_id:'saved',model:'whisper-large-v3',created:'now',audio_sha256:'old'},taskAssignMeeting:{correction:{profileId:'prof-soniox',model:'stt-async-v6'}}},async p=>{
    await openMeeting(p,'Cuộc họp Alpha');await p.getByRole('button',{name:'Hiệu chỉnh lại',exact:true}).click();const d=p.getByRole('dialog',{name:'Hiệu chỉnh từ file ghi âm'});await d.getByLabel('Cấu hình AI hiệu chỉnh').selectOption('prof-soniox');await d.getByLabel('Model hiệu chỉnh',{exact:true}).selectOption('stt-async-v6');await d.getByRole('button',{name:'Hiệu chỉnh lại',exact:true}).click();await d.getByRole('alert').waitFor();assert((await d.getByRole('alert').innerText()).includes('429'));assert.equal(await p.evaluate(()=>window.qa.snapshot.sessions[0].corrected.text),'keep previous correction');assert.equal(await p.evaluate(()=>window.qa.snapshot.sessions[0].segments[0].text),'Nội dung thử tiếng Việt.');
  });
  for(const viewport of [{width:900,height:640},{width:1180,height:800}])await check(`Audio correction dialog spacing and actions remain usable ${viewport.width}`,{viewport,profiles:mockProfiles,taskAssignMeeting:{correction:{profileId:'prof-soniox',model:'stt-async-v6'}}},async p=>{
    await openMeeting(p,'Cuộc họp Alpha');await p.getByRole('button',{name:'Hiệu chỉnh',exact:true}).click();const d=p.getByRole('dialog',{name:'Hiệu chỉnh từ file ghi âm'}),a=await d.getByLabel('Cấu hình AI hiệu chỉnh').boundingBox(),b=await d.getByLabel('Model hiệu chỉnh',{exact:true}).boundingBox(),button=await d.getByRole('button',{name:'Bắt đầu hiệu chỉnh',exact:true}).boundingBox(),box=await d.boundingBox();assert(b.y-a.y-a.height>=20);assert(box.x>=0&&box.x+box.width<=viewport.width+1);assert(button.y>=0&&button.y+button.height<=viewport.height+1);await d.screenshot({path:join(output,`correction-dialog-${viewport.width}.png`)});
  });
  await check("Audio correction local models use no API profile and match the recording language",{viewport:{width:900,height:640}},async p=>{
    await openMeeting(p,'Cuộc họp Alpha');await p.getByRole('button',{name:'Hiệu chỉnh',exact:true}).click();const d=p.getByRole('dialog',{name:'Hiệu chỉnh từ file ghi âm'});await d.getByLabel('Cấu hình AI hiệu chỉnh').selectOption('__local__');const models=await d.getByLabel('Model hiệu chỉnh',{exact:true}).locator('option').evaluateAll(o=>o.map(x=>x.value));assert(models.includes('moonshine-vi')&&models.includes('nemotron-3.5-q8'));assert(!models.includes('moonshine-en'));
    const button=await d.getByRole('button',{name:'Bắt đầu hiệu chỉnh',exact:true}).boundingBox();assert(button.y>=0&&button.y+button.height<=640);await d.screenshot({path:join(output,'local-correction-dialog-900.png')});
    await d.getByLabel('Model hiệu chỉnh',{exact:true}).selectOption('moonshine-vi');await d.getByRole('button',{name:'Bắt đầu hiệu chỉnh',exact:true}).click();await p.getByRole('tab',{name:'Bản hiệu chỉnh',exact:true}).waitFor();const c=await p.evaluate(()=>window.qa.calls.find(c=>c.cmd==='correct_transcript').args);assert.equal(c.profileId,'__local__');assert.equal(c.model,'moonshine-vi');assert.equal(c.fallback,null);assert(!await p.evaluate(()=>window.qa.calls.some(c=>c.cmd==='test_ai_connection'||c.cmd==='start_caption')));assert.equal(await p.evaluate(()=>window.qa.snapshot.sessions[0].corrected.provider),'local');
  });
  await check("Audio correction local model missing blocks start without downloading or choosing cloud",{missingBundled:true,missingModels:true},async p=>{
    await openMeeting(p,'Cuộc họp Alpha');await p.getByRole('button',{name:'Hiệu chỉnh',exact:true}).click();const d=p.getByRole('dialog',{name:'Hiệu chỉnh từ file ghi âm'});await d.getByLabel('Cấu hình AI hiệu chỉnh').selectOption('__local__');assert(await d.getByRole('button',{name:'Bắt đầu hiệu chỉnh',exact:true}).isDisabled());assert((await d.innerText()).includes('Chưa có model local đã tải'));assert(!await p.evaluate(()=>window.qa.calls.some(c=>c.cmd==='download_model'||c.cmd==='correct_transcript')));
  });
  for(const [source,model] of [['vi','moonshine-vi'],['en','moonshine-en'],['multi','nemotron-3.5-q8'],['auto','nemotron-3.5-q8'],['ko','nemotron-3.5-q8'],['zh','nemotron-3.5-q8']])await check(`Audio correction default local follows saved recording language ${source}`,{profiles:mockProfiles,recordingLanguage:source},async p=>{
    await openMeeting(p,'Cuộc họp Alpha');await p.getByRole('button',{name:'Hiệu chỉnh',exact:true}).click();const d=p.getByRole('dialog',{name:'Hiệu chỉnh từ file ghi âm'});assert.equal(await d.getByLabel('Cấu hình AI hiệu chỉnh').inputValue(),'__local__');assert.equal(await d.getByLabel('Model hiệu chỉnh',{exact:true}).inputValue(),model);assert(await d.getByRole('button',{name:'Bắt đầu hiệu chỉnh',exact:true}).isEnabled());assert(!await p.evaluate(()=>window.qa.calls.some(c=>['correct_transcript','download_model','test_ai_connection'].includes(c.cmd))));await d.getByRole('button',{name:'Bắt đầu hiệu chỉnh',exact:true}).click();await p.getByRole('tab',{name:'Bản hiệu chỉnh',exact:true}).waitFor();const call=await p.evaluate(()=>window.qa.calls.find(c=>c.cmd==='correct_transcript').args);assert.equal(call.profileId,'__local__');assert.equal(call.model,model);assert.equal(call.fallback,null);
  });
  await check("Audio correction missing multilingual default blocks work and still allows cloud override",{profiles:mockProfiles,missingModels:true,recordingLanguage:"multi"},async p=>{
    await openMeeting(p,'Cuộc họp Alpha');await p.getByRole('button',{name:'Hiệu chỉnh',exact:true}).click();const d=p.getByRole('dialog',{name:'Hiệu chỉnh từ file ghi âm'});assert.equal(await d.getByLabel('Cấu hình AI hiệu chỉnh').inputValue(),'__local__');assert.equal(await d.getByLabel('Model hiệu chỉnh',{exact:true}).inputValue(),'nemotron-3.5-q8');assert((await d.getByLabel('Model hiệu chỉnh',{exact:true}).innerText()).includes('Chưa sẵn sàng'));assert(await d.getByRole('button',{name:'Bắt đầu hiệu chỉnh',exact:true}).isDisabled());assert(!await p.evaluate(()=>window.qa.calls.some(c=>['correct_transcript','download_model'].includes(c.cmd))));await d.getByLabel('Cấu hình AI hiệu chỉnh').selectOption('prof-soniox');await d.getByLabel('Model hiệu chỉnh',{exact:true}).selectOption('stt-async-v6');await d.getByRole('button',{name:'Bắt đầu hiệu chỉnh',exact:true}).click();await p.getByRole('tab',{name:'Bản hiệu chỉnh',exact:true}).waitFor();assert.equal(await p.evaluate(()=>window.qa.calls.find(c=>c.cmd==='correct_transcript').args.profileId),'prof-soniox');
  });
  await check("Audio correction cancellation preserves an existing result and releases its controls",{profiles:mockProfiles,correctionDelay:800,corrected:{text:'keep previous correction',segments:[],provider:'groq',profile_id:'saved',model:'whisper-large-v3',created:'now',audio_sha256:'old'},taskAssignMeeting:{correction:{profileId:'prof-soniox',model:'stt-async-v6'}}},async p=>{
    await openMeeting(p,'Cuộc họp Alpha');await p.getByRole('button',{name:'Hiệu chỉnh lại',exact:true}).click();const d=p.getByRole('dialog',{name:'Hiệu chỉnh từ file ghi âm'});
    await d.getByLabel('Cấu hình AI hiệu chỉnh').selectOption('prof-soniox');await d.getByLabel('Model hiệu chỉnh',{exact:true}).selectOption('stt-async-v6');await d.getByRole('button',{name:'Hiệu chỉnh lại',exact:true}).click();await d.getByRole('button',{name:'Hủy hiệu chỉnh',exact:true}).click();await d.getByRole('alert').waitFor();assert((await d.getByRole('alert').innerText()).includes('Đã hủy'));assert(await d.getByRole('button',{name:'Hiệu chỉnh lại',exact:true}).isEnabled());assert.equal(await p.evaluate(()=>window.qa.snapshot.sessions[0].corrected.text),'keep previous correction');
  });
  await check(
    "Meeting summary dialog generates, switches tabs, and renders structured content",
    {profiles:mockProfiles, taskAssignMeeting:{summary:{profileId:"prof-openai",model:"gpt-4o-mini"}}},
    async (p) => {
      await openMeeting(p, "Cuộc họp Alpha");


      await p.getByRole("button", { name: "Tóm tắt", exact: true }).click();
      const dialog = p.locator("dialog.summary-dialog");
      await dialog.waitFor();
      assert(await dialog.isVisible());

      assert.equal(await dialog.getByLabel("Cấu hình AI tóm tắt").inputValue(),"");assert(await dialog.getByRole("button",{name:"Tạo tóm tắt",exact:true}).isDisabled());await dialog.getByLabel("Cấu hình AI tóm tắt").selectOption("prof-openai");await dialog.getByLabel("Model tóm tắt",{exact:true}).selectOption("gpt-4o-mini");
      await dialog.getByLabel("Kiểu tóm tắt").selectOption("decisions");
      await dialog.getByRole("button", { name: "Tạo tóm tắt", exact: true }).click();
      await dialog.waitFor({ state: "hidden" });

      assert(await p.getByRole("tab", { name: "Tóm tắt" }).isVisible());
      assert(await p.getByText("Ý chính cuộc họp 1").isVisible());
      assert(await p.getByText("Đã thông qua ngân sách").isVisible());

      await p.getByRole("tab", { name: "Bản chép lời" }).click();
      assert(await p.getByText("Nội dung thử tiếng Việt.").isVisible());

      await p.getByRole("tab", { name: "Tóm tắt" }).click();
      assert(await p.getByText("Ý chính cuộc họp 1").isVisible());
      await p.screenshot({ path: join(output, "meeting-summary.png") });

      await p.getByRole("button", { name: "Xóa tóm tắt", exact: true }).click();
      await p.waitForFunction(() => !window.qa.snapshot.sessions[0].summary);
      assert.equal(await p.getByRole("tab", { name: "Tóm tắt" }).count(), 0);
    },
  );
  for(const viewport of [{width:900,height:640},{width:1180,height:800}])await check(`Post-session summary model picker layout ${viewport.width}`,{viewport,profiles:mockProfiles},async p=>{
    await openMeeting(p,'Cuộc họp Alpha');await p.getByRole('button',{name:'Tóm tắt',exact:true}).click();const d=p.locator('dialog.summary-dialog');await d.getByLabel('Cấu hình AI tóm tắt').selectOption('prof-openai');assert(await d.getByRole('button',{name:'Tạo tóm tắt',exact:true}).isDisabled());await d.getByLabel('Model tóm tắt',{exact:true}).selectOption('gpt-4o');const box=await d.boundingBox(),b=await d.getByRole('button',{name:'Tạo tóm tắt',exact:true}).boundingBox();assert(box.x>=0&&box.x+box.width<=viewport.width+1);assert(b.y>=0&&b.y+b.height<=viewport.height+1);await d.screenshot({path:join(output,`summary-picker-${viewport.width}.png`)});await d.getByRole('button',{name:'Hủy',exact:true}).click();assert(!await p.evaluate(()=>window.qa.calls.some(c=>c.cmd==='test_ai_connection'||c.cmd==='summarize_transcript')));
  });
  await check("Post-session summary resets hidden fallback and keeps failure inline",{profiles:mockProfiles,summaryError:'OpenAI 429: hết hạn mức',taskAssignMeeting:{summary:{profileId:'prof-openai',model:'gpt-4o-mini',fallback:{profileId:'missing',model:'old'}}}},async p=>{
    await openMeeting(p,'Cuộc họp Alpha');await p.getByRole('button',{name:'Tóm tắt',exact:true}).click();const d=p.locator('dialog.summary-dialog');assert.equal(await d.getByLabel('Cấu hình AI tóm tắt').inputValue(),'');await d.getByLabel('Cấu hình AI tóm tắt').selectOption('prof-openai');await d.getByLabel('Model tóm tắt',{exact:true}).selectOption('gpt-4o');await d.locator('summary').click();const backup=d.getByLabel('Model tóm tắt dự phòng');await backup.selectOption(JSON.stringify({profileId:'prof-gemini',model:'gemini-2.5-flash'}));await d.getByLabel('Model tóm tắt',{exact:true}).selectOption('gpt-4o-mini');assert.equal(await backup.inputValue(),'');await d.getByRole('button',{name:'Tạo tóm tắt',exact:true}).click();await d.getByRole('alert').waitFor();assert((await d.getByRole('alert').innerText()).includes('429'));assert(await d.getByRole('button',{name:'Tạo tóm tắt',exact:true}).isEnabled());assert.equal(await p.evaluate(()=>window.qa.calls.find(c=>c.cmd==='summarize_transcript').args.fallback),null);assert.equal(await p.evaluate(()=>window.qa.snapshot.sessions[0].summary),undefined);assert.equal(await p.evaluate(()=>window.qa.snapshot.sessions[0].segments[0].text),'Nội dung thử tiếng Việt.');
  });
  await check("Meeting shared model selection synchronizes transcription in both directions", {profiles:[...mockProfiles,{...mockProfiles.find(p=>p.provider==='soniox'),id:'prof-soniox2',name:'Soniox 2'}]},async p=>{
    await p.getByLabel('Bật phụ đề khi bắt đầu cuộc họp').check();await p.getByLabel('Dịch sang trong cuộc họp').check();assert(await p.getByRole('button',{name:'Bắt đầu ghi',exact:true}).isDisabled());
    const shared=p.getByLabel('Model dùng chung cho cuộc họp');const choice={profileId:'prof-soniox2',model:'stt-rt-v6'};
    const labels=await shared.locator('option').allTextContents();assert(labels.includes('Soniox 2/stt-rt-v6'));assert(!labels.some(s=>s.includes('gpt-')||s.includes('gemini')||s.includes('Local/')));
    await shared.selectOption(JSON.stringify(choice));assert.equal(await p.getByLabel('Cách chép lời',{exact:true}).inputValue(),'soniox');assert.equal(await p.getByLabel('Cấu hình Soniox',{exact:true}).inputValue(),'prof-soniox2');assert((await p.getByLabel('Model chép lời Soniox').innerText()).includes('Soniox 2/stt-rt-v6'));
    await p.getByLabel('Cấu hình Soniox',{exact:true}).selectOption('prof-soniox');assert.equal(await shared.inputValue(),JSON.stringify({profileId:'prof-soniox',model:'stt-rt-v6'}));
    await shared.selectOption(JSON.stringify(choice));await p.getByLabel('Ngôn ngữ bản dịch',{exact:true}).selectOption('ko');await shared.scrollIntoViewIfNeeded();await p.screenshot({path:join(output,'meeting-shared-model.png')});
    await p.getByRole('button',{name:'Bắt đầu ghi',exact:true}).click();await p.waitForFunction(()=>window.qa.calls.some(c=>c.cmd==='start'));const calls=await p.evaluate(()=>window.qa.calls),o=calls.find(c=>c.cmd==='start').args.options;assert.equal(o.provider,'soniox');assert.equal(o.profileId,'prof-soniox2');assert.equal(o.translationLanguage,'ko');assert.equal(o.captionsEnabled,true);assert(!calls.some(c=>c.cmd==='start_caption'));
  });
  await check("Meeting shared original captions allow local and block translation after switching back",{profiles:mockProfiles},async p=>{
    await p.getByLabel('Ngôn ngữ',{exact:true}).selectOption('vi');await p.getByLabel('Bật phụ đề khi bắt đầu cuộc họp').check();const shared=p.getByLabel('Model dùng chung cho cuộc họp');assert.equal(await shared.inputValue(),JSON.stringify({profileId:'__local__',model:'moonshine-vi'}));
    await shared.selectOption(JSON.stringify({profileId:'prof-soniox',model:'stt-rt-v6'}));await shared.selectOption(JSON.stringify({profileId:'__local__',model:'moonshine-vi'}));assert.equal(await p.getByLabel('Cách chép lời',{exact:true}).inputValue(),'local');
    await p.getByLabel('Dịch sang trong cuộc họp').check();assert(await p.getByRole('button',{name:'Bắt đầu ghi',exact:true}).isDisabled());await shared.selectOption(JSON.stringify({profileId:'prof-soniox',model:'stt-rt-v6'}));await p.getByLabel('Cách chép lời',{exact:true}).selectOption('local');assert.equal(await shared.inputValue(),'');assert(await p.getByRole('button',{name:'Bắt đầu ghi',exact:true}).isDisabled());
  });
  await check("Soniox starts transcription-only without local downloads, and translation is opt-in", {profiles:mockProfiles,missingModels:true}, async p=>{
    await p.getByLabel('Ngôn ngữ',{exact:true}).selectOption('multi');
    await p.getByLabel('Cách chép lời',{exact:true}).selectOption('soniox');
    await p.getByLabel('Cấu hình Soniox',{exact:true}).selectOption('prof-soniox');
    assert.equal(await p.getByLabel('Bật phụ đề khi bắt đầu cuộc họp').isChecked(),false);
    assert.equal(await p.getByLabel('Dịch sang trong cuộc họp').count(),1);
    assert(!await p.getByLabel('Dịch sang trong cuộc họp').isChecked());
    assert.equal(await p.getByLabel('Dự phòng local khi Soniox lỗi').isChecked(),false);
    await p.getByRole('button',{name:'Bắt đầu ghi',exact:true}).click();
    const opts=await p.evaluate(()=>window.qa.calls.find(c=>c.cmd==='start').args.options);
    assert.equal(opts.provider,'soniox');assert.equal(opts.translationLanguage,null);assert.equal(opts.localFallback,false);
    assert(!await p.evaluate(()=>window.qa.calls.some(c=>c.cmd==='start_caption'||c.cmd==='open_caption_window')));
  });
  await check("Caption sidebar manages current meeting and opening overlay attaches its existing stream", {profiles:mockProfiles}, async p=>{
    await p.getByLabel('Dịch sang trong cuộc họp').check();
    await p.getByLabel('Model dùng chung cho cuộc họp').selectOption(JSON.stringify({profileId:'prof-soniox',model:'stt-rt-v6'}));
    await p.getByRole('button',{name:'Bắt đầu ghi',exact:true}).click();
    await p.getByRole('button',{name:'Chỉ phụ đề',exact:true}).first().click();
    const panel=p.getByLabel('Phụ đề cuộc họp đang ghi');
    await panel.waitFor();
    assert(await p.getByLabel('Dịch sang trong phiên đang ghi').isChecked());
    assert.equal(await p.getByLabel('Cấu hình AI phụ đề').count(),0,'Do not show unrelated standalone configuration');
    await p.screenshot({path:join(output,'active-meeting-caption-panel.png')});
    await p.getByRole('button',{name:'Hiện hộp phụ đề',exact:true}).click();
    await p.waitForFunction(()=>window.qa.calls.some(c=>c.cmd==='start_caption'));
    const calls=await p.evaluate(()=>window.qa.calls);
    const options=calls.find(c=>c.cmd==='start_caption').args.options;
    assert.equal(options.mode,'meeting');assert.equal(options.profileId,'prof-soniox');assert.equal(options.translate,true);
    assert.equal(calls.filter(c=>c.cmd==='start').length,1);
    await p.getByRole('button',{name:'Ẩn hộp phụ đề',exact:true}).click();
    await p.getByRole('button',{name:'Hiện hộp phụ đề',exact:true}).click();
    assert.equal(await p.evaluate(()=>window.qa.calls.filter(c=>c.cmd==='start_caption').length),1,'Showing again reuses active stream');
    await p.getByRole('button',{name:'Về cuộc họp đang ghi',exact:true}).click();
    await p.locator('.meeting-page').waitFor();
  });
  await check("Meeting translation works without initial captions and captions can start later", {profiles:mockProfiles}, async p=>{
    await p.getByLabel('Dịch sang trong cuộc họp').check();
    await p.getByLabel('Ngôn ngữ bản dịch',{exact:true}).selectOption('vi');
    await p.getByLabel('Model dùng chung cho cuộc họp').selectOption(JSON.stringify({profileId:'prof-soniox',model:'stt-rt-v6'}));
    assert(!await p.getByLabel('Bật phụ đề khi bắt đầu cuộc họp').isChecked());
    await p.getByRole('button',{name:'Bắt đầu ghi',exact:true}).click();
    await p.waitForFunction(()=>window.qa.calls.some(c=>c.cmd==='start'));
    const opts=await p.evaluate(()=>window.qa.calls.find(c=>c.cmd==='start').args.options);
    assert.equal(opts.translationLanguage,'vi');assert.equal(opts.captionsEnabled,false);
    assert(!await p.evaluate(()=>window.qa.calls.some(c=>c.cmd==='open_caption_window')));
    await p.getByLabel('Nội dung hiển thị').selectOption('bilingual');
    await p.evaluate(async()=>{const s=window.qa.snapshot.sessions.find(s=>s.id===window.qa.snapshot.active_id);const {emit}=await import('/node_modules/@tauri-apps/api/event.js');await emit('meeting-translation-preview',{id:s.id,language:'vi',text:'Bản dịch đang chạy.',committed:0})});
    await p.locator('[data-live-translation]').getByText('Bản dịch đang chạy.',{exact:true}).waitFor();
    await p.evaluate(async()=>{const s=window.qa.snapshot.sessions.find(s=>s.id===window.qa.snapshot.active_id);s.translations=[{id:'committed',source_id:'source',start:0,end:2,original:'你好',text:'Bản dịch đã chốt.',language:'vi',provider:'soniox'}];window.qa.snapshot.translation_preview={id:s.id,language:'vi',text:'Câu tiếp theo.',committed:1};const {emit}=await import('/node_modules/@tauri-apps/api/event.js');await emit('meeting-translation-preview',window.qa.snapshot.translation_preview);await emit('session-changed',s.id)});
    await p.waitForFunction(()=>document.querySelector('.saved-translation')?.textContent==='Bản dịch đã chốt. Câu tiếp theo.');
    await p.getByLabel('Nội dung hiển thị').selectOption('translation');
    assert.equal((await p.locator('[data-live-translation]').innerText()).trim(),'Câu tiếp theo.');
    assert.equal(await p.locator('[data-live-caption]').count(),0);
    assert(await p.getByLabel('Dịch sang khi bật phụ đề').isEnabled());
    await p.locator('.record-controls').getByRole('button',{name:'Phụ đề',exact:true}).click();
    await p.waitForFunction(()=>window.qa.calls.some(c=>c.cmd==='start_caption'));
    const c=await p.evaluate(()=>window.qa.calls.find(c=>c.cmd==='start_caption').args.options);
    assert.equal(c.translate,true);assert.equal(c.profileId,'prof-soniox');
    await p.getByLabel('Dịch sang khi bật phụ đề').uncheck();
    await p.waitForFunction(()=>window.qa.calls.some(c=>c.cmd==='set_meeting_translation'&&c.args.target===null));
    await p.getByLabel('Dịch sang khi bật phụ đề').check();
    await p.getByLabel('Ngôn ngữ dịch đích cuộc họp').selectOption('en');
    await p.waitForFunction(()=>window.qa.calls.some(c=>c.cmd==='set_meeting_translation'&&c.args.target==='en'));
    assert.equal(await p.evaluate(()=>window.qa.calls.filter(c=>c.cmd==='start_caption').length),1,'Translation changes do not start another capture worker');
  });
  await check("Soniox translation checkbox shares the meeting engine and automatically shows overlay", {profiles:mockProfiles}, async p=>{
    await p.getByLabel('Cách chép lời',{exact:true}).selectOption('soniox');
    await p.getByLabel('Cấu hình Soniox',{exact:true}).selectOption('prof-soniox');
    await p.getByLabel('Bật phụ đề khi bắt đầu cuộc họp').check();
    await p.getByLabel('Dịch sang trong cuộc họp').check();
    await p.getByLabel('Ngôn ngữ bản dịch',{exact:true}).selectOption('en');
    await p.getByRole('button',{name:'Bắt đầu ghi',exact:true}).click();
    await p.waitForFunction(()=>window.qa.calls.some(c=>c.cmd==='open_caption_window'));
    const opts=await p.evaluate(()=>window.qa.calls.find(c=>c.cmd==='start').args.options);
    assert.equal(opts.translationLanguage,'en');
    assert(!await p.evaluate(()=>window.qa.calls.some(c=>c.cmd==='start_caption')),'No second STT session from the UI');
  });
  await check("Meeting source captions are opt-in and share Soniox without requesting translation", {profiles:mockProfiles}, async p=>{
    await p.getByLabel('Cách chép lời',{exact:true}).selectOption('soniox');
    await p.getByLabel('Cấu hình Soniox',{exact:true}).selectOption('prof-soniox');
    await p.getByLabel('Bật phụ đề khi bắt đầu cuộc họp').check();
    assert(!await p.getByLabel('Dịch sang trong cuộc họp').isChecked());
    await p.getByRole('button',{name:'Bắt đầu ghi',exact:true}).click();
    await p.waitForFunction(()=>window.qa.calls.some(c=>c.cmd==='open_caption_window'));
    const options=await p.evaluate(()=>window.qa.calls.find(c=>c.cmd==='start').args.options);
    assert.equal(options.captionsEnabled,true);assert.equal(options.translationLanguage,null);
    assert(!await p.evaluate(()=>window.qa.calls.some(c=>c.cmd==='start_caption')));
    assert.equal(await p.getByText('Tùy chỉnh chữ phụ đề',{exact:true}).count(),0);
  });
  await check("Standalone original captions do not translate or hide original behind bilingual setting", {profiles:mockProfiles}, async p=>{
    await p.getByRole('button',{name:'Chỉ phụ đề',exact:true}).click();
    assert(!await p.getByLabel('Dịch sang',{exact:true}).isChecked());
    assert.equal(await p.getByLabel('Ngôn ngữ dịch đích').count(),0);
    await p.getByLabel('Ngôn ngữ gốc').selectOption('en');
    await p.getByLabel('Cấu hình AI phụ đề').selectOption('prof-soniox');
    await p.getByRole('button',{name:'Bắt đầu phụ đề',exact:true}).click();
    await p.waitForFunction(()=>window.qa.calls.some(c=>c.cmd==='start_caption'));
    const options=await p.evaluate(()=>window.qa.caption.options);
    assert.equal(options.translate,false);assert.equal(options.sourceLanguage,'en');
    assert.equal(await p.getByText('Tùy chỉnh chữ phụ đề',{exact:true}).count(),0);
  });
  await check("Local Moonshine captions need no profile or API and persist after reload", {profiles:[]},async p=>{
    await p.getByRole('button',{name:'Chỉ phụ đề',exact:true}).click();await p.getByLabel('Ngôn ngữ gốc').selectOption('vi');await p.getByLabel('Cấu hình AI phụ đề').selectOption('__local__');await p.getByLabel('Model phụ đề').selectOption('moonshine-vi');
    assert.equal(await p.getByRole('button',{name:'Kiểm tra model',exact:true}).count(),0);
    await p.reload();await p.getByRole('button',{name:'Chỉ phụ đề',exact:true}).click();assert.equal(await p.getByLabel('Cấu hình AI phụ đề').inputValue(),'__local__');assert.equal(await p.getByLabel('Model phụ đề').inputValue(),'moonshine-vi');
    await p.getByRole('button',{name:'Bắt đầu phụ đề',exact:true}).click();await p.waitForFunction(()=>window.qa.calls.some(c=>c.cmd==='start_caption'));
    const calls=await p.evaluate(()=>window.qa.calls),o=calls.find(c=>c.cmd==='start_caption').args.options;assert.equal(o.profileId,'__local__');assert.equal(o.model,'moonshine-vi');assert.equal(o.translate,false);assert.equal(o.apiKey,'');assert.equal(o.fallback,null);assert(!calls.some(c=>c.cmd==='test_ai_connection'||c.cmd==='start'));
  });
  await check("Local caption models follow source language and cannot translate",{},async p=>{
    await p.getByRole('button',{name:'Chỉ phụ đề',exact:true}).click();await p.getByLabel('Ngôn ngữ gốc').selectOption('vi');await p.getByLabel('Cấu hình AI phụ đề').selectOption('__local__');assert.equal(await p.getByLabel('Model phụ đề').inputValue(),'moonshine-vi');
    await p.getByLabel('Ngôn ngữ gốc').selectOption('en');const moon=p.getByLabel('Model phụ đề').locator('option[value="moonshine-vi"]');await p.getByLabel('Model phụ đề').locator('option[value="moonshine-vi"]:disabled').waitFor({state:'attached'});assert((await moon.innerText()).includes('Đã tải'));assert(await moon.evaluate(o=>o.disabled));assert(await p.getByRole('button',{name:'Bắt đầu phụ đề',exact:true}).isDisabled());await p.getByLabel('Model phụ đề').selectOption('nemotron-3.5-q8');assert(await p.getByRole('button',{name:'Bắt đầu phụ đề',exact:true}).isEnabled());await p.getByLabel('Model phụ đề').scrollIntoViewIfNeeded();await p.screenshot({path:join(output,'local-installed-models-english.png')});
    await p.getByLabel('Ngôn ngữ gốc').selectOption('vi');await p.getByLabel('Model phụ đề').locator('option[value="moonshine-vi"]:not(:disabled)').waitFor({state:'attached'});assert(!await moon.evaluate(o=>o.disabled));await p.getByLabel('Model phụ đề').selectOption('moonshine-vi');assert(await p.getByRole('button',{name:'Bắt đầu phụ đề',exact:true}).isEnabled());
    await p.getByLabel('Dịch sang',{exact:true}).check();assert.equal(await p.getByLabel('Cấu hình AI phụ đề').locator('option[value="__local__"]').count(),0);assert(await p.getByRole('button',{name:'Bắt đầu phụ đề',exact:true}).isDisabled());
  });
  await check("Local caption model must be installed before starting",{missingBundled:true},async p=>{
    await p.getByRole('button',{name:'Chỉ phụ đề',exact:true}).click();await p.getByLabel('Ngôn ngữ gốc').selectOption('vi');await p.getByLabel('Cấu hình AI phụ đề').selectOption('__local__');assert(await p.getByRole('button',{name:'Bắt đầu phụ đề',exact:true}).isDisabled());await p.getByText('Thiếu model đi kèm. Hãy cài lại Innote.',{exact:false}).waitFor();
  });
  const groqProfile={id:'prof-groq',name:'Groq Free',provider:'groq',base_url:'',has_key:true,key_tail:'mock',created:'2026-11-03'};
  await check("Processing levels preserve Groq model, profile and language across reload",{profiles:[groqProfile]},async p=>{
    await p.getByRole('button',{name:'Chỉ phụ đề',exact:true}).click();
    await p.getByLabel('Ngôn ngữ gốc').selectOption('en');await p.getByLabel('Cấu hình AI phụ đề').selectOption('prof-groq');
    await p.getByLabel('Model phụ đề').selectOption('whisper-large-v3-turbo');
    for(const label of ['Nhanh','Cân bằng','Chính xác']) {
      await p.getByRole('radio',{name:'Phụ đề · '+label,exact:true}).check();
      assert.equal(await p.getByLabel('Model phụ đề').inputValue(),'whisper-large-v3-turbo');assert.equal(await p.getByLabel('Cấu hình AI phụ đề').inputValue(),'prof-groq');
      assert.equal(await p.getByLabel('Ngôn ngữ gốc').inputValue(),'en');
    }
    await p.reload();await p.getByRole('button',{name:'Chỉ phụ đề',exact:true}).click();
    assert(await p.getByRole('radio',{name:'Phụ đề · Chính xác',exact:true}).isChecked());
    await p.getByText('Giải thích mức xử lý',{exact:true}).click();
    assert((await p.getByText(/Giữ nguyên model Whisper:/).innerText()).includes('4 giây'));
    await p.getByRole('button',{name:'Bắt đầu phụ đề',exact:true}).click();
    await p.waitForFunction(()=>window.qa.calls.some(c=>c.cmd==='start_caption'));
    const options=await p.evaluate(()=>window.qa.calls.find(c=>c.cmd==='start_caption').args.options);
    assert.equal(options.model,'whisper-large-v3-turbo');assert.equal(options.profileId,'prof-groq');assert.equal(options.speechQuality,'accurate');
    assert(await p.getByRole('radio',{name:'Phụ đề · Nhanh',exact:true}).isDisabled());
    await p.screenshot({path:join(output,'speech-quality-groq.png')});
  });
  await check("Processing levels isolate meeting and captions and use the selected local model",{profiles:[groqProfile]},async p=>{
    await p.getByLabel('Ngôn ngữ',{exact:true}).selectOption('en');
    await p.getByLabel('Model chép lời local',{exact:true}).selectOption('nemotron-3.5-q8');
    await p.getByRole('radio',{name:'Cuộc họp · Nhanh',exact:true}).check();
    await p.getByRole('button',{name:'Chỉ phụ đề',exact:true}).click();
    await p.getByLabel('Ngôn ngữ gốc').selectOption('en');await p.getByLabel('Cấu hình AI phụ đề').selectOption('prof-groq');
    assert(await p.getByRole('radio',{name:'Phụ đề · Cân bằng',exact:true}).isChecked());
    await p.getByRole('radio',{name:'Phụ đề · Chính xác',exact:true}).check();
    await p.getByRole('button',{name:'Cuộc họp mới',exact:true}).click();
    assert.equal(await p.getByLabel('Model chép lời local',{exact:true}).inputValue(),'nemotron-3.5-q8');
    await p.getByLabel('Dịch sang trong cuộc họp').uncheck();
    assert(await p.getByRole('radio',{name:'Cuộc họp · Nhanh',exact:true}).isChecked());
    await p.getByRole('button',{name:'Bắt đầu ghi',exact:true}).click();await p.waitForFunction(()=>window.qa.calls.some(c=>c.cmd==='start'));
    const options=await p.evaluate(()=>window.qa.calls.find(c=>c.cmd==='start').args.options);assert.equal(options.model,'nemotron-3.5-q8');assert.equal(options.speechQuality,'fast');
  });
  await check("Processing levels do not pretend fixed APIs expose accuracy controls",{profiles:mockProfiles},async p=>{
    await p.getByRole('button',{name:'Chỉ phụ đề',exact:true}).click();await p.getByLabel('Dịch sang',{exact:true}).check();
    await p.getByLabel('Cấu hình AI phụ đề').selectOption('prof-openai');
    assert(await p.getByRole('radio',{name:'Phụ đề · Nhanh',exact:true}).isDisabled());assert(await p.getByRole('radio',{name:'Phụ đề · Chính xác',exact:true}).isDisabled());
    assert(await p.getByRole('radio',{name:'Phụ đề · Cân bằng',exact:true}).isChecked());
  });
  await check("Groq provider can be added only after verification and exposes relevant model IDs", {}, async p=>{
    await p.getByRole('button',{name:'Cài đặt',exact:true}).click();await p.getByRole('button',{name:'Thêm cấu hình',exact:true}).click();
    const dialog=p.getByRole('dialog',{name:'Thêm cấu hình'});await dialog.getByLabel('Nhà cung cấp',{exact:true}).selectOption('groq');await dialog.getByLabel('Tên cấu hình').fill('Groq Free');await dialog.getByLabel('API key',{exact:true}).fill('mock-groq-key');
    const save=dialog.getByRole('button',{name:'Thêm cấu hình',exact:true});assert(await save.isDisabled());
    await dialog.screenshot({path:join(output,'groq-profile-dialog.png')});
    await dialog.getByRole('button',{name:'Kiểm tra kết nối',exact:true}).click();await p.waitForFunction(()=>window.qa.calls.some(c=>c.cmd==='test_ai_connection'&&c.args.input.provider==='groq'));assert(await save.isEnabled());
  });
  await check("Profile creation has no model or editable Groq endpoint and verifies the key only", {}, async p=>{
    await p.getByRole('button',{name:'Cài đặt',exact:true}).click();await p.getByRole('button',{name:'Thêm cấu hình',exact:true}).click();const d=p.getByRole('dialog',{name:'Thêm cấu hình'});
    await d.getByLabel('Nhà cung cấp',{exact:true}).selectOption('groq');assert.equal(await d.getByLabel('Model',{exact:true}).count(),0);assert.equal(await d.getByLabel('Base URL',{exact:true}).count(),0);assert.equal(await d.getByRole('button',{name:'Lấy model',exact:true}).count(),0);
    await d.getByLabel('Tên cấu hình').fill('groq1');await d.getByLabel('API key',{exact:true}).fill('mock-key');await d.getByRole('button',{name:'Kiểm tra kết nối',exact:true}).click();await p.waitForFunction(()=>window.qa.calls.some(c=>c.cmd==='test_ai_connection'));const input=await p.evaluate(()=>window.qa.calls.find(c=>c.cmd==='test_ai_connection').args.input);assert.equal(input.model,'');assert.equal(input.baseUrl,'');
  });
  await check("Aliased models distinguish two Groq profiles and caption fallback persists as raw IDs", {profiles:[...mockProfiles,groqProfile,{...groqProfile,id:'prof-groq2',name:'groq2'}]}, async p=>{
    await p.getByRole('button',{name:'Mẫu cấu hình',exact:true}).click();await p.getByRole('tab',{name:'Phụ đề',exact:true}).click();await p.getByRole('button',{name:'Tạo mẫu',exact:true}).click();await p.getByLabel('Dịch sang trong mẫu').uncheck();
    await p.locator('label',{hasText:'Phụ đề trực tiếp'}).locator('select').first().selectOption('prof-groq');
    const model=p.getByLabel('Model phụ đề',{exact:true});assert((await model.locator('option').allTextContents()).includes('Groq Free/whisper-large-v3-turbo'));
    const fallback=p.getByLabel('Dự phòng · Phụ đề trực tiếp',{exact:true});const choice={profileId:'prof-groq2',model:'whisper-large-v3'};await fallback.selectOption(JSON.stringify(choice));assert((await fallback.locator('option:checked').innerText()).includes('groq2/whisper-large-v3'));
    await p.getByLabel('Dịch sang trong mẫu').check();assert(!(await fallback.locator('option').allTextContents()).some(s=>s.includes('groq2/whisper')));
    await p.getByLabel('Dịch sang trong mẫu').uncheck();assert((await fallback.locator('option').allTextContents()).some(s=>s.includes('groq2/whisper')));
    assert.equal(await p.locator('.task-fallbacks select').count(),1);
    await p.locator('.task-fallbacks').scrollIntoViewIfNeeded();await p.screenshot({path:join(output,'profile-alias-fallback.png')});
    await p.getByLabel('Tên mẫu',{exact:true}).fill('Groq có dự phòng');await p.getByRole('button',{name:'Lưu mẫu',exact:true}).click();await p.locator('.preset-row').filter({hasText:'Groq có dự phòng'}).getByRole('button',{name:'Sử dụng',exact:true}).click();await p.getByRole('button',{name:'Bắt đầu phụ đề',exact:true}).click();await p.waitForFunction(()=>window.qa.calls.some(c=>c.cmd==='start_caption'));const options=await p.evaluate(()=>window.qa.caption.options);assert.deepEqual(options.fallback,choice);assert.equal(options.model,'whisper-large-v3-turbo');
  });
  await check("Groq Whisper is selectable for original captions without translation and keeps model choice", {profiles:[...mockProfiles,groqProfile]}, async p=>{
    await p.getByRole('button',{name:'Chỉ phụ đề',exact:true}).click();await p.getByLabel('Cấu hình AI phụ đề').selectOption('prof-groq');
    const models=await p.getByLabel('Model phụ đề').locator('option').allTextContents();assert(models.includes('Groq Free/whisper-large-v3')&&models.includes('Groq Free/whisper-large-v3-turbo'));assert(!models.includes('openai/gpt-oss-20b'));
    await p.getByLabel('Model phụ đề').selectOption('whisper-large-v3');await p.getByRole('button',{name:'Bắt đầu phụ đề',exact:true}).click();await p.waitForFunction(()=>window.qa.calls.some(c=>c.cmd==='start_caption'));
    const opts=await p.evaluate(()=>window.qa.caption.options);assert.equal(opts.model,'whisper-large-v3');assert.equal(opts.profileId,'prof-groq');assert.equal(opts.translate,false);
    await p.getByRole('button',{name:'Dừng phụ đề',exact:true}).click();await p.getByRole('button',{name:'Bắt đầu phụ đề',exact:true}).waitFor();
    await p.getByLabel('Dịch sang',{exact:true}).check();assert(await p.getByRole('button',{name:'Bắt đầu phụ đề',exact:true}).isDisabled());
  });
  await check("Groq original-caption assignment persists through Settings and restart", {profiles:[...mockProfiles,groqProfile]}, async p=>{
    await p.getByRole('button',{name:'Mẫu cấu hình',exact:true}).click();await p.getByRole('tab',{name:'Phụ đề',exact:true}).click();await p.getByRole('button',{name:'Tạo mẫu',exact:true}).click();
    await p.getByLabel('Dịch sang trong mẫu').uncheck();
    const profile=p.locator('label',{hasText:'Phụ đề trực tiếp'}).locator('select').first();await profile.selectOption('prof-groq');await p.getByLabel('Model phụ đề').selectOption('whisper-large-v3');
    await p.getByLabel('Tên mẫu',{exact:true}).fill('Groq saved');await p.getByRole('button',{name:'Lưu mẫu',exact:true}).click();await p.reload();await p.getByRole('button',{name:'Mẫu cấu hình',exact:true}).click();await p.getByRole('tab',{name:'Phụ đề',exact:true}).click();await p.locator('.preset-row').filter({hasText:'Groq saved'}).getByRole('button',{name:'Sử dụng',exact:true}).click();assert(!await p.getByLabel('Dịch sang',{exact:true}).isChecked());assert.equal(await p.getByLabel('Cấu hình AI phụ đề').inputValue(),'prof-groq');assert.equal(await p.getByLabel('Model phụ đề').inputValue(),'whisper-large-v3');
  });
  await check("Groq text tasks offer GPT OSS and Qwen but never Whisper", {profiles:[...mockProfiles,groqProfile]}, async p=>{
    await openMeeting(p,'Cuộc họp Alpha');await p.getByRole('button',{name:'Tóm tắt',exact:true}).click();const d=p.locator('dialog.summary-dialog');await d.getByLabel('Cấu hình AI tóm tắt').selectOption('prof-groq');
    const values=await d.getByLabel('Model tóm tắt',{exact:true}).locator('option').evaluateAll(options=>options.map(o=>o.value));for(const m of ['openai/gpt-oss-20b','openai/gpt-oss-120b','qwen/qwen3.8-27b'])assert(values.includes(m));assert(!values.includes('whisper-large-v3'));
  });
  for(const viewport of [{width:900,height:640},{width:1180,height:800}]) {
    await check(`Inline caption appearance applies presets and edits only the session ${viewport.width}`,{viewport,profiles:mockProfiles},async p=>{
      const presetsBefore=await p.evaluate(()=>localStorage.getItem('innote.workflow-presets.v1'));
      await p.getByRole('button',{name:'Chỉ phụ đề',exact:true}).click();const panel=p.locator('#standalone-caption-style');
      assert(await panel.getByLabel('Cỡ chữ phụ đề').isVisible(),'Appearance opens in the caption page by default');assert.equal(await p.locator('.preset-page').count(),0);
      await panel.getByLabel('Cỡ chữ phụ đề').fill('34');await panel.getByLabel('Cỡ chữ phụ đề').dispatchEvent('input');await panel.getByLabel('Song ngữ',{exact:true}).check();
      assert.equal(await p.evaluate(()=>JSON.parse(localStorage.getItem('innote.caption.style')).fontSize),34);assert.equal(await p.evaluate(()=>localStorage.getItem('innote.workflow-presets.v1')),presetsBefore,'Editing session appearance cannot mutate a saved template');
      await p.evaluate(()=>{const key='innote.workflow-presets.v1',root=JSON.parse(localStorage.getItem(key)),base=root.presets.find(x=>x.mode==='captionsonly');root.presets.push({...base,id:'inline-style',name:'Phụ đề chữ xanh',source:'en',translate:false,tasks:{caption:{profileId:'prof-soniox',model:'stt-rt-v6',targetLang:'vi',translate:false}},captionStyle:{fontSize:40,align:'left',color:'#112233',backgroundColor:'#334455',showOriginal:true,transparent:false}});localStorage.setItem(key,JSON.stringify(root));});await p.reload();await p.getByRole('button',{name:'Chỉ phụ đề',exact:true}).click();
      assert.equal(await panel.locator('summary').count(),0,'No disclosure to hide the appearance fields');assert.equal(await p.getByText('Thu từ loa/tai nghe',{exact:true}).count(),0);await p.getByLabel('Mẫu cấu hình phụ đề').selectOption('inline-style');await p.getByRole('button',{name:'Áp mẫu',exact:true}).click();
      await p.waitForFunction(()=>document.querySelector('#standalone-caption-style [aria-label="Cỡ chữ phụ đề"]')?.value==='40');assert(await panel.getByLabel('Cỡ chữ phụ đề').isVisible(),'Applying a template keeps appearance visible');assert.equal(await panel.getByLabel('Căn chữ phụ đề').inputValue(),'left');assert.equal(await panel.getByLabel('Màu chữ phụ đề').inputValue(),'#112233');assert.equal(await panel.getByLabel('Màu nền hộp phụ đề').inputValue(),'#334455');assert(await panel.getByLabel('Song ngữ',{exact:true}).isChecked());assert(!await panel.getByLabel('Trong suốt',{exact:true}).isChecked());
      await panel.scrollIntoViewIfNeeded();assert(await p.locator('.caption-page').evaluate(el=>el.scrollWidth<=el.clientWidth));assert.equal(await panel.locator('.caption-customize-fields').evaluate(el=>getComputedStyle(el).gridTemplateColumns.split(' ').length),2);await p.locator('.toast').waitFor({state:'hidden'});await p.screenshot({path:join(output,`inline-caption-style-${viewport.width}.png`)});
      await panel.getByLabel('Cỡ chữ phụ đề').fill('30');await panel.getByLabel('Cỡ chữ phụ đề').dispatchEvent('input');assert.equal(await p.getByLabel('Mẫu cấu hình phụ đề').inputValue(),'');assert.equal(await p.evaluate(()=>JSON.parse(localStorage.getItem('innote.workflow-presets.v1')).presets.find(x=>x.id==='inline-style').captionStyle.fontSize),40);
      await p.getByLabel('Cấu hình AI phụ đề').selectOption('prof-soniox');await p.getByRole('button',{name:'Bắt đầu phụ đề',exact:true}).click();await p.getByRole('button',{name:'Dừng phụ đề',exact:true}).waitFor();await panel.getByLabel('Căn chữ phụ đề').selectOption('center');assert.equal(await p.evaluate(()=>window.qa.calls.filter(c=>c.cmd==='start_caption').length),1);assert(!await p.evaluate(()=>window.qa.calls.some(c=>c.cmd==='stop_caption')),'Live appearance edits do not restart the audio engine');assert.equal(await p.locator('.preset-page').count(),0);await p.getByRole('button',{name:'Dừng phụ đề',exact:true}).click();
      await p.reload();await p.getByRole('button',{name:'Chỉ phụ đề',exact:true}).click();assert.equal(await panel.getByLabel('Cỡ chữ phụ đề').inputValue(),'30');assert.equal(await panel.getByLabel('Căn chữ phụ đề').inputValue(),'center');
    });
  }
  await check("Saved original and translated text can be viewed, copied and exported independently", {profiles:mockProfiles,bilingual:true}, async p=>{
    await openMeeting(p,'Cuộc họp Alpha');
    await p.getByLabel('Nội dung hiển thị').selectOption('bilingual');
    await p.locator('.saved-translation').getByText('English translated reference.',{exact:true}).waitFor();
    await p.getByLabel('Nội dung hiển thị').selectOption('translation');
    assert(!(await p.locator('.transcript').innerText()).includes('Nội dung thử tiếng Việt.'));
    await p.getByRole('button',{name:'Tải .txt',exact:true}).click();
    await p.waitForFunction(()=>window.qa.calls.some(c=>c.cmd==='export_session'&&c.args.version==='translation:en'));
    await p.getByLabel('Nội dung hiển thị').selectOption('original');
    await p.locator('.transcript').getByText('Nội dung thử tiếng Việt.',{exact:true}).waitFor();
  });
  await check("AI credentials have one Settings entry point", {profiles:mockProfiles}, async(p)=>{
    await p.getByRole('button',{name:'Cài đặt',exact:true}).click();
    assert.equal(await p.locator('.soniox-settings').count(),0);
    assert.equal(await p.locator('input[type="password"]').count(),0);
    await p.getByRole('button',{name:'Thêm cấu hình',exact:true}).click();
    assert.equal(await p.getByRole('dialog',{name:'Thêm cấu hình'}).getByLabel('API key',{exact:true}).count(),1);
  });
  await check(
    "Interrupted meeting keeps its saved transcript without a resume action",
    { interrupted: true },
    async (p) => {
      await openMeeting(p, "Cuộc họp Alpha");
      await p
        .getByText("Nội dung thử tiếng Việt.", { exact: true })
        .waitFor();
      assert.equal(
        await p
          .getByRole("button", {
            name: "Chép tiếp bằng local",
            exact: true,
          })
          .count(),
        0,
      );
      assert.equal(await p.getByText("Cần kiểm tra", { exact: true }).count(), 1);
    },
  );
  await check(
    "Finish asks for confirmation and keeps the transcript",
    { active: true },
    async (p) => {
      await openMeeting(p, "Phiên đang ghi");
      await p.getByRole("button", { name: "Kết thúc", exact: true }).click();
      const dialog = p.getByRole("dialog");
      await dialog
        .getByText("Kết thúc và lưu cuộc họp?", { exact: true })
        .waitFor();
      await dialog
        .getByRole("button", { name: "Tiếp tục ghi", exact: true })
        .click();
      assert.equal(await p.getByRole("dialog").count(), 0);
      await p.getByRole("button", { name: "Kết thúc", exact: true }).click();
      await dialog
        .getByRole("button", { name: "Kết thúc và lưu", exact: true })
        .click();
      await p.waitForFunction(() =>
        window.qa.calls.some((c) => c.cmd === "stop"),
      );
      assert.equal(
        await p
          .getByRole("button", {
            name: "Dừng chép, giữ bản thu",
            exact: true,
          })
          .count(),
        0,
      );
    },
  );
  await check(
    "Long meeting list retains readable rows and scrolls to selectable items",
    {manyMeetings:true},
    async(p)=>{
      await p.getByRole('button',{name:'Danh sách cuộc họp',exact:true}).click();
      const dialog=p.getByRole('dialog',{name:'Cuộc họp của bạn'}),list=dialog.locator('.meetings-list');
      assert.equal(await dialog.locator('.meeting-card').count(),60);
      const sizes=await list.evaluate(el=>({viewport:el.clientHeight,content:el.scrollHeight,row:el.firstElementChild.getBoundingClientRect().height}));
      assert(sizes.row>=48,'Rows collapsed instead of scrolling');assert(sizes.content>sizes.viewport);
      // Use exact title because the row also includes its saved date.
      const target=dialog.locator('.meeting-card').filter({has: p.getByText('Cuộc họp 59',{exact:true})});
      await target.scrollIntoViewIfNeeded();
      await p.screenshot({path:join(output,'meeting-list-60-rows.png')});
      await target.locator('.meeting-card-main').click();
      await p.waitForFunction(()=>document.querySelector('input.meeting-title')?.value==='Cuộc họp 59');
    },
  );
  await check(
    "Meeting list cards can rename and delete meetings",
    {},
    async (p) => {
      await p
        .getByRole("button", { name: "Danh sách cuộc họp", exact: true })
        .click();
      const list = p.locator("dialog.meetings-dialog");
      await p.screenshot({ path: join(output, "meeting-list.png") });
      const alpha = list.locator(".meeting-card").filter({ hasText: "Cuộc họp Alpha" });
      await alpha.getByRole("button", { name: /Đổi tên cuộc họp/ }).click();
      await alpha.getByRole("textbox", { name: "Tên cuộc họp" }).fill("Cuộc họp Alpha mới");
      await alpha.getByRole("button", { name: "Lưu tên", exact: true }).click();
      await p.waitForFunction(() =>
        window.qa.snapshot.sessions.some((s) => s.title === "Cuộc họp Alpha mới"),
      );
      const beta = list.locator(".meeting-card").filter({ hasText: "Cuộc họp Beta" });
      await beta.getByRole("button", { name: /Xóa cuộc họp/ }).click();
      const confirm = p
        .getByRole("dialog")
        .filter({ hasText: "Xóa cuộc họp này?" });
      await confirm.getByRole("button", { name: "Xóa cuộc họp", exact: true }).click();
      await p.waitForFunction(
        () => window.qa.snapshot.sessions.length === 1,
      );
      assert.equal(
        (await p.evaluate(() => window.qa.snapshot.sessions[0].title)),
        "Cuộc họp Alpha mới",
      );
    },
  );
  await check(
    "Delete confirmation removes only the intended mock meeting",
    {},
    async (p) => {
      await openMeeting(p, "Cuộc họp Alpha");
      await p
        .getByRole("button", { name: "Xóa cuộc họp", exact: true })
        .click();
      await p
        .getByRole("dialog")
        .getByRole("button", { name: "Xóa cuộc họp", exact: true })
        .click();
      await p.getByRole("dialog").waitFor({ state: "hidden" });
      assert.deepEqual(
        await p.evaluate(() => window.qa.snapshot.sessions.map((s) => s.title)),
        ["Cuộc họp Beta"],
      );
    },
  );
  await check(
    "Failed model download can be retried and releases progress",
    { missingModels: true, failDownload: true },
    async (p) => {
      await p.getByRole("button", { name: "Mẫu cấu hình", exact: true }).click();
      await p
        .getByRole("button", { name: "Tải model", exact: true })
        .first()
        .click();
      await p
        .getByRole("alert")
        .filter({ hasText: "Không tải được model" })
        .waitFor();
      assert(
        await p
          .getByRole("button", { name: "Tải model", exact: true })
          .first()
          .isEnabled(),
      );
    },
  );
  await check(
    "System and microphone tests target exactly one selected source",
    { signal: true },
    async (p) => {
      await p
        .getByRole("button", { name: "Kiểm tra âm thanh", exact: true })
        .click();
      await p.getByText(/Đã nhận tín hiệu từ âm thanh cuộc họp/).waitFor();
      let args = await p.evaluate(
        () => window.qa.calls.filter((c) => c.cmd === "test_audio").at(-1).args,
      );
      assert.deepEqual(args, { system: "speaker", microphone: null });
      await p
        .getByRole("button", { name: "Kiểm tra micro", exact: true })
        .click();
      await p.getByText(/Đã nhận tín hiệu từ micro/).waitFor();
      args = await p.evaluate(
        () => window.qa.calls.filter((c) => c.cmd === "test_audio").at(-1).args,
      );
      assert.deepEqual(args, { system: null, microphone: "mic" });
    },
  );
  await check(
    "A disabled system source does not prevent testing the microphone",
    {},
    async (p) => {
      await p.getByLabel("Thiết bị phát âm thanh cuộc họp").selectOption("");
      assert(
        await p
          .getByRole("button", { name: "Kiểm tra âm thanh", exact: true })
          .isDisabled(),
      );
      assert(
        await p
          .getByRole("button", { name: "Kiểm tra micro", exact: true })
          .isEnabled(),
      );
      await p
        .getByRole("button", { name: "Kiểm tra micro", exact: true })
        .click();
      await p.getByLabel("Nghe bản thu thử").waitFor();
    },
  );
  await check(
    "Source changes clear stale test audio and diagnosis",
    { signal: true },
    async (p) => {
      await p
        .getByRole("button", { name: "Kiểm tra micro", exact: true })
        .click();
      await p.getByLabel("Nghe bản thu thử").waitFor();
      await p.getByLabel("Micro của bạn").selectOption("");
      assert.equal(await p.getByLabel("Nghe bản thu thử").count(), 0);
      assert.equal(await p.getByText(/Đã nhận tín hiệu từ micro/).count(), 0);
    },
  );
  await check(
    "Audio test errors release controls and do not leave an old player",
    { failTest: true },
    async (p) => {
      await p
        .getByRole("button", { name: "Kiểm tra micro", exact: true })
        .click();
      await p
        .getByRole("alert")
        .filter({ hasText: "Không mở được nguồn thử" })
        .waitFor();
      assert(
        await p
          .getByRole("button", { name: "Kiểm tra micro", exact: true })
          .isEnabled(),
      );
      assert.equal(await p.getByLabel("Nghe bản thu thử").count(), 0);
    },
  );
  await check(
    "Recording, source changes and navigation cannot race an audio test",
    { slowTest: true },
    async (p) => {
      await p
        .getByRole("button", { name: "Kiểm tra micro", exact: true })
        .click();
      assert(
        await p
          .getByRole("button", { name: "Đang kiểm tra nguồn…", exact: true })
          .isDisabled(),
      );
      assert(await p.getByLabel("Micro của bạn").isDisabled());
      await p.getByRole("button", { name: "Cài đặt", exact: true }).click();
      await p.getByRole("alert").filter({ hasText: "Đang thu thử" }).waitFor();
      await p.getByLabel("Nghe bản thu thử").waitFor();
      assert.equal(
        await p.evaluate(
          () => window.qa.calls.filter((c) => c.cmd === "test_audio").length,
        ),
        1,
      );
    },
  );
  await check(
    "Capture warnings are preserved in the test result",
    { signal: true, testWarning: true },
    async (p) => {
      await p
        .getByRole("button", { name: "Kiểm tra âm thanh", exact: true })
        .click();
      await p.getByText("3 lưu ý khi thu thử", { exact: true }).click();
      await p
        .getByText("Nguồn system báo gián đoạn dữ liệu âm thanh. (3 lần)", {
          exact: true,
        })
        .waitFor();
    },
  );
  await check(
    "Old preferences cannot silently opt a new meeting into Soniox",
    {
      preferences: {
        system: "speaker",
        microphone: "mic",
        language: "vi",
        model: "base-q5_1",
        transcribe: true,
        provider: "soniox",
      },
    },
    async (p) => {
      assert.equal(await p.getByLabel("Cách chép lời").inputValue(), "local");
    },
  );
  await check(
    "Fallback reason is visible with export reachable at 900x640",
    { fallback: true, viewport: { width: 900, height: 640 } },
    async (p) => {
      await openMeeting(p, "Cuộc họp Alpha");
      assert(
        await p
          .getByRole("status")
          .filter({ hasText: "Innote đã chuyển sang local" })
          .isVisible(),
      );
      const box = await p
        .getByRole("button", { name: "Tải .md", exact: true })
        .boundingBox();
      assert(box && box.y + box.height <= 640);
      await p.screenshot({ path: join(output, "fallback-900x640.png") });
    },
  );
  await check("Local stays available; missing Soniox key does not silently change engine", {}, async p=>{
    assert.equal(await p.getByLabel('Cách chép lời').inputValue(),'local');
    await p.getByLabel('Cách chép lời').selectOption('soniox');
    await p.getByRole('button',{name:'Bắt đầu ghi',exact:true}).click();
    await p.getByText(/Chưa chọn cấu hình Soniox hợp lệ/).waitFor();
    assert.equal(await p.evaluate(()=>window.qa.snapshot.active_id),null);
    await p.getByLabel('Cách chép lời').selectOption('local');
    await p.getByRole('button',{name:'Bắt đầu ghi',exact:true}).click();
    await p.getByRole('button',{name:'Kết thúc',exact:true}).waitFor();
    assert.equal(await p.evaluate(()=>window.qa.snapshot.sessions.at(-1).provider),'local');
  });
  await check(
    "Soniox key stays out of preferences and start payload; each new meeting defaults local",
    {profiles:mockProfiles},
    async (p) => {
      assert(
        !(await p.evaluate(() => JSON.stringify(localStorage))).includes(
          "fake-ui-secret",
        ),
      );
      await p
        .getByRole("button", { name: "Cuộc họp mới", exact: true })
        .click();
      assert.equal(await p.getByLabel("Cách chép lời").inputValue(), "local");
      await p.getByLabel("Cách chép lời").selectOption("soniox");
      await p.getByLabel("Cấu hình Soniox",{exact:true}).selectOption("prof-soniox");
      await p.getByRole("button", { name: "Bắt đầu ghi", exact: true }).click();
      await p.getByRole("button", { name: "Kết thúc", exact: true }).waitFor();
      const opts = await p.evaluate(
        () => window.qa.calls.find((c) => c.cmd === "start").args.options,
      );
      assert.equal(opts.provider, "soniox");
      assert(!JSON.stringify(opts).includes("fake-ui-secret"));
      await p.evaluate(async()=>{const {emit}=await import('/node_modules/@tauri-apps/api/event.js');await emit('caption-status',{state:'listening',provider:'soniox',sending:true,message:'Đang gửi âm thanh lên Soniox'});});
      await p.locator(".local-label").filter({hasText:"Soniox"}).waitFor();
    },
  );
  await check(
    "Soniox needs a local model only when local fallback was explicitly enabled",
    { missingModels: true, missingBundled:true, profiles:mockProfiles },
    async (p) => {
      await p.getByLabel("Cách chép lời").selectOption("soniox");
      assert(await p.getByRole("button",{name:"Bắt đầu ghi",exact:true}).isEnabled());
      await p.getByLabel("Dự phòng local khi Soniox lỗi").check();
      assert(
        await p
          .getByRole("button", { name: "Bắt đầu ghi", exact: true })
          .isDisabled(),
      );
    },
  );
  await check("Removing a saved Soniox profile leaves local recording available", {profiles:mockProfiles}, async(p)=>{
    await p.getByRole('button',{name:'Cài đặt',exact:true}).click();
    const row=p.getByText('Soniox công việc',{exact:true}).locator('..').locator('..');
    await row.getByRole('button',{name:'Xóa',exact:true}).click();
    await p.waitForFunction(()=>!document.querySelector('.task-assign-grid select option[value="prof-soniox"]'));
    await p.getByRole('button',{name:'Cuộc họp mới',exact:true}).click();
    assert.equal(await p.getByLabel('Cách chép lời').inputValue(),'local');
  });
  await check("Caption presentation controls edit and persist a named template",{profiles:mockProfiles},async p=>{
    await p.getByRole('button',{name:'Mẫu cấu hình',exact:true}).click();await p.getByRole('button',{name:'Tạo mẫu',exact:true}).click();
    await p.getByLabel('Tên mẫu',{exact:true}).fill('Chữ song ngữ');await p.getByLabel('Song ngữ',{exact:true}).check();await p.getByLabel('Cỡ chữ phụ đề').fill('32');await p.getByLabel('Cỡ chữ phụ đề').dispatchEvent('input');await p.getByLabel('Căn chữ phụ đề').selectOption('left');
    await p.getByRole('button',{name:'Lưu mẫu',exact:true}).click();await p.reload();await p.getByRole('button',{name:'Mẫu cấu hình',exact:true}).click();await p.locator('.preset-row').filter({hasText:'Chữ song ngữ'}).getByRole('button',{name:'Sửa',exact:true}).click();
    assert.equal(await p.getByLabel('Cỡ chữ phụ đề').inputValue(),'32');assert(await p.getByLabel('Song ngữ',{exact:true}).isChecked());assert.equal(await p.getByLabel('Căn chữ phụ đề').inputValue(),'left');
  });
  await check("Settings offers bundled Moonshine and only downloads optional Nemotron",{missingModels:true},async p=>{
    await p.getByRole('button',{name:'Mẫu cấu hình',exact:true}).click();
    assert.equal(await p.locator('.model-row').filter({hasText:/PhoWhisper/}).count(),0);
    for(const name of ['Moonshine · Tiếng Việt · Có sẵn','Moonshine · English · Có sẵn']) {
      const row=p.locator('.model-row').filter({hasText:name});
      assert((await row.innerText()).includes('Có sẵn trong Innote'));
      assert.equal(await row.getByRole('button',{name:'Tải model'}).count(),0);
    }
    await p.locator('.model-row').filter({hasText:'Nemotron'}).getByRole('button',{name:'Tải model'}).click();
    await p.waitForFunction(()=>window.qa.calls.some(c=>c.cmd==='download_model'));
    assert.equal(await p.evaluate(()=>window.qa.calls.find(c=>c.cmd==='download_model').args.id),'nemotron-3.5-q8');
  });
  await check("Fresh install starts Vietnamese and English using bundled models without optional downloads",{missingModels:true,profiles:[]},async p=>{
    for(const language of ['vi','en']) {
      await p.getByLabel('Ngôn ngữ',{exact:true}).selectOption(language);
      assert(await p.getByRole('button',{name:'Bắt đầu ghi',exact:true}).isEnabled());
      assert.equal(await p.getByRole('button',{name:/Tải ·/}).count(),0);
    }
    await p.getByRole('button',{name:'Bắt đầu ghi',exact:true}).click();
    await p.waitForFunction(()=>window.qa.calls.some(c=>c.cmd==='start'));
    const options=await p.evaluate(()=>window.qa.calls.find(c=>c.cmd==='start').args.options);
    assert.equal(options.model,'moonshine-en');
    assert(!await p.evaluate(()=>window.qa.calls.some(c=>c.cmd==='download_model')));
  });
  await check("Old local caption preferences migrate without changing historical transcripts",{preferences:{system:'speaker',language:'vi',model:'phowhisper-small'},standalone:{profileId:'__local__',model:'phowhisper-small',translate:false,sourceLang:'vi'},profiles:[]},async p=>{
    await p.getByRole('button',{name:'Chỉ phụ đề',exact:true}).click();
    assert.equal(await p.getByLabel('Model phụ đề').inputValue(),'moonshine-vi');
    assert.equal(await p.getByLabel('Model phụ đề').locator('option[value="phowhisper-small"]').count(),0);
    const saved=await p.evaluate(()=>JSON.parse(localStorage.getItem('may.caption.standalone')));
    assert.equal(saved.model,'moonshine-vi');
    assert.equal(await p.evaluate(()=>window.qa.calls.filter(c=>['save_segment','delete_session'].includes(c.cmd)).length),0);
  });
  await check(
    "Delayed audio response cannot attach a previous meeting recording",
    { delayAudio: true },
    async (p) => {
      await openMeeting(p, "Cuộc họp Alpha");
      await openMeeting(p, "Cuộc họp Beta");
      await p.waitForTimeout(700);
      assert(
        (await p.getByLabel("Nghe lại cuộc họp").getAttribute("src")).includes(
          ids[1],
        ),
      );
    },
  );
  await check(
    "An active recording remains reachable from settings and other meetings",
    { active: true },
    async (p) => {
      await p.getByRole("button", { name: "Cài đặt", exact: true }).click();
      await p.getByRole("button", { name: "Về phiên đang chạy" }).click();
      await p.getByRole("button", { name: "Kết thúc", exact: true }).waitFor();
      await openMeeting(p, "Cuộc họp Alpha");
      await p.getByRole("button", { name: "Về phiên đang chạy" }).waitFor();
    },
  );
  await check(
    "Export waits for an in-flight notes save",
    { slowSave: true },
    async (p) => {
      await openMeeting(p, "Cuộc họp Alpha");
      await p.getByRole("button", { name: "Mở ghi chú" }).click();
      await p
        .getByRole("textbox", { name: "Ghi chú của bạn", exact: true })
        .fill("Ghi chú vừa cập nhật");
      await p.getByRole("button", { name: "Tải .md", exact: true }).click();
      await p.waitForFunction(() => window.qa.exports.length > 0);
      assert.equal(
        await p.evaluate(() => window.qa.exports[0].notes),
        "Ghi chú vừa cập nhật",
      );
    },
  );
  await check(
    "Unsaved transcript edits are protected when changing meetings",
    {},
    async (p) => {
      await openMeeting(p, "Cuộc họp Alpha");
      await p.getByRole("button", { name: "Sửa đoạn 00:01" }).click();
      await p
        .getByRole("textbox", { name: "Sửa đoạn chép lời" })
        .fill("Bản nháp cần giữ");
      await openMeeting(p, "Cuộc họp Beta");
      assert.equal(
        await p
          .getByRole("textbox", { name: "Sửa đoạn chép lời" })
          .inputValue(),
        "Bản nháp cần giữ",
      );
      await p.getByRole("button", { name: "Cài đặt", exact: true }).click();
      assert.equal(
        await p
          .getByRole("textbox", { name: "Sửa đoạn chép lời" })
          .inputValue(),
        "Bản nháp cần giữ",
      );
    },
  );
  await check(
    "No selected sources blocks start and audio test and preserves preferences after reload",
    {
      preferences: {
        system: "",
        microphone: "",
        model: "base-q5_1",
        transcribe: false,
      },
    },
    async (p) => {
      assert(await p.getByRole("button", { name: "Bắt đầu ghi" }).isDisabled());
      assert(
        await p
          .getByRole("button", { name: "Kiểm tra nguồn đã chọn · 5 giây" })
          .isDisabled(),
      );
      await p.reload();
      await p
        .getByRole("button", { name: "Danh sách cuộc họp", exact: true })
        .waitFor();
      assert.equal(
        await p
          .getByRole("checkbox", { name: "Chép lời khi đang họp" })
          .isChecked(),
        false,
      );
      assert.equal(
        await p.getByLabel("Thiết bị phát âm thanh cuộc họp").inputValue(),
        "",
      );
    },
  );
  await check(
    "A missing endpoint has a clear recovery path",
    {
      preferences: {
        system: "old-machine-speaker",
        microphone: "",
        transcribe: false,
      },
    },
    async (p) => {
      assert(await p.getByRole("button", { name: "Bắt đầu ghi" }).isDisabled());
      await p
        .getByText("Thiết bị đã chọn không còn khả dụng.", { exact: false })
        .waitFor();
      await p
        .getByLabel("Thiết bị phát âm thanh cuộc họp")
        .selectOption("speaker");
      assert(await p.getByRole("button", { name: "Bắt đầu ghi" }).isEnabled());
    },
  );
  await check(
    "Recording without a downloaded model remains available",
    { missingModels: true },
    async (p) => {
      assert(await p.getByRole("button", { name: "Bắt đầu ghi" }).isEnabled());
      await p
        .getByRole("checkbox", { name: "Chép lời khi đang họp" })
        .uncheck();
      await p.getByText("Chỉ thu âm.", { exact: false }).waitFor();
      await p.getByRole("button", { name: "Bắt đầu ghi" }).click();
      await p.getByRole("button", { name: "Kết thúc", exact: true }).waitFor();
      assert.equal(
        await p.evaluate(
          () =>
            window.qa.calls.find((c) => c.cmd === "start").args.options
              .transcribe,
        ),
        false,
      );
      await p.getByRole("button", { name: "Tạm dừng", exact: true }).click();
      await p.getByRole("button", { name: "Tiếp tục", exact: true }).waitFor();
    },
  );
  await check(
    "Silence during test gives guidance without claiming device failure",
    {},
    async (p) => {
      await p
        .getByRole("button", { name: "Kiểm tra nguồn đã chọn · 5 giây" })
        .click();
      await p.getByText("Chưa thấy tín hiệu từ", { exact: false }).waitFor();
      assert(await p.getByLabel("Nghe bản thu thử").isVisible());
    },
  );
  await check(
    "Failed start reports the error and releases busy state",
    { failStart: true },
    async (p) => {
      await p.getByRole("button", { name: "Bắt đầu ghi" }).click();
      await p
        .getByRole("alert")
        .filter({ hasText: "Không mở được thiết bị" })
        .waitFor();
      assert(await p.getByRole("button", { name: "Bắt đầu ghi" }).isEnabled());
    },
  );
  await check("Empty title restores the saved title", {}, async (p) => {
    await openMeeting(p, "Cuộc họp Alpha");
    await p.getByRole("textbox", { name: "Tên cuộc họp" }).fill("   ");
    await p.getByRole("textbox", { name: "Tên cuộc họp" }).press("Tab");
    assert.equal(
      await p.getByRole("textbox", { name: "Tên cuộc họp" }).inputValue(),
      "Cuộc họp Alpha",
    );
  });
  await check(
    "Delete confirmation can be cancelled with Escape and retains the session",
    {},
    async (p) => {
      await openMeeting(p, "Cuộc họp Alpha");
      await p
        .getByRole("button", { name: "Xóa cuộc họp", exact: true })
        .click();
      await p.getByRole("dialog").waitFor();
      await p.keyboard.press("Escape");
      await p.getByRole("dialog").waitFor({ state: "hidden" });
      assert(
        await p.getByRole("textbox", { name: "Tên cuộc họp" }).isVisible(),
      );
      assert.equal(
        await p.evaluate(
          () =>
            window.qa.calls.filter((c) => c.cmd === "delete_session").length,
        ),
        0,
      );
    },
  );
  await check(
    "Corrupt preference types cannot crash the setup screen",
    {
      preferences: {
        system: { broken: true },
        microphone: 25,
        model: 42,
        transcribe: "false",
        language: "unsupported",
      },
    },
    async (p) => {
      assert(await p.getByRole("button", { name: "Bắt đầu ghi" }).isVisible());
      assert.equal(
        await p
          .getByRole("checkbox", { name: "Chép lời khi đang họp" })
          .isChecked(),
        true,
      );
    },
  );
await check(
    "AI profile add requires a successful connection test before saving",
    { viewport: { width: 1180, height: 800 }, profiles: mockProfiles },
    async (p) => {
      await p.getByRole("button", { name: "Cài đặt", exact: true }).click();
      await p.getByRole("button", { name: "Thêm cấu hình", exact: true }).click();
      const dialog = p.getByRole("dialog", { name: "Thêm cấu hình" });
      await dialog.getByLabel("Tên cấu hình").fill("Cấu hình test");
      await dialog.getByLabel("API key").fill("sk-test-1234");
      const save = dialog.getByRole("button", { name: "Thêm cấu hình", exact: true });
      assert(!(await save.isEnabled()), "save must be disabled before a successful test");
      await dialog.getByRole("button", { name: "Kiểm tra kết nối", exact: true }).click();
      await p.waitForFunction(() =>
        window.qa.calls.some((c) => c.cmd === "test_ai_connection"),
      );
      assert(await save.isEnabled(), "save enabled after a successful test");
      await dialog.screenshot({path:join(output,'verified-profile-dialog.png')});
      await save.click();
      await p.waitForFunction(() =>
        window.qa.calls.some((c) => c.cmd === "save_ai_profile"),
      );
      await p.screenshot({ path: join(output, "ai-profile-add-1180x800.png") });
    },
  );

  await check("Adding Soniox updates every eligible selector immediately; deleting removes it without reload",
    { profiles: [mockProfiles[1]] }, async (p) => {
      await p.getByRole("button", {name:"Cài đặt",exact:true}).click();
      await p.getByRole("button", {name:"Thêm cấu hình",exact:true}).click();
      const d=p.getByRole("dialog",{name:"Thêm cấu hình"});
      await d.getByLabel("Tên cấu hình").fill("Soniox added during this session");
      await d.getByLabel("Nhà cung cấp",{exact:true}).selectOption("soniox");
      await d.getByLabel("API key").fill("fake-soniox-key");
      await d.getByRole("button",{name:"Kiểm tra kết nối",exact:true}).click();
      const save=d.getByRole("button",{name:"Thêm cấu hình",exact:true});
      await p.waitForFunction(()=>!document.querySelector(".profile-fields")?.disabled);
      await save.click();
      await p.getByRole('button',{name:'Mẫu cấu hình',exact:true}).click();await p.getByRole('button',{name:'Tạo mẫu',exact:true}).click();
      const caption=p.getByLabel('Phụ đề trực tiếp',{exact:true});
      await caption.locator('option[value="prof-new"]').waitFor({state:"attached"});
      await caption.selectOption("prof-new");
      assert.equal(await p.getByLabel("Model phụ đề").inputValue(),"stt-rt-v6");
      const transcription=p.locator("label",{hasText:"Chép lời (transcription)"}).locator("select").first();
      assert.equal(await transcription.locator('option[value="prof-new"]').count(),1);
      await p.getByRole("button",{name:"Cài đặt",exact:true}).click();
      const card=p.locator(".ai-profile-card").filter({hasText:"Soniox added during this session"});
      // Select by the visible profile row, independently of row CSS naming.
      const row=p.getByText("Soniox added during this session",{exact:true}).locator("..").locator("..");
      await row.getByRole("button",{name:"Xóa",exact:true}).click();
      await p.getByRole('button',{name:'Mẫu cấu hình',exact:true}).click();await p.getByRole('button',{name:'Tạo mẫu',exact:true}).click();
      assert.equal(await transcription.locator('option[value="prof-new"]').count(),0);
    });

  await check("Invalid key shows an inline error and never enables save", { profiles: mockProfiles }, async (p) => {
    await p.getByRole("button", { name: "Cài đặt", exact: true }).click();
    await p.getByRole("button", { name: "Thêm cấu hình", exact: true }).click();
    const d = p.getByRole("dialog", { name: "Thêm cấu hình" });
    await d.getByLabel("Tên cấu hình").fill("Invalid test");
    await d.getByLabel("Nhà cung cấp", { exact: true }).selectOption("soniox");
    await d.getByLabel("API key").fill("invalid-key");
    await d.getByRole("button", { name: "Kiểm tra kết nối", exact: true }).click();
    await d.getByRole("status").filter({ hasText: "401" }).waitFor();
    assert(!(await d.getByRole("button", { name: "Thêm cấu hình", exact: true }).isEnabled()));
    assert(!(await p.evaluate(() => window.qa.calls.some(c => c.cmd === "save_ai_profile"))));
    await p.screenshot({ path: join(output, "api-invalid-inline.png") });
  });

  await check("Connection fields lock while testing and URL/provider changes invalidate verification", { profiles: mockProfiles, delayAiTest: true }, async (p) => {
    await p.getByRole("button", { name: "Cài đặt", exact: true }).click();
    await p.getByRole("button", { name: "Thêm cấu hình", exact: true }).click();
    const d = p.getByRole("dialog", { name: "Thêm cấu hình" });
    await d.getByLabel("Nhà cung cấp",{exact:true}).selectOption("openai-compatible");
    await d.getByLabel("Tên cấu hình").fill("Test scopes");
    await d.getByLabel("API key").fill("fake-key");
    const save = d.getByRole("button", { name: "Thêm cấu hình", exact: true });
    await d.getByRole("button", { name: "Kiểm tra kết nối", exact: true }).click();
    assert(!(await d.getByLabel("API key").isEnabled()));
    await save.waitFor({ state: "visible" });
    await p.waitForFunction(() => !document.querySelector(".profile-fields")?.disabled);
    assert(await save.isEnabled());
    await d.getByLabel("Base URL", { exact: true }).fill("https://proxy.example/v1");
    assert(!(await save.isEnabled()));
    await d.getByRole("button", { name: "Kiểm tra kết nối", exact: true }).click();
    await p.waitForFunction(() => !document.querySelector(".profile-fields")?.disabled);
    assert(await save.isEnabled());
    await d.getByLabel("Nhà cung cấp", { exact: true }).selectOption("gemini");
    assert(!(await save.isEnabled()));
  });

  await check(
    "Editing an AI profile requires a fresh test after the key changes",
    { viewport: { width: 1180, height: 800 }, profiles: mockProfiles },
    async (p) => {
      await p.getByRole("button", { name: "Cài đặt", exact: true }).click();
      await p.getByRole("button", { name: "Sửa", exact: true }).first().click();
      const dialog = p.getByRole("dialog", { name: "Sửa" });
      const save = dialog.getByRole("button", { name: "Lưu cấu hình", exact: true });
      assert(!(await save.isEnabled()), "save disabled until a fresh test");
      await dialog.getByLabel("API key").fill("sk-different-9999");
      assert(!(await save.isEnabled()), "changing the key stays unsaved");
      await dialog.getByRole("button", { name: "Kiểm tra kết nối", exact: true }).click();
      await p.waitForFunction(() =>
        window.qa.calls.some((c) => c.cmd === "test_ai_connection"),
      );
      assert(await save.isEnabled(), "save enabled after the fresh test");
    },
  );

  await check(
    "Task assignment persists separately for meeting and caption-only modes",
    { viewport: { width: 1180, height: 800 }, profiles: mockProfiles },
    async (p) => {
      await p.getByRole('button',{name:'Mẫu cấu hình',exact:true}).click();
      await p.getByRole('button',{name:'Tạo mẫu',exact:true}).click();
      await p.getByLabel('Tên mẫu',{exact:true}).fill('Mẫu họp');
      await p.locator('label',{hasText:'Chép lời (transcription)'}).locator('select').first().selectOption('__local__');
      await p.getByRole('button',{name:'Lưu mẫu',exact:true}).click();
      await p.getByRole('tab',{name:'Phụ đề',exact:true}).click();
      assert.equal(await p.locator('.preset-row').filter({hasText:'Mẫu họp'}).count(),0);
      await p.getByRole('tab',{name:'Cuộc họp',exact:true}).click();
      await p.locator('.preset-row').filter({hasText:'Mẫu họp'}).waitFor();
      await p.screenshot({ path: join(output, "task-assign-1180x800.png") });
    },
  );

  await check(
    "Caption-only page starts and stops a standalone caption session",
    { viewport: { width: 1180, height: 800 }, profiles: mockProfiles },
    async (p) => {
      await p.getByRole("button", { name: "Chỉ phụ đề", exact: true }).click();
      await p.getByLabel("Cấu hình AI phụ đề").selectOption("prof-soniox");
      await p.getByLabel("Model phụ đề").selectOption("stt-rt-v6");
      await p.getByRole("button", { name: "Bắt đầu phụ đề", exact: true }).click();
      await p.waitForFunction(() =>
        window.qa.calls.some((c) => c.cmd === "start_caption"),
      );
      assert.equal(await p.evaluate(() => window.qa?.caption?.options?.mode), "standalone");
      assert(
        await p.evaluate(() => window.qa.calls.some((c) => c.cmd === "open_caption_window")),
        "caption window opens",
      );
      await p.getByRole("button", { name: "Dừng phụ đề", exact: true }).click();
      await p.waitForFunction(() =>
        window.qa.calls.some((c) => c.cmd === "stop_caption"),
      );
      await p.screenshot({ path: join(output, "caption-page-1180x800.png") });
    },
  );

  await check("Caption page only offers dedicated realtime models and tests model access", { profiles: mockProfiles }, async (p) => {
    await p.getByRole("button", { name:"Chỉ phụ đề", exact:true }).click();
    await p.getByLabel("Dịch sang",{exact:true}).check();
    for (const [id, model] of [["prof-soniox","stt-rt-v6"],["prof-openai","gpt-realtime-translate"],["prof-gemini","gemini-3.5-live-translate-preview"]]) {
      await p.getByLabel("Cấu hình AI phụ đề").selectOption(id);
      assert.equal(await p.getByLabel("Model phụ đề").inputValue(),model);
      await p.getByRole("button",{name:"Kiểm tra model",exact:true}).click();
      await p.waitForFunction(m=>window.qa.calls.some(c=>c.cmd==="test_ai_connection"&&c.args.input.model===m),model);
    }
    await p.getByLabel("Ngôn ngữ dịch đích").selectOption("en");
    await p.reload();
    await p.getByRole("button", { name:"Chỉ phụ đề", exact:true }).click();
    assert.equal(await p.getByLabel("Cấu hình AI phụ đề").inputValue(),"prof-gemini");
    assert.equal(await p.getByLabel("Model phụ đề").inputValue(),"gemini-3.5-live-translate-preview");
    assert.equal(await p.getByLabel("Ngôn ngữ dịch đích").inputValue(),"en");
  });

  await check("Caption visibility starts off, previews without upload, auto-shows and hides independently", {profiles:mockProfiles}, async(p)=>{
    await p.getByRole('button',{name:'Chỉ phụ đề',exact:true}).click();
    const show=p.getByRole('button',{name:'Hiện hộp phụ đề',exact:true});
    assert.equal(await show.getAttribute('aria-pressed'),'false');
    await show.click();
    await p.getByRole('button',{name:'Ẩn hộp phụ đề',exact:true}).waitFor();
    assert(!await p.evaluate(()=>window.qa.calls.some(c=>c.cmd==='start_caption')));
    await p.getByRole('button',{name:'Ẩn hộp phụ đề',exact:true}).click();
    await show.waitFor();
    await p.getByLabel('Cấu hình AI phụ đề').selectOption('prof-soniox');
    await p.getByRole('button',{name:'Bắt đầu phụ đề',exact:true}).click();
    await p.getByRole('button',{name:'Ẩn hộp phụ đề',exact:true}).waitFor();
    await p.getByRole('button',{name:'Ẩn hộp phụ đề',exact:true}).click();
    await show.waitFor();
    assert(await p.evaluate(()=>window.qa.caption.running));
    assert(!await p.evaluate(()=>window.qa.calls.some(c=>c.cmd==='stop_caption')));
    await show.click();
    await p.getByRole('button',{name:'Dừng phụ đề',exact:true}).click();
    await show.waitFor();
    assert.equal(await p.evaluate(()=>window.qa.captionVisible),false);
  });

  for (const viewport of [{ width: 900, height: 640 }, { width: 1180, height: 800 }]) {
    await check(`Caption form spacing and running controls ${viewport.width}x${viewport.height}`, { viewport, profiles: mockProfiles }, async (p) => {
      await p.getByRole("button", { name: "Chỉ phụ đề", exact: true }).click();
      assert(await p.getByLabel("Model phụ đề").isDisabled(), "No profile means no model to select");
      const layout = await p.locator(".caption-mode-row label").evaluateAll(labels => labels.map(label => {
        const range = document.createRange(); range.selectNodeContents(label.firstChild);
        const a = range.getBoundingClientRect(), b = label.querySelector("select").getBoundingClientRect();
        return { gap: b.top-a.bottom, width: b.width, right: b.right };
      }));
      assert(layout.every(l => l.gap >= 8 && l.width >= 180 && l.right <= viewport.width), JSON.stringify(layout));
      assert(await p.locator(".caption-page").evaluate(el => el.scrollWidth <= el.clientWidth), "No horizontal scroll");
      await p.getByLabel("Cấu hình AI phụ đề").selectOption("prof-soniox");
      await p.getByRole("button", { name: "Bắt đầu phụ đề", exact: true }).click();
      await p.getByRole("button", { name: "Dừng phụ đề", exact: true }).waitFor();
      for (const label of ["Thiết bị loa/tai nghe", "Cấu hình AI phụ đề", "Model phụ đề", "Ngôn ngữ gốc"]) {
        assert(await p.getByLabel(label).isDisabled(), `${label} cannot silently change the running stream`);
      }
      await p.getByRole("button", { name: "Dừng phụ đề", exact: true }).click();
      await p.getByRole("button", { name: "Bắt đầu phụ đề", exact: true }).waitFor();
      await p.screenshot({ path: join(output, `caption-spaced-${viewport.width}x${viewport.height}.png`) });
    });
  }

  await check("Stopping captions waits for the backend worker before offering restart", { profiles: mockProfiles, delayCaptionStop: true }, async (p) => {
    await p.getByRole('button',{name:'Chỉ phụ đề',exact:true}).click();
    await p.getByLabel('Cấu hình AI phụ đề').selectOption('prof-soniox');
    await p.getByRole('button',{name:'Bắt đầu phụ đề',exact:true}).click();
    await p.getByRole('button',{name:'Dừng phụ đề',exact:true}).click();
    assert.equal(await p.getByRole('button',{name:'Bắt đầu phụ đề',exact:true}).count(),0);
    await p.getByRole('button',{name:'Bắt đầu phụ đề',exact:true}).waitFor();
    assert.equal(await p.evaluate(()=>window.qa.caption.running),false);
  });

  await check(
    "Meeting caption toggle starts a shared-meeting stream and stops cleanly",
    {
      active: true,
      profiles: mockProfiles,
      taskAssignMeeting: { caption: { profileId: "prof-soniox", model: "stt-rt-v6", targetLang: "vi" } },
      viewport: { width: 1180, height: 800 },
    },
    async (p) => {
      await p.getByRole("button", { name: "Về phiên đang chạy", exact: true }).click();
      await p.locator(".record-controls").getByRole("button", { name: "Phụ đề", exact: true }).click();
      await p.waitForFunction(() =>
        window.qa.calls.some((c) => c.cmd === "start_caption" && c.args.options.mode === "meeting"),
      );
      assert.equal(await p.evaluate(() => window.qa?.caption?.options?.mode), "meeting");
      await p.getByRole("button", { name: "Tắt phụ đề", exact: true }).click();
      await p.waitForFunction(() =>
        window.qa.calls.some((c) => c.cmd === "stop_caption"),
      );
      await p.screenshot({ path: join(output, "meeting-caption-toggle-1180x800.png") });
    },
  );

  for (const viewport of [
    { width: 900, height: 640 },
    { width: 1180, height: 800 },
    { width: 900, height: 640 },
  ]) {
    const notes = results.some((r) => r.name === "Layout 900x640 notes=false");
    await check(
      `Layout ${viewport.width}x${viewport.height} notes=${notes}`,
      { viewport },
      async (p) => {
        await openMeeting(p, "Cuộc họp Alpha");
        if (notes) await p.getByRole("button", { name: "Mở ghi chú" }).click();
        await p.waitForTimeout(50);
        assert(
          await p.evaluate(
            () => document.documentElement.scrollWidth <= innerWidth,
          ),
          "Horizontal overflow",
        );
        const box = await p
          .getByRole("button", { name: "Tải .md", exact: true })
          .boundingBox();
        assert(
          box && box.y >= 0 && box.y + box.height <= viewport.height,
          "Export below viewport",
        );
        await p.screenshot({
          path: join(
            output,
            `layout-${viewport.width}x${viewport.height}-${notes}.png`,
          ),
        });
      },
    );
  }
} finally {
  await browser.close();
}
