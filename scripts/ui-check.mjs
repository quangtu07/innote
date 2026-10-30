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
            id: "large-v3-turbo-q5_0",
            name: "Ưu tiên độ chính xác",
            bytes: 574041195,
            installed: !options.missingModels,
          },
        ],
        active_id: null,
        data_dir: "C:\\QA\\Innote",
      },
      exports: [],
      delayAudio: !!options.delayAudio,
    };
    window.qa.snapshot.models.push({id:"nemotron-3.5-q8",name:"Đa ngôn ngữ · Việt/Anh/Hàn/Trung",bytes:741548352,installed:!options.missingModels&&!options.missingMultilingual});
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
    if (options.preferences)
      localStorage.setItem(
        "may.preferences",
        JSON.stringify(options.preferences),
      );
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
        if (cmd === "refine_transcript") {
          const s = qa.snapshot.sessions.find((s) => s.id === args.id);
          s.segments = [
            { id: "refined-1", start: 0, end: 5, text: "Bản chép đã hiệu chỉnh bằng PhoWhisper.", edited: false }
          ];
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
          await new Promise((r) => setTimeout(r, options.slowTest ? 3000 : 15));
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
              qa.snapshot.soniox_configured
                ? "soniox"
                : "local",
            warnings:
              args.options.provider === "soniox" &&
              !qa.snapshot.soniox_configured
                ? [
                    "Chưa có key Soniox. Phiên này dùng model local miễn phí; không gửi audio lên Soniox.",
                  ]
                : [],
          };
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
    content: `${mocks}\n(${init.toString()})(${JSON.stringify({ ids, options })});`,
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
    await p.screenshot({ path: join(output, "author-900x640.png") });
  });
  await check("Starting capture locks language provider and transcription choices", {slowStart:true}, async p=>{
    await p.getByRole('button',{name:'Bắt đầu ghi',exact:true}).click();
    for (const label of ['Ngôn ngữ','Cách chép lời'])
      assert(await p.getByLabel(label,{exact:true}).isDisabled(),label);
    assert.equal(await p.getByLabel('Chất lượng',{exact:true}).count(),0);
    assert.equal(await p.getByLabel('Model',{exact:true}).count(),0);
    const options=await p.evaluate(()=>window.qa.calls.find(c=>c.cmd==='start').args.options);
    assert.equal(options.model,'large-v3-turbo-q5_0');
    assert(await p.getByRole('checkbox',{name:/Chép lời khi/}).isDisabled());
    await p.getByRole('button',{name:'Tạm dừng',exact:true}).waitFor();
  });
  await check("Mixed meeting keeps VI EN KO ZH text and commits editable turns without translation", {}, async p=>{
    await p.getByLabel('Ngôn ngữ',{exact:true}).selectOption('multi');
    assert(await p.getByLabel('Cách chép lời',{exact:true}).isDisabled());
    await p.getByRole('button',{name:'Bắt đầu ghi',exact:true}).click();
    const options=await p.evaluate(()=>window.qa.calls.find(c=>c.cmd==='start').args.options);
    assert.equal(options.language,'multi');assert.equal(options.model,'nemotron-3.5-q8');assert.equal(options.provider,'local');
    await p.getByRole('button',{name:'Kết thúc',exact:true}).waitFor();
    const text='Xin chào. Hello team. 안녕하세요. 大家好。';
    await p.evaluate(async ({id,text})=>{const {emit}=await import('/node_modules/@tauri-apps/api/event.js');await emit('live-transcript',{id,update:{text,start:0,end:5,final_text:false,words:[]}});},{id:ids[2],text});
    await p.locator('[data-live-caption]').getByText(text,{exact:true}).waitFor();
    await p.evaluate(({id,text})=>{const s=window.qa.snapshot.sessions.find(s=>s.id===id);s.duration=5;s.processed=5;s.segments=[{id:'multi-final',start:0,end:5,text,edited:false}];},{id:ids[2],text});
    await p.waitForTimeout(1500);
    await p.getByRole('button',{name:'Sửa đoạn 00:00',exact:true}).waitFor();
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
    assert.equal(options.model,'large-v3-turbo-q5_0');
  });
  await check("Korean Chinese and saved mixed preferences select the matching local model", {preferences:{system:'speaker',microphone:'mic',language:'multi',model:'large-v3-turbo-q5_0',transcribe:true}}, async p=>{
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
    await p.getByText('Đang hiệu chỉnh… Có thể tải bản chép khi xong.',{exact:true}).waitFor();
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
    {},
    async (p) => {
      await openMeeting(p, "Cuộc họp Alpha");
      await p.getByRole("button", { name: "Cài đặt", exact: true }).click();
      await p.getByLabel(/API key cho AI/).fill("sk-test");
      await p.getByRole("button", { name: "Kiểm tra API", exact: true }).click();
      await p.getByRole("button", { name: "Lưu API key", exact: true }).click();
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
      await p
        .getByRole("region", { name: "Bản làm mịn bằng AI" })
        .getByText("Nội dung thử tiếng Việt. (đã làm mịn)", { exact: true })
        .waitFor();
      await p.screenshot({ path: join(output, "ai-smoothing.png") });
      assert(
        await p.getByText("Nội dung thử tiếng Việt.", { exact: true }).isVisible(),
      );
      await p
        .getByRole("group", { name: "Chọn bản để tải" })
        .getByRole("button", { name: "Bản làm mịn", exact: true })
        .click();
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
        .getByRole("region", { name: "Bản làm mịn bằng AI" })
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
    {},
    async (p) => {
      await openMeeting(p, "Cuộc họp Alpha");
      await p.getByRole("button", { name: "Cài đặt", exact: true }).click();
      await p.getByLabel(/API key cho AI/).fill("sk-test");
      await p.getByRole("button", { name: "Kiểm tra API", exact: true }).click();
      await p.getByRole("button", { name: "Lưu API key", exact: true }).click();
      await openMeeting(p, "Cuộc họp Alpha");

      await p.getByRole("button", { name: "Tóm tắt", exact: true }).click();
      const dialog = p.locator("dialog.summary-dialog");
      await dialog.waitFor();
      assert(await dialog.isVisible());

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
  await check(
    "AI smoothing key is kept in memory from Settings",
    {},
    async (p) => {
      await p.getByRole("button", { name: "Cài đặt", exact: true }).click();
      assert.equal(
        await p.getByLabel("Nhà cung cấp", { exact: true }).inputValue(),
        "openai",
      );
      await p.getByLabel(/API key cho AI/).fill("sk-test");
      await p.getByRole("button", { name: "Kiểm tra API", exact: true }).click();
      await p.getByRole("button", { name: "Lấy model", exact: true }).click();
      await p.waitForFunction(() =>
        window.qa.calls.some((c) => c.cmd === "fetch_llm_models"),
      );
      await p.getByRole("button", { name: "Lưu API key", exact: true }).click();
      await p.waitForFunction(() => window.qa.snapshot.llm_configured === true);
      await p.screenshot({ path: join(output, "ai-settings.png") });
      assert(
        await p.getByText("Đã có API key trong bộ nhớ.", { exact: false }).isVisible(),
      );
    },
  );
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
        .nth(1)
        .click();
      await p
        .getByRole("alert")
        .filter({ hasText: "Không tải được model" })
        .waitFor();
      assert(
        await p
          .getByRole("button", { name: "Tải model", exact: true })
          .nth(1)
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
  await check(
    "Local is the default; missing Soniox key falls back without blocking start",
    {},
    async (p) => {
      assert.equal(await p.getByLabel("Cách chép lời").inputValue(), "local");
      await p.getByLabel("Cách chép lời").selectOption("soniox");
      assert(
        await p.getByText(/Chưa có key: phiên này sẽ dùng local/).isVisible(),
      );
      await p.getByRole("button", { name: "Bắt đầu ghi", exact: true }).click();
      await p
        .getByText(/Chưa có key Soniox. Phiên này dùng model local/)
        .first()
        .waitFor();
      assert.equal(
        await p.evaluate(() => window.qa.snapshot.sessions.at(-1).provider),
        "local",
      );
    },
  );
  await check(
    "Soniox key stays out of preferences and start payload; each new meeting defaults local",
    {},
    async (p) => {
      await p.getByRole("button", { name: "Cài đặt", exact: true }).click();
      await p.getByLabel("API key Soniox").fill("fake-ui-secret");
      await p
        .getByRole("button", { name: "Giữ key trong bộ nhớ", exact: true })
        .click();
      await p.getByText(/Đã có key trong bộ nhớ/).waitFor();
      assert.equal(await p.getByLabel("API key Soniox").inputValue(), "");
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
    "Selecting Soniox cannot bypass the required downloaded fallback model",
    { missingModels: true },
    async (p) => {
      await p.getByLabel("Cách chép lời").selectOption("soniox");
      assert(
        await p
          .getByRole("button", { name: "Bắt đầu ghi", exact: true })
          .isDisabled(),
      );
    },
  );
  await check(
    "Clearing a configured Soniox key keeps local available",
    {},
    async (p) => {
      await p.getByRole("button", { name: "Cài đặt", exact: true }).click();
      await p.getByLabel("API key Soniox").fill("fake-ui-secret");
      await p
        .getByRole("button", { name: "Giữ key trong bộ nhớ", exact: true })
        .click();
      await p.getByText(/Đã có key trong bộ nhớ/).waitFor();
      await p.getByRole("button", { name: "Xóa key", exact: true }).click();
      await p
        .getByText(/Chưa có key. Innote vẫn dùng local bình thường/)
        .waitFor();
    },
  );
  await check(
    "Settings downloads the fixed high-accuracy model without a quality picker",
    { missingModels: true },
    async (p) => {
      await p.getByRole("button", { name: "Cài đặt", exact: true }).click();
      assert.equal(await p.getByText("Tiết kiệm tài nguyên", { exact: true }).count(), 0);
      assert.equal(await p.getByText("Cân bằng", { exact: true }).count(), 0);
      await p
        .locator(".model-row")
        .filter({ hasText: "Ưu tiên độ chính xác" })
        .getByRole("button", { name: "Tải model" })
        .click();
      await p.waitForFunction(() =>
        window.qa.calls.some((c) => c.cmd === "download_model"),
      );
      assert.equal(
        await p.evaluate(
          () => window.qa.calls.find((c) => c.cmd === "download_model").args.id,
        ),
        "large-v3-turbo-q5_0",
      );
    },
  );
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
      assert(await p.getByRole("button", { name: "Bắt đầu ghi" }).isDisabled());
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
