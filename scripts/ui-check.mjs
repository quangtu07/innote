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
      language: "vi",
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
      window.qa.snapshot.sessions[0].speech={model:'stt-rt-v5',profile_id:'prof-soniox',translation_language:'en',connections:1,uploaded_samples:160000,local_fallback:false};
      window.qa.snapshot.sessions[0].translations=[{id:'translated-one',source_id:'segment-one',start:1,end:4,original:segment.text,text:'English translated reference.',language:'en',provider:'soniox'}];
    }
    if (options.active) {
      window.qa.snapshot.active_id = ids[2];
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
    if(options.standalone)localStorage.setItem("may.caption.standalone",JSON.stringify(options.standalone));
    if (options.preferences)
      localStorage.setItem(
        "may.preferences",
        JSON.stringify(options.preferences),
      );
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
        if (cmd === "smooth_transcript") {
          const s = qa.snapshot.sessions.find((s) => s.id === args.id);
          s.smoothed = {
            text: s.segments.map((x) => x.text).join("\n") + " (đã làm mịn)",
            provider: args.provider,
            model: args.model,
            prompt: args.prompt,
            created: "2026-10-30T00:00:00Z",
          };
          return structuredClone(s);
        }
        if (cmd === "summarize_transcript") {
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
          const s = {
            ...make(ids[2], args.options.title || "Cuộc họp mới", ""),
            language: args.options.language,
            model: args.options.model,
            status: "recording",
            duration: 0,
            processed: 0,
            segments: [],
            provider:
              args.options.provider === "soniox" &&
              (qa.snapshot.soniox_configured || qa.profiles.some(p=>p.id===args.options.profileId && p.provider==="soniox"))
                ? "soniox"
                : "local",
            warnings:
              args.options.provider === "soniox" &&
              !qa.snapshot.soniox_configured && !qa.profiles.some(p=>p.id===args.options.profileId && p.provider==="soniox")
                ? [
                    "Chưa có key Soniox. Phiên này dùng model local miễn phí; không gửi audio lên Soniox.",
                  ]
                : [],
          };
          if(args.options.provider==='soniox')s.speech={model:'stt-rt-v5',profile_id:args.options.profileId,translation_language:args.options.translationLanguage??null,connections:1,uploaded_samples:0,local_fallback:false};
          if(args.options.translationLanguage || args.options.captionsEnabled)qa.caption={running:true,options:{mode:'meeting'}};
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
    await p.getByRole('button',{name:'Phụ đề',exact:true}).click();
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
    await p.getByRole('button',{name:'Phụ đề',exact:true}).click();
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
    assert(await p.getByText(/Your meetings/).isVisible());
    await p.screenshot({path:join(output,"language-english-900x640.png")});
  });
  await check("Settings switches the interface between Korean and Chinese", {viewport:{width:900,height:640}}, async p=>{
    await p.getByRole("button",{name:"Cài đặt",exact:true}).click();
    const localeSelect = p.locator("select").first();
    await localeSelect.selectOption("ko");
    await p.getByRole("button",{name:"새 회의",exact:true}).waitFor();
    assert(await p.getByText(/내 회의/).isVisible());
    await p.screenshot({path:join(output,"language-korean-900x640.png")});
    await localeSelect.selectOption("zh");
    await p.getByRole("button",{name:"新建会议",exact:true}).waitFor();
    assert(await p.getByText(/你的会议/).isVisible());
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
  await check(
    "AI smoothing keeps the original transcript and re-runs with an optional prompt",
    {profiles:mockProfiles, taskAssignMeeting:{smooth:{profileId:"prof-openai",model:"gpt-4o-mini"}}},
    async (p) => {
      await openMeeting(p, "Cuộc họp Alpha");


      await p.getByRole("button", { name: "Làm mịn", exact: true }).click();
      const dialog = p.getByRole("dialog");
      if (
        await dialog
          .getByRole("button", { name: "Kiểm tra API", exact: true })
          .isVisible()
          .catch(() => false)
      ) {
        await dialog.getByRole("button", { name: "Kiểm tra API", exact: true }).click();
      }
      await dialog.getByRole("button", { name: "Làm mịn", exact: true }).click();
      const smoothTab = p.getByRole("tab", { name: "Bản làm mịn", exact: true });
      await p
        .getByText("Nội dung thử tiếng Việt. (đã làm mịn)", { exact: true })
        .waitFor();
      await p.screenshot({ path: join(output, "ai-smoothing.png") });
      assert(
        await smoothTab.getAttribute("aria-selected") === "true",
      );
      await p.getByRole("tabpanel", { name: "Bản làm mịn bằng AI" })
        .getByRole("button", { name: "Về Bản gốc", exact: true })
        .click();
      assert(
        await p.getByText("Nội dung thử tiếng Việt.", { exact: true }).isVisible(),
      );
      assert.equal(await p.getByText("Nội dung thử tiếng Việt. (đã làm mịn)", { exact: true }).count(), 0);
      await smoothTab.click();
      await p
        .getByText("Nội dung thử tiếng Việt. (đã làm mịn)", { exact: true })
        .waitFor();
      await p.getByRole("button", { name: "Tải .txt", exact: true }).click();
      await p.waitForFunction(() =>
        window.qa.calls.some((c) => c.cmd === "export_session"),
      );
      assert.equal(
        await p.evaluate(
          () =>
            window.qa.calls.find((c) => c.cmd === "export_session").args
              .version,
        ),
        "smoothed",
      );
      await p
        .getByRole("button", { name: "Làm mịn lại", exact: true })
        .click();
      const rerunDialog = p.getByRole("dialog");
      await rerunDialog
        .getByRole("textbox", { name: "Yêu cầu thêm (không bắt buộc)" })
        .fill("Giữ nguyên tên riêng");
      await rerunDialog
        .getByRole("button", { name: "Làm mịn lại", exact: true })
        .click();
      await p.waitForFunction(
        () =>
          window.qa.calls.filter((c) => c.cmd === "smooth_transcript").length ===
          2,
      );
      const calls = await p.evaluate(() =>
        window.qa.calls
          .filter((c) => c.cmd === "smooth_transcript")
          .map((c) => c.args.prompt),
      );
      assert.deepEqual(calls, ["", "Giữ nguyên tên riêng"]);
    },
  );
  await check(
    "Meeting summary dialog generates, switches tabs, and renders structured content",
    {profiles:mockProfiles, taskAssignMeeting:{summary:{profileId:"prof-openai",model:"gpt-4o-mini"}}},
    async (p) => {
      await openMeeting(p, "Cuộc họp Alpha");


      await p.getByRole("button", { name: "Tóm tắt", exact: true }).click();
      const dialog = p.locator("dialog.summary-dialog");
      await dialog.waitFor();
      assert(await dialog.isVisible());

      await dialog.getByRole("button", { name: "Kiểm tra API", exact: true }).click();
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
  await check("Meeting shared model selection synchronizes transcription in both directions", {profiles:[...mockProfiles,{...mockProfiles.find(p=>p.provider==='soniox'),id:'prof-soniox2',name:'Soniox 2'}]},async p=>{
    await p.getByLabel('Bật phụ đề khi bắt đầu cuộc họp').check();await p.getByLabel('Dịch sang trong cuộc họp').check();assert(await p.getByRole('button',{name:'Bắt đầu ghi',exact:true}).isDisabled());
    const shared=p.getByLabel('Model dùng chung cho cuộc họp');const choice={profileId:'prof-soniox2',model:'stt-rt-v5'};
    const labels=await shared.locator('option').allTextContents();assert(labels.includes('Soniox 2/stt-rt-v5'));assert(!labels.some(s=>s.includes('gpt-')||s.includes('gemini')||s.includes('Local/')));
    await shared.selectOption(JSON.stringify(choice));assert.equal(await p.getByLabel('Cách chép lời',{exact:true}).inputValue(),'soniox');assert.equal(await p.getByLabel('Cấu hình Soniox',{exact:true}).inputValue(),'prof-soniox2');assert((await p.getByLabel('Model chép lời Soniox').innerText()).includes('Soniox 2/stt-rt-v5'));
    await p.getByLabel('Cấu hình Soniox',{exact:true}).selectOption('prof-soniox');assert.equal(await shared.inputValue(),JSON.stringify({profileId:'prof-soniox',model:'stt-rt-v5'}));
    await shared.selectOption(JSON.stringify(choice));await p.getByLabel('Ngôn ngữ bản dịch',{exact:true}).selectOption('ko');await shared.scrollIntoViewIfNeeded();await p.screenshot({path:join(output,'meeting-shared-model.png')});
    await p.getByRole('button',{name:'Bắt đầu ghi',exact:true}).click();await p.waitForFunction(()=>window.qa.calls.some(c=>c.cmd==='start'));const calls=await p.evaluate(()=>window.qa.calls),o=calls.find(c=>c.cmd==='start').args.options;assert.equal(o.provider,'soniox');assert.equal(o.profileId,'prof-soniox2');assert.equal(o.translationLanguage,'ko');assert.equal(o.captionsEnabled,true);assert(!calls.some(c=>c.cmd==='start_caption'));
  });
  await check("Meeting shared original captions allow local and block translation after switching back",{profiles:mockProfiles},async p=>{
    await p.getByLabel('Ngôn ngữ',{exact:true}).selectOption('vi');await p.getByLabel('Bật phụ đề khi bắt đầu cuộc họp').check();const shared=p.getByLabel('Model dùng chung cho cuộc họp');assert.equal(await shared.inputValue(),JSON.stringify({profileId:'__local__',model:'moonshine-vi'}));
    await shared.selectOption(JSON.stringify({profileId:'prof-soniox',model:'stt-rt-v5'}));await shared.selectOption(JSON.stringify({profileId:'__local__',model:'moonshine-vi'}));assert.equal(await p.getByLabel('Cách chép lời',{exact:true}).inputValue(),'local');
    await p.getByLabel('Dịch sang trong cuộc họp').check();assert(await p.getByRole('button',{name:'Bắt đầu ghi',exact:true}).isDisabled());await shared.selectOption(JSON.stringify({profileId:'prof-soniox',model:'stt-rt-v5'}));await p.getByLabel('Cách chép lời',{exact:true}).selectOption('local');assert.equal(await shared.inputValue(),'');assert(await p.getByRole('button',{name:'Bắt đầu ghi',exact:true}).isDisabled());
  });
  await check("Soniox starts transcription-only without local downloads, and translation is opt-in", {profiles:mockProfiles,missingModels:true}, async p=>{
    await p.getByLabel('Ngôn ngữ',{exact:true}).selectOption('multi');
    await p.getByLabel('Cách chép lời',{exact:true}).selectOption('soniox');
    await p.getByLabel('Cấu hình Soniox',{exact:true}).selectOption('prof-soniox');
    assert.equal(await p.getByLabel('Bật phụ đề khi bắt đầu cuộc họp').isChecked(),false);
    assert.equal(await p.getByLabel('Dịch sang trong cuộc họp').count(),0);
    assert.equal(await p.getByLabel('Dự phòng local khi Soniox lỗi').isChecked(),false);
    await p.getByRole('button',{name:'Bắt đầu ghi',exact:true}).click();
    const opts=await p.evaluate(()=>window.qa.calls.find(c=>c.cmd==='start').args.options);
    assert.equal(opts.provider,'soniox');assert.equal(opts.translationLanguage,null);assert.equal(opts.localFallback,false);
    assert(!await p.evaluate(()=>window.qa.calls.some(c=>c.cmd==='start_caption'||c.cmd==='open_caption_window')));
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
    await p.getByRole('button',{name:'Phụ đề',exact:true}).click();
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
    await p.getByRole('button',{name:'Phụ đề',exact:true}).click();await p.getByLabel('Ngôn ngữ gốc').selectOption('vi');await p.getByLabel('Cấu hình AI phụ đề').selectOption('__local__');await p.getByLabel('Model phụ đề').selectOption('moonshine-vi');
    assert.equal(await p.getByRole('button',{name:'Kiểm tra model',exact:true}).count(),0);
    await p.reload();await p.getByRole('button',{name:'Phụ đề',exact:true}).click();assert.equal(await p.getByLabel('Cấu hình AI phụ đề').inputValue(),'__local__');assert.equal(await p.getByLabel('Model phụ đề').inputValue(),'moonshine-vi');
    await p.getByRole('button',{name:'Bắt đầu phụ đề',exact:true}).click();await p.waitForFunction(()=>window.qa.calls.some(c=>c.cmd==='start_caption'));
    const calls=await p.evaluate(()=>window.qa.calls),o=calls.find(c=>c.cmd==='start_caption').args.options;assert.equal(o.profileId,'__local__');assert.equal(o.model,'moonshine-vi');assert.equal(o.translate,false);assert.equal(o.apiKey,'');assert.equal(o.fallback,null);assert(!calls.some(c=>c.cmd==='test_ai_connection'||c.cmd==='start'));
  });
  await check("Local caption models follow source language and cannot translate",{},async p=>{
    await p.getByRole('button',{name:'Phụ đề',exact:true}).click();await p.getByLabel('Ngôn ngữ gốc').selectOption('vi');await p.getByLabel('Cấu hình AI phụ đề').selectOption('__local__');assert.equal(await p.getByLabel('Model phụ đề').inputValue(),'moonshine-vi');
    await p.getByLabel('Ngôn ngữ gốc').selectOption('en');const moon=p.getByLabel('Model phụ đề').locator('option[value="moonshine-vi"]');await p.getByLabel('Model phụ đề').locator('option[value="moonshine-vi"]:disabled').waitFor({state:'attached'});assert((await moon.innerText()).includes('Đã tải'));assert(await moon.evaluate(o=>o.disabled));assert(await p.getByRole('button',{name:'Bắt đầu phụ đề',exact:true}).isDisabled());await p.getByLabel('Model phụ đề').selectOption('nemotron-3.5-q8');assert(await p.getByRole('button',{name:'Bắt đầu phụ đề',exact:true}).isEnabled());await p.getByLabel('Model phụ đề').scrollIntoViewIfNeeded();await p.screenshot({path:join(output,'local-installed-models-english.png')});
    await p.getByLabel('Ngôn ngữ gốc').selectOption('vi');await p.getByLabel('Model phụ đề').locator('option[value="moonshine-vi"]:not(:disabled)').waitFor({state:'attached'});assert(!await moon.evaluate(o=>o.disabled));await p.getByLabel('Model phụ đề').selectOption('moonshine-vi');assert(await p.getByRole('button',{name:'Bắt đầu phụ đề',exact:true}).isEnabled());
    await p.getByLabel('Dịch sang',{exact:true}).check();assert.equal(await p.getByLabel('Cấu hình AI phụ đề').locator('option[value="__local__"]').count(),0);assert(await p.getByRole('button',{name:'Bắt đầu phụ đề',exact:true}).isDisabled());
  });
  await check("Local caption model must be installed before starting",{missingBundled:true},async p=>{
    await p.getByRole('button',{name:'Phụ đề',exact:true}).click();await p.getByLabel('Ngôn ngữ gốc').selectOption('vi');await p.getByLabel('Cấu hình AI phụ đề').selectOption('__local__');assert(await p.getByRole('button',{name:'Bắt đầu phụ đề',exact:true}).isDisabled());await p.getByText('Thiếu model đi kèm. Hãy cài lại Innote.',{exact:false}).waitFor();
  });
  const groqProfile={id:'prof-groq',name:'Groq Free',provider:'groq',base_url:'',has_key:true,key_tail:'mock',created:'2026-11-03'};
  await check("Processing levels preserve Groq model, profile and language across reload",{profiles:[groqProfile]},async p=>{
    await p.getByRole('button',{name:'Phụ đề',exact:true}).click();
    await p.getByLabel('Ngôn ngữ gốc').selectOption('en');await p.getByLabel('Cấu hình AI phụ đề').selectOption('prof-groq');
    await p.getByLabel('Model phụ đề').selectOption('whisper-large-v3-turbo');
    for(const label of ['Nhanh','Cân bằng','Chính xác']) {
      await p.getByRole('radio',{name:'Phụ đề · '+label,exact:true}).check();
      assert.equal(await p.getByLabel('Model phụ đề').inputValue(),'whisper-large-v3-turbo');assert.equal(await p.getByLabel('Cấu hình AI phụ đề').inputValue(),'prof-groq');
      assert.equal(await p.getByLabel('Ngôn ngữ gốc').inputValue(),'en');
    }
    await p.reload();await p.getByRole('button',{name:'Phụ đề',exact:true}).click();
    assert(await p.getByRole('radio',{name:'Phụ đề · Chính xác',exact:true}).isChecked());
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
    await p.getByRole('button',{name:'Phụ đề',exact:true}).click();
    await p.getByLabel('Ngôn ngữ gốc').selectOption('en');await p.getByLabel('Cấu hình AI phụ đề').selectOption('prof-groq');
    assert(await p.getByRole('radio',{name:'Phụ đề · Cân bằng',exact:true}).isChecked());
    await p.getByRole('radio',{name:'Phụ đề · Chính xác',exact:true}).check();
    await p.getByRole('button',{name:'Cuộc họp mới',exact:true}).click();
    assert.equal(await p.getByLabel('Model chép lời local',{exact:true}).inputValue(),'nemotron-3.5-q8');
    assert(await p.getByRole('radio',{name:'Cuộc họp · Nhanh',exact:true}).isChecked());
    await p.getByRole('button',{name:'Bắt đầu ghi',exact:true}).click();await p.waitForFunction(()=>window.qa.calls.some(c=>c.cmd==='start'));
    const options=await p.evaluate(()=>window.qa.calls.find(c=>c.cmd==='start').args.options);assert.equal(options.model,'nemotron-3.5-q8');assert.equal(options.speechQuality,'fast');
  });
  await check("Processing levels do not pretend fixed APIs expose accuracy controls",{profiles:mockProfiles},async p=>{
    await p.getByRole('button',{name:'Phụ đề',exact:true}).click();await p.getByLabel('Dịch sang',{exact:true}).check();
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
    await p.getByRole('button',{name:'Cài đặt',exact:true}).click();await p.getByRole('tab',{name:'Chỉ phụ đề',exact:true}).click();await p.getByLabel('Dịch sang cho phụ đề dùng riêng').uncheck();
    await p.locator('label',{hasText:'Phụ đề trực tiếp'}).locator('select').first().selectOption('prof-groq');
    const model=p.getByLabel('Model phụ đề',{exact:true});assert((await model.locator('option').allTextContents()).includes('Groq Free/whisper-large-v3-turbo'));
    const fallback=p.getByLabel('Dự phòng · Phụ đề trực tiếp',{exact:true});const choice={profileId:'prof-groq2',model:'whisper-large-v3'};await fallback.selectOption(JSON.stringify(choice));assert((await fallback.locator('option:checked').innerText()).includes('groq2/whisper-large-v3'));
    await p.getByLabel('Dịch sang cho phụ đề dùng riêng').check();assert(!(await fallback.locator('option').allTextContents()).some(s=>s.includes('groq2/whisper')));
    await p.getByLabel('Dịch sang cho phụ đề dùng riêng').uncheck();assert((await fallback.locator('option').allTextContents()).some(s=>s.includes('groq2/whisper')));
    await p.getByRole('tab',{name:'Trong cuộc họp',exact:true}).click();assert.equal(await p.locator('.task-fallbacks select').count(),4);
    await p.getByRole('tab',{name:'Chỉ phụ đề',exact:true}).click();assert.equal(await p.locator('.task-fallbacks select').count(),1);
    await p.locator('.task-fallbacks').scrollIntoViewIfNeeded();await p.screenshot({path:join(output,'profile-alias-fallback.png')});
    await p.reload();await p.getByRole('button',{name:'Phụ đề',exact:true}).click();await p.getByRole('button',{name:'Bắt đầu phụ đề',exact:true}).click();await p.waitForFunction(()=>window.qa.calls.some(c=>c.cmd==='start_caption'));const options=await p.evaluate(()=>window.qa.caption.options);assert.deepEqual(options.fallback,choice);assert.equal(options.model,'whisper-large-v3-turbo');
  });
  await check("Groq Whisper is selectable for original captions without translation and keeps model choice", {profiles:[...mockProfiles,groqProfile]}, async p=>{
    await p.getByRole('button',{name:'Phụ đề',exact:true}).click();await p.getByLabel('Cấu hình AI phụ đề').selectOption('prof-groq');
    const models=await p.getByLabel('Model phụ đề').locator('option').allTextContents();assert(models.includes('Groq Free/whisper-large-v3')&&models.includes('Groq Free/whisper-large-v3-turbo'));assert(!models.includes('openai/gpt-oss-20b'));
    await p.getByLabel('Model phụ đề').selectOption('whisper-large-v3');await p.getByRole('button',{name:'Bắt đầu phụ đề',exact:true}).click();await p.waitForFunction(()=>window.qa.calls.some(c=>c.cmd==='start_caption'));
    const opts=await p.evaluate(()=>window.qa.caption.options);assert.equal(opts.model,'whisper-large-v3');assert.equal(opts.profileId,'prof-groq');assert.equal(opts.translate,false);
    await p.getByRole('button',{name:'Dừng phụ đề',exact:true}).click();await p.getByRole('button',{name:'Bắt đầu phụ đề',exact:true}).waitFor();
    await p.getByLabel('Dịch sang',{exact:true}).check();assert(await p.getByRole('button',{name:'Bắt đầu phụ đề',exact:true}).isDisabled());
  });
  await check("Groq original-caption assignment persists through Settings and restart", {profiles:[...mockProfiles,groqProfile]}, async p=>{
    await p.getByRole('button',{name:'Cài đặt',exact:true}).click();await p.getByRole('tab',{name:'Chỉ phụ đề',exact:true}).click();
    await p.getByLabel('Dịch sang cho phụ đề dùng riêng').uncheck();
    const profile=p.locator('label',{hasText:'Phụ đề trực tiếp'}).locator('select').first();await profile.selectOption('prof-groq');await p.getByLabel('Model phụ đề').selectOption('whisper-large-v3');
    await p.reload();await p.getByRole('button',{name:'Phụ đề',exact:true}).click();assert(!await p.getByLabel('Dịch sang',{exact:true}).isChecked());assert.equal(await p.getByLabel('Cấu hình AI phụ đề').inputValue(),'prof-groq');assert.equal(await p.getByLabel('Model phụ đề').inputValue(),'whisper-large-v3');
  });
  await check("Groq text tasks offer GPT OSS and Qwen but never Whisper", {profiles:[...mockProfiles,groqProfile]}, async p=>{
    await p.getByRole('button',{name:'Cài đặt',exact:true}).click();const summary=p.locator('label',{hasText:'Tóm tắt'}).locator('select').first();await summary.selectOption('prof-groq');
    const values=await p.getByLabel('Tóm tắt · Model',{exact:true}).locator('option').evaluateAll(options=>options.map(o=>o.value));for(const m of ['openai/gpt-oss-20b','openai/gpt-oss-120b','qwen/qwen3.8-27b'])assert(values.includes(m));assert(!values.includes('whisper-large-v3'));
  });
  await check("Caption appearance is only in shared Settings and reachable from caption page", {profiles:mockProfiles}, async p=>{
    await p.getByRole('button',{name:'Phụ đề',exact:true}).click();
    await p.getByRole('button',{name:'Cài đặt chữ và hộp phụ đề',exact:true}).click();
    await p.getByText('Tùy chỉnh chữ phụ đề',{exact:true}).click();
    await p.getByLabel('Cỡ chữ phụ đề').fill('34');await p.getByLabel('Cỡ chữ phụ đề').dispatchEvent('input');
    await p.getByLabel('Trong suốt',{exact:true}).check();
    await p.reload();await p.getByRole('button',{name:'Cài đặt',exact:true}).click();await p.getByText('Tùy chỉnh chữ phụ đề',{exact:true}).click();
    assert.equal(await p.getByLabel('Cỡ chữ phụ đề').inputValue(),'34');assert(await p.getByLabel('Trong suốt',{exact:true}).isChecked());
  });
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
      await p.getByRole("button", { name: "Cài đặt", exact: true }).click();
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
      assert(
        await p
          .getByText("Soniox · dịch vụ ngoài", { exact: true })
          .isVisible(),
      );
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
  await check("Caption presentation controls live inside Innote", {profiles:mockProfiles}, async(p)=>{
    await p.getByRole('button',{name:'Cài đặt',exact:true}).click();
    await p.getByText('Tùy chỉnh chữ phụ đề',{exact:true}).click();
    const bilingual=p.getByRole('checkbox',{name:'Song ngữ',exact:true});
    assert.equal(await bilingual.isChecked(),false);
    await bilingual.check();
    await p.getByLabel('Cỡ chữ phụ đề').fill('32');
    await p.getByLabel('Cỡ chữ phụ đề').dispatchEvent('input');
    await p.getByLabel('Căn chữ phụ đề').selectOption('left');
    const style=await p.evaluate(()=>JSON.parse(localStorage.getItem('innote.caption.style')));
    assert.equal(style.showOriginal,true);assert.equal(style.fontSize,32);assert.equal(style.align,'left');
    await p.reload();
    await p.getByRole('button',{name:'Cài đặt',exact:true}).click();
    await p.getByText('Tùy chỉnh chữ phụ đề',{exact:true}).click();
    assert(await p.getByRole('checkbox',{name:'Song ngữ',exact:true}).isChecked());
  });
  await check("Settings offers bundled Moonshine and only downloads optional Nemotron",{missingModels:true},async p=>{
    await p.getByRole('button',{name:'Cài đặt',exact:true}).click();
    assert.equal(await p.getByText(/PhoWhisper/).count(),0);
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
    await p.getByRole('button',{name:'Phụ đề',exact:true}).click();
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
      const caption = p.locator("label", {hasText:"Phụ đề trực tiếp"}).locator("select").first();
      assert.equal(await caption.locator("option").count(),2);
      await p.getByRole("button", {name:"Thêm cấu hình",exact:true}).click();
      const d=p.getByRole("dialog",{name:"Thêm cấu hình"});
      await d.getByLabel("Tên cấu hình").fill("Soniox added during this session");
      await d.getByLabel("Nhà cung cấp",{exact:true}).selectOption("soniox");
      await d.getByLabel("API key").fill("fake-soniox-key");
      await d.getByRole("button",{name:"Kiểm tra kết nối",exact:true}).click();
      const save=d.getByRole("button",{name:"Thêm cấu hình",exact:true});
      await p.waitForFunction(()=>!document.querySelector(".profile-fields")?.disabled);
      await save.click();
      await caption.locator('option[value="prof-new"]').waitFor({state:"attached"});
      await caption.selectOption("prof-new");
      assert.equal(await p.getByLabel("Model phụ đề").inputValue(),"stt-rt-v5");
      const transcription=p.locator("label",{hasText:"Chép lời (transcription)"}).locator("select").first();
      assert.equal(await transcription.locator('option[value="prof-new"]').count(),1);
      const card=p.locator(".ai-profile-card").filter({hasText:"Soniox added during this session"});
      // Select by the visible profile row, independently of row CSS naming.
      const row=p.getByText("Soniox added during this session",{exact:true}).locator("..").locator("..");
      await row.getByRole("button",{name:"Xóa",exact:true}).click();
      await p.waitForFunction(()=>!document.querySelector('.task-assign-grid select option[value="prof-new"]'));
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
      await p.getByRole("button", { name: "Cài đặt", exact: true }).click();
      await p.getByText("Gán cấu hình cho tác vụ", { exact: true }).waitFor();
      const labels = ["Chép lời (transcription)", "Phụ đề trực tiếp"];
      const selects = labels.map((label) =>
        p.locator("label", { hasText: label }).locator("select").first(),
      );
      await selects[0].selectOption("__local__");
      await selects[1].selectOption("prof-soniox");
      const stored = await p.evaluate(() =>
        JSON.parse(localStorage.getItem("may.task_assign.meeting") ?? "{}"),
      );
      assert.equal(stored.transcription.profileId, "__local__");
      assert.equal(stored.caption.profileId, "prof-soniox");
      await p.getByRole("tab", { name: "Chỉ phụ đề" }).click();
      await p.getByRole("tab", { name: "Trong cuộc họp" }).click();
      await p.screenshot({ path: join(output, "task-assign-1180x800.png") });
    },
  );

  await check(
    "Caption-only page starts and stops a standalone caption session",
    { viewport: { width: 1180, height: 800 }, profiles: mockProfiles },
    async (p) => {
      await p.getByRole("button", { name: "Phụ đề", exact: true }).click();
      await p.getByLabel("Cấu hình AI phụ đề").selectOption("prof-soniox");
      await p.getByLabel("Model phụ đề").selectOption("stt-rt-v5");
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
    await p.getByRole("button", { name:"Phụ đề", exact:true }).click();
    await p.getByLabel("Dịch sang",{exact:true}).check();
    for (const [id, model] of [["prof-soniox","stt-rt-v5"],["prof-openai","gpt-realtime-translate"],["prof-gemini","gemini-3.5-live-translate-preview"]]) {
      await p.getByLabel("Cấu hình AI phụ đề").selectOption(id);
      assert.equal(await p.getByLabel("Model phụ đề").inputValue(),model);
      await p.getByRole("button",{name:"Kiểm tra model",exact:true}).click();
      await p.waitForFunction(m=>window.qa.calls.some(c=>c.cmd==="test_ai_connection"&&c.args.input.model===m),model);
    }
    await p.getByLabel("Ngôn ngữ dịch đích").selectOption("en");
    await p.reload();
    await p.getByRole("button", { name:"Phụ đề", exact:true }).click();
    assert.equal(await p.getByLabel("Cấu hình AI phụ đề").inputValue(),"prof-gemini");
    assert.equal(await p.getByLabel("Model phụ đề").inputValue(),"gemini-3.5-live-translate-preview");
    assert.equal(await p.getByLabel("Ngôn ngữ dịch đích").inputValue(),"en");
  });

  await check("Caption visibility starts off, previews without upload, auto-shows and hides independently", {profiles:mockProfiles}, async(p)=>{
    await p.getByRole('button',{name:'Phụ đề',exact:true}).click();
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
      await p.getByRole("button", { name: "Phụ đề", exact: true }).click();
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
    await p.getByRole('button',{name:'Phụ đề',exact:true}).click();
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
      taskAssignMeeting: { caption: { profileId: "prof-soniox", model: "stt-rt-v5", targetLang: "vi" } },
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
