<script lang="ts">
  import { onMount, onDestroy, tick } from "svelte";
  import { version as appVersion } from "../package.json";
  import { invoke, convertFileSrc } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { check, type Update } from "@tauri-apps/plugin-updater";
  import type { Segment } from "./types";
  import Logo from "./Logo.svelte";
  import SpeechQualityControl from "./SpeechQualityControl.svelte";
  import WhatsNewDialog from "./WhatsNewDialog.svelte";
  import {SPEECH_LEVELS_KEY,parseProcessingLevels,qualityKey,qualitySupported,type SpeechQuality} from "./lib/speech-quality";
  import CaptionControls from "./CaptionControls.svelte";
  import CaptionWindowToggle from "./CaptionWindowToggle.svelte";
  import { CaptionPresentation, type CaptionCue } from "./lib/caption-presentation";
  import { readCaptionStyle, CAPTION_STYLE_KEY } from "./lib/caption-style";
  import { showCaptionWindow, closeCaptionWindow } from "./lib/caption-window";
  import {
    Plus,
    Search,
    Settings,
    Mic,
    Headphones,
    Circle,
    Play,
    Pause,
    Square,
    Download,
    FolderOpen,
    FileText,
    Check,
    X,
    AlertCircle,
    RefreshCw,
    ArrowDown,
    Trash2,
    Pencil,
    NotebookPen,
    CheckCheck,
    Globe,
    Sparkles,
    List,
    ClipboardList,
    Copy,
    Captions,
  } from "@lucide/svelte";
  import type { Snapshot, Meeting } from "./types";
  import {
    locales,
    localeNames,
    localeEnglishNames,
    localeTags,
    isUiLocale,
    fill,
    stringsFor,
    type UiLocale,
  } from "./i18n";
  function storedUiLocale(): UiLocale | null {
    try {
      const value = localStorage.getItem("may.ui_locale");
      return isUiLocale(value) ? value : null;
    } catch {
      return null;
    }
  }
  function rememberUiLocale(value: UiLocale) {
    try {
      localStorage.setItem("may.ui_locale", value);
    } catch {
      /* The interface still switches for this run if storage is unavailable. */
    }
  }
  let uiLocale = storedUiLocale();
  let tr = stringsFor(uiLocale ?? "vi");
  $: tr = stringsFor(uiLocale ?? "vi");
  function setUiLocale(value: UiLocale) {
    uiLocale = value;
    rememberUiLocale(value);
  }
  let data: Snapshot = {
    sessions: [],
    devices: [],
    models: [],
    data_dir: "",
    active_id: null,
  };
  let selected: string | null = null;
  let page = "meeting";
  let pendingActions = 0;
  $: busy = pendingActions > 0;
  let pendingEdits: Promise<unknown> = Promise.resolve();
  let audioRequest = 0;
  let audioLoadingFor = "";
  let snapshotRequest = 0;
  let snapshotApplied = 0;
  let error = "";
  let toast = "";
  let title = "";
  let query = "";
  let searchText = "";
  let system = "";
  let microphone = "";
  let language = "vi";
  const model = "moonshine-vi";
  let localMeetingModels: Record<string,string> = {};
  let processingLevels: Record<string,SpeechQuality> = {};
  function compatibleLocalModels(source:string) { return source==="vi"?["moonshine-vi","nemotron-3.5-q8"]:source==="en"?["moonshine-en","nemotron-3.5-q8"]:["multi","ko","zh"].includes(source)?["nemotron-3.5-q8"]:[]; }
  function chooseLocalMeetingModel(value:string) {
    if(!compatibleLocalModels(language).includes(value))return;
    localMeetingModels={...localMeetingModels,[language]:value};
    localStorage.setItem("may.meeting.local-models",JSON.stringify(localMeetingModels));
  }
  function chooseProcessingLevel(key:string,quality:SpeechQuality) {
    processingLevels={...processingLevels,[key]:quality};
    localStorage.setItem(SPEECH_LEVELS_KEY,JSON.stringify(processingLevels));
  }
  let sourcesInitialized = false;
  let transcribe = true;
  let meetingCaptions = false;
  let meetingTranslate = false;
  let meetingTargetLang = "vi";
  let localFallback = false;
  let translationView: "original" | "translation" | "bilingual" = "original";
  let translationLanguage = "";
  let translationMeeting = "";
  $: if ((selected ?? "") !== translationMeeting) { translationMeeting=selected ?? ""; translationView="original"; translationLanguage=""; }
  $: translationLanguages = [...new Set((meeting?.translations ?? []).map(t=>t.language))];
  $: visibleTranslationLanguage = translationLanguage || translationLanguages[0] || "vi";
  $: savedTranslations = (meeting?.translations ?? []).filter(t=>t.language===visibleTranslationLanguage);
  $: if(active?.speech?.translation_language) meetingTargetLang=active.speech.translation_language;

  let provider = "local";
  let sonioxKey = "";
  let sonioxConnected = false;
  let aiProfiles: AiProfileSummary[] = [];
  interface AiProfileSummary {
    id: string;
    name: string;
    provider: string;
    base_url: string;
    has_key: boolean;
    key_tail: string;
    created: string;
  }
  interface ModelChoice { profileId: string; model: string; }
  function modelLabel(id:string|undefined,model:string){return id === "__local__" ? `Local/${model}${model === "moonshine-vi" ? " · Tiếng Việt" : model === "moonshine-en" ? " · English" : " · Việt, Anh, Hàn, Trung"}` : `${profileOf(id,aiProfiles)?.name ?? "Cấu hình"}/${model}`;}
  interface TaskAssign {
    transcription?: { profileId: string; model: string; fallback?:ModelChoice };
    caption?: { profileId: string; model: string; targetLang: string; translate?: boolean; fallback?:ModelChoice };
    smooth?: { profileId: string; model: string; fallback?:ModelChoice };
    summary?: { profileId: string; model: string; fallback?:ModelChoice };
  }
  const chatProviders = [
    "groq",
    "openai",
    "deepseek",
    "gemini",
    "openai-compatible",
  ] as const;
  const captionProviders = ["soniox", "openai", "gemini", "groq"] as const;
  let confOpen = false;
  let profileDialog: HTMLDialogElement;
  $: if (confOpen) tick().then(() => {
    if (profileDialog && !profileDialog.open) {
      profileDialog.showModal();
      profileDialog.querySelector<HTMLInputElement>('input')?.focus();
    }
  });
  function closeProfileDialog() {
    profileDialog?.close();
    confOpen = false;
  }
  function profileDialogKeydown(event: KeyboardEvent) {
    if(event.key !== 'Tab')return;
    const controls=Array.from(profileDialog.querySelectorAll<HTMLElement>('button:not(:disabled), input:not(:disabled), select:not(:disabled), [tabindex="0"]')).filter(el=>el.getClientRects().length>0);
    const first=controls[0],last=controls.at(-1);
    if(event.shiftKey && document.activeElement===first){event.preventDefault();last?.focus();}
    else if(!event.shiftKey && document.activeElement===last){event.preventDefault();first?.focus();}
  }
  let confEditingId: string | null = null;
  let confName = "";
  let confProvider = "openai";
  let confBaseUrl = "";
  let confKey = "";
  let confModel = "";
  let confModels: string[] = [];
  let confTesting = false;
  let confTested = false;
  let confTestSignature = "";
  let confStatusText = "";
  let confSaveOk = false;
  let confGeneration = 0;
  let assignmentsReady = false;
  let summaryModel = "gpt-4o-mini";
  const realtimeModels: Record<string, string[]> = {
    soniox: ["stt-rt-v5"],
    openai: ["gpt-realtime-translate"],
    gemini: ["gemini-3.5-live-translate-preview"],
  };
  function captionModels(id: string, profiles: AiProfileSummary[], translate = true, source = "auto") {
    if(id === "__local__")return translate ? [] : source === "vi" ? ["moonshine-vi",multilingualModel] : source === "en" ? ["moonshine-en",multilingualModel] : ["multi","ko","zh"].includes(source) ? [multilingualModel] : ["","auto"].includes(source) ? ["moonshine-vi","moonshine-en",multilingualModel] : [];
    const provider=profileOf(id,profiles)?.provider??"";
    if(!translate)return provider === "groq" ? ["whisper-large-v3-turbo","whisper-large-v3"] : provider === "soniox" ? ["stt-rt-v5"] : [];
    return realtimeModels[provider] ?? [];
  }
  function fallbackChoices(task:keyof TaskAssign, profiles:AiProfileSummary[], assignment:TaskAssign|undefined, mode:string, translate:boolean) {
    return profiles.flatMap(p=>{
      const models=task === "transcription" ? (p.provider === "soniox" ? ["stt-rt-v5"] : []) : task === "caption" ? (mode === "meeting" && p.provider === "groq" ? [] : captionModels(p.id,profiles,translate)) : chatProviders.includes(p.provider as any) ? textModels(p.id,profiles) : [];
      return models.map(model=>({profileId:p.id,model}));
    }).filter(choice=>choice.profileId!==assignment?.[task]?.profileId || choice.model!==assignment?.[task]?.model);
  }
  function taskLabel(task:keyof TaskAssign){return task === "transcription" ? tr.taskTranscription : task === "caption" ? tr.taskCaption : task === "smooth" ? tr.taskSmooth : tr.taskSummary;}
  function chooseFallback(task:keyof TaskAssign,value:string){
    const primary=taskAssigns[taskMode]?.[task];if(!primary)return;
    setAssignment(task,{...primary,fallback:value ? JSON.parse(value) : undefined});
  }
  function textModels(id: string, profiles: AiProfileSummary[]) {
    const provider = profileOf(id, profiles)?.provider ?? smoothProvider;
    return provider === "groq" ? ["openai/gpt-oss-20b","openai/gpt-oss-120b","qwen/qwen3.8-27b"] : provider === "gemini"
      ? ["gemini-2.5-flash", "gemini-2.5-pro"]
      : provider === "deepseek"
        ? ["deepseek-chat", "deepseek-reasoner"]
        : provider === "openai"
          ? ["gpt-4o-mini", "gpt-4o"]
          : [];
  }
  $: if (assignmentsReady) {
    localStorage.setItem(
      "may.caption.standalone",
      JSON.stringify({
        profileId: captionProfileId,
        model: captionModel,
        targetLang: captionTargetLang,
        translate: captionTranslate,
        sourceLang: captionSourceLang,
        device: captionDevice,
      }),
    );
  }
  let taskMode: "meeting" | "captionsonly" = "meeting";
  let taskAssigns: Record<string, TaskAssign> = {};
  $: taskCaptionTranslate = taskMode === "meeting" || taskAssigns[taskMode]?.caption?.translate !== false;
  let captionDevice = "";
  let captionTargetLang = "vi";
  let captionTranslate = false;
  let captionSourceLang = "auto";
  $: captionLocalReady = captionProfileId !== "__local__" || data.models.some(m=>m.id===captionModel&&m.installed);
  let captionProfileId = "";
  let captionModel = "";
  let captionRunning = false;
  const hadPreviousInstallation=!!(localStorage.getItem("may.ui_locale") || localStorage.getItem("may.preferences"));
  let captionStateReady=false;
  let captionTesting = false;
  let captionSending = false;
  let captionProvider = "";
  let captionStatusRevision = 0;
  let captionTextRevision = 0;
  let captionStatusMessage = "";
  let captionError = "";
  let captionText = "";
  let captionOriginal = "";
  const captionPresentation=new CaptionPresentation();
  let captionBilingual=readCaptionStyle().showOriginal;
  function applyCaptionPreview(){const cue=captionPresentation.view(captionBilingual);captionText=cue.text;captionOriginal=captionBilingual?cue.original:"";}
  let captionFallbackMessage = "";
  let transcriptionProfileId = "";
  $: meetingSharedChoice = provider === "soniox" && transcriptionProfileId
    ? JSON.stringify({profileId:transcriptionProfileId,model:"stt-rt-v5"})
    : provider === "local" && !meetingTranslate ? JSON.stringify({profileId:"__local__",model:effectiveModel}) : "";
  function chooseMeetingSharedModel(value:string) {
    if(!value)return;
    const choice=JSON.parse(value) as ModelChoice;
    if(choice.profileId === "__local__" && !meetingTranslate){chooseLocalMeetingModel(choice.model);provider="local";transcriptionProfileId="";return;}
    const profile=profileOf(choice.profileId,aiProfiles);
    if(profile?.provider !== "soniox" || choice.model !== "stt-rt-v5") {error="Model này chưa hỗ trợ chép lời và dịch phụ đề dùng chung trong cuộc họp.";return;}
    provider="soniox";transcriptionProfileId=profile.id;error="";
  }
  let smoothProfileId = "";
  let summaryProfileId = "";
  type SmoothProvider = "openai" | "deepseek" | "gemini" | "openai-compatible" | "groq";
  const smoothDefaults: Record<SmoothProvider, string> = {
    openai: "gpt-4o-mini",
    deepseek: "deepseek-chat",
    gemini: "gemini-2.5-flash",
    "openai-compatible": "",
    groq: "openai/gpt-oss-20b",
  };
  const isSmoothProvider = (value: unknown): value is SmoothProvider =>
    ["openai", "deepseek", "gemini", "openai-compatible", "groq"].includes(
      String(value),
    );
  let smoothProvider: SmoothProvider = "openai";
  let smoothModel = smoothDefaults.openai;
  let smoothBaseUrl = "";
  let smoothKey = "";
  let smoothPrompt = "";
  let smoothOpen = false;
  let smoothDialog: HTMLDialogElement;
  let transcriptView: "original" | "smoothed" = "original";
  let smoothModels: string[] = [];
  let llmConnectionSignature = "";
  type SummaryStyle = "overview" | "decisions" | "full";
  let summaryStyle: SummaryStyle = "overview";
  let summarySource: "original" | "smoothed" = "original";
  let summaryPrompt = "";
  let summaryOpen = false;
  let summaryDialog: HTMLDialogElement;
  let transcriptTab: "transcript" | "summary" = "transcript";
  let smoothPrefsInitialized = false;
  let modelPickerOpen = false;
  let modelPickerDialog: HTMLDialogElement;
  let modelPickerSearch = "";
  let systemLevel = 0;
  let micLevel = 0;
  let progress: { id: string; bytes: number; total: number } | null = null;
  let audioUrl = "";
  let audio: HTMLAudioElement;
  let transcript: HTMLDivElement;
  let autoScroll = true;
  let editing = "";
  let draft = "";
  let showNotes = false;
  let notesDraft = "";
  let deleteOpen = false;
  let deleteDialog: HTMLDialogElement;
  let deleteTarget: Meeting | null = null;
  let meetingsOpen = false;
  let meetingsDialog: HTMLDialogElement;
  let renamingId = "";
  let renameDraft = "";
  let stopOpen = false;
  let stopDialog: HTMLDialogElement;
  $: if (deleteOpen)
    tick().then(() => {
      if (deleteDialog && !deleteDialog.open) deleteDialog.showModal();
    });
  $: if (stopOpen)
    tick().then(() => {
      if (stopDialog && !stopDialog.open) stopDialog.showModal();
    });
  $: if (meetingsOpen)
    tick().then(() => {
      if (meetingsDialog && !meetingsDialog.open) meetingsDialog.showModal();
    });
  $: if (summaryOpen)
    tick().then(() => {
      if (summaryDialog && !summaryDialog.open) summaryDialog.showModal();
    });
  $: if (smoothOpen)
    tick().then(() => {
      if (smoothDialog && !smoothDialog.open) smoothDialog.showModal();
    });
  $: if (modelPickerOpen)
    tick().then(() => {
      if (modelPickerDialog && !modelPickerDialog.open)
        modelPickerDialog.showModal();
    });

  let lastExport = "";
  let testUrl = "";
  let testing = false;
  let updateInfo: Update | null = null;
  let updatePhase: "available" | "downloading" | "installing" | null = null;
  let updateChecking = false;
  let updatePercent = 0;
  let updateDownloaded = 0;
  let updateTotalBytes = 0;
  let testFeedback = "";
  let testKind: "system" | "microphone" | "both" = "both";
  let testSeconds = 5;
  let testLabel = "";
  let testWarnings: string[] = [];
  let testQuiet = false;
  $: groupedTestWarnings = Array.from(
    testWarnings.reduce(
      (counts, message) => counts.set(message, (counts.get(message) ?? 0) + 1),
      new Map<string, number>(),
    ),
    ([message, count]) => ({ message, count }),
  );
  let testAudio: HTMLAudioElement;
  let lastCount = 0;
  const desktop = "__TAURI_INTERNALS__" in window;
  $: meeting = data.sessions.find((s) => s.id === selected);
  $: canSmooth =
    !!meeting &&
    meeting.segments.length > 0 &&
    !["recording", "paused", "processing"].includes(meeting.status);
  $: llmConfigSignature = JSON.stringify([
    summaryOpen ? summaryProfileId : smoothProfileId,
    summaryOpen ? summaryModel : smoothModel,
    smoothProvider,
    smoothBaseUrl,
    aiProfiles.map((p) => [p.id, p.provider, p.base_url, p.key_tail]),
  ]);
  $: llmConnected = llmConfigSignature === llmConnectionSignature || (!!profileOf(summaryOpen ? summaryProfileId : smoothProfileId,aiProfiles) && !!(summaryOpen ? taskAssigns.meeting?.summary?.fallback : taskAssigns.meeting?.smooth?.fallback));
  $: filteredModels = smoothModels.filter((m) =>
    m.toLowerCase().includes(modelPickerSearch.trim().toLowerCase()),
  );
  function summaryHash(text: string) {
    let hash = 0;
    for (let index = 0; index < text.length; index += 1) {
      hash = (hash * 31 + text.charCodeAt(index)) | 0;
    }
    return String(hash >>> 0);
  }
  $: summaryCurrentHash = (() => {
    if (!meeting || !meeting.summary) return "";
    const source =
      meeting.summary.source === "smoothed"
        ? (meeting.smoothed?.text ?? "")
        : meeting.segments.map((s) => s.text).join("\n");
    return summaryHash(source);
  })();
  $: summaryStale =
    !!meeting?.summary?.source_hash &&
    summaryCurrentHash !== meeting.summary.source_hash;
  $: smoothSegments = (() => {
    if (!meeting?.smoothed?.text || meeting.segments.length === 0) return [];
    const source = meeting.segments;
    const smoothed = meeting.smoothed.text.trim();
    if (!smoothed) return [];
    const totalWords = Math.max(
      1,
      source.reduce(
        (sum, s) =>
          sum + (s.text.trim().split(/\s+/).filter(Boolean).length || 1),
        0,
      ),
    );
    const smoothWords = smoothed.split(/\s+/).filter(Boolean);
    const totalSmoothWords = Math.max(1, smoothWords.length);
    const result: { id: string; start: number; end: number; text: string }[] =
      [];
    let cursor = 0;
    let smoothStart = 0;
    source.forEach((s, index) => {
      const words = Math.max(
        1,
        Math.round(
          ((s.text.trim().split(/\s+/).filter(Boolean).length || 1) /
            totalWords) *
            totalSmoothWords,
        ),
      );
      const part = smoothWords
        .slice(smoothStart, smoothStart + words)
        .join(" ");
      smoothStart += words;
      const nextStart = cursor + Math.max(0, s.end - s.start);
      if (part.trim())
        result.push({
          id: `smooth-${index}`,
          start: s.start,
          end: Math.max(s.start, nextStart),
          text: part.trim(),
        });
      cursor = nextStart;
    });
    const remaining = smoothWords.slice(smoothStart).join(" ");
    if (remaining.trim() && result.length && result[result.length - 1]) {
      result[result.length - 1].text =
        `${result[result.length - 1].text} ${remaining.trim()}`.trim();
    }
    return result;
  })();
  $: exportingVersion =
    transcriptView === "smoothed" && meeting?.smoothed
      ? "smoothed"
      : translationView === "original" ? "original" : `${translationView}:${visibleTranslationLanguage}`;
  function parseSummarySections(text: string) {
    const lines = text.split("\n");
    const sections: { title?: string; items: string[]; text?: string }[] = [];
    let currentTitle: string | undefined = undefined;
    let currentItems: string[] = [];
    let currentParagraphs: string[] = [];

    function flush() {
      if (
        currentTitle !== undefined ||
        currentItems.length > 0 ||
        currentParagraphs.length > 0
      ) {
        sections.push({
          title: currentTitle,
          items: [...currentItems],
          text: currentParagraphs.join("\n").trim() || undefined,
        });
        currentItems = [];
        currentParagraphs = [];
      }
    }

    for (const rawLine of lines) {
      const line = rawLine.trim();
      if (line.startsWith("#")) {
        flush();
        currentTitle = line.replace(/^#+\s*/, "");
      } else if (line.startsWith("- ") || line.startsWith("* ")) {
        currentItems.push(line.replace(/^[-*]\s*/, ""));
      } else if (line.length > 0) {
        currentParagraphs.push(line);
      }
    }
    flush();
    return sections;
  }
  $: if (smoothPrefsInitialized) {
    try {
      localStorage.setItem(
        "may.llm_preferences",
        JSON.stringify({
          provider: smoothProvider,
          model: smoothModel,
          baseUrl: smoothBaseUrl,
        }),
      );
    } catch {
      /* Keep the current configuration for this run. */
    }
  }
  $: active = data.sessions.find((s) => s.id === data.active_id);
  $: recording = !!active && ["recording", "paused"].includes(active.status);
  const multilingualModel = "nemotron-3.5-q8";
  const authorUrls = {
    zalo: "https://zalo.me/0962239818",
    linkedin: "https://www.linkedin.com/in/alex-linh-790794237/",
    website: "https://alexlinh.io.vn/",
    feedback: "https://forms.gle/hkeSofwG1YNGtwMM9",
  } as const;
  function openAuthor(kind: keyof typeof authorUrls) {
    if (desktop) {
      void invoke("open_author_link", { kind }).catch((e) => {
        error = String(e);
      });
      return;
    }
    window.open(authorUrls[kind], "_blank", "noopener,noreferrer");
  }
  $: multilingual = ["multi", "ko", "zh"].includes(language);
  $: effectiveModel = compatibleLocalModels(language).includes(localMeetingModels[language]) ? localMeetingModels[language] : multilingual ? multilingualModel : language === "en" ? "moonshine-en" : model;
  $: meetingLevelKey=qualityKey("meeting",provider==="local"?"__local__":transcriptionProfileId,provider==="local"?effectiveModel:"stt-rt-v5",language,meetingTranslate);
  $: meetingSpeechQuality=processingLevels[meetingLevelKey]??"balanced";
  $: captionQualityProvider=captionProfileId==="__local__"?"local":profileOf(captionProfileId,aiProfiles)?.provider??"";
  $: captionLevelKey=qualityKey("captionsonly",captionProfileId,captionModel,captionSourceLang,captionTranslate);
  $: captionSpeechQuality=qualitySupported(captionQualityProvider)?processingLevels[captionLevelKey]??"balanced":"balanced";
  $: currentModel = data.models.find((m) => m.id === effectiveModel);

  $: fallbackNotice = meeting?.warnings.find(
    (w) =>
      w.includes("Innote đã chuyển sang local") ||
      w.startsWith("Chưa có key Soniox"),
  );
  $: readyMessage =
    !system && !microphone
      ? tr.readyChoose
      : systemMissing || microphoneMissing
        ? tr.readyMissing
        : transcribe && (provider === "local" || localFallback) && !currentModel?.installed
          ? effectiveModel.startsWith("moonshine-") ? tr.modelMissingBundled : tr.readyDownload
          : tr.ready;
  $: systemMissing =
    !!system &&
    !data.devices.some((d) => d.kind === "system" && d.id === system);
  $: microphoneMissing =
    !!microphone &&
    !data.devices.some((d) => d.kind === "microphone" && d.id === microphone);
  $: history = data.sessions.filter((s) =>
    s.title.toLocaleLowerCase("vi").includes(query.toLocaleLowerCase("vi")),
  );
  let liveUpdates: Record<
    string,
    {
      words: Segment[];
      partial: string;
      partialWords?: Segment[];
      end: number;
      unavailable?: boolean;
    }
  > = {};
  $: currentLive = meeting ? liveUpdates[meeting.id] : undefined;
  $: liveWords = meeting
    ? Array.from(
        new Map(
          [...(meeting.live ?? []), ...(currentLive?.words ?? [])].map((w) => [
            w.id,
            w,
          ]),
        ).values(),
      )
        .filter((w) => w.end > meeting!.processed + 0.00001)
        .sort((a, b) => a.start - b.start)
    : [];
  $: liveText = liveWords.map((w) => w.text).join(" ");
  $: partialText =
    meeting &&
    currentLive &&
    currentLive.end > meeting.processed &&
    ["recording", "paused"].includes(meeting.status)
      ? currentLive.partial
          .split(/\s+/)
          .slice(
            (currentLive.partialWords ?? []).filter(
              (w) => w.end <= meeting!.processed + 0.00001,
            ).length,
          )
          .join(" ")
      : "";
  $: refinementPending =
    !!meeting &&
    (["recording", "paused", "processing"].includes(meeting.status) &&
        (meeting.segments.length > 0 || !!(liveText || partialText)));
  $: showLive =
    !!meeting &&
    ["recording", "paused", "processing", "interrupted"].includes(
      meeting.status,
    ) &&
    !!(liveText || partialText) &&
    (!searchText ||
      `${liveText} ${partialText}`
        .toLocaleLowerCase("vi")
        .includes(searchText.toLocaleLowerCase("vi")));
  $: liveSegment =
    meeting && showLive
      ? {
          id: "live-preview",
          start: liveWords[0]?.start ?? meeting.processed,
          end: Math.max(
            meeting.processed,
            currentLive?.end ?? 0,
            liveWords[liveWords.length - 1]?.end ?? 0,
          ),
          text: `${liveText}${liveText && partialText ? " " : ""}${partialText}`,
          edited: false,
        }
      : null;
  // Keep DOM mutation outside the reactive statement: assigning scrollTop to a
  // bound node inside it invalidates that same dependency and can starve rendering.
  async function followLive(_text: string, _partial: string, follow: boolean) {
    if (!follow) return;
    await tick();
    if (transcript) transcript.scrollTop = transcript.scrollHeight;
  }
  $: if (flowingFinalText || liveText || partialText)
    void followLive(flowingFinalText, `${liveText} ${partialText}`, autoScroll);
  $: segments =
    (translationView === "translation" ? savedTranslations.map(t=>({...t,edited:false})) : meeting?.segments ?? []).filter((s) =>
      s.text
        .toLocaleLowerCase("vi")
        .includes(searchText.toLocaleLowerCase("vi")),
    );
  $: continuousTranscript = !!meeting && ["recording", "paused"].includes(meeting.status);
  $: flowingFinalText = segments.map(s=>s.text).join(" ");
  $: if (sourcesInitialized) {
    try {
      localStorage.setItem(
        "may.preferences",
        JSON.stringify({ system, microphone, language, model, transcribe }),
      );
    } catch {
      /* Storage may be unavailable; session data still lives in SQLite. */
    }
  }
  $: if (meeting && autoScroll && meeting.segments.length !== lastCount) {
    lastCount = meeting.segments.length;
    tick().then(() => {
      if (transcript) transcript.scrollTop = transcript.scrollHeight;
    });
  }
  const clock = (seconds: number) => {
    const s = Math.floor(seconds);
    return `${Math.floor(s / 60)
      .toString()
      .padStart(2, "0")}:${(s % 60).toString().padStart(2, "0")}`;
  };
  const day = (iso: string) =>
    new Date(iso).toLocaleDateString(localeTags[uiLocale ?? "vi"], {
      day: "2-digit",
      month: "2-digit",
    });
  const time = (iso: string) =>
    new Date(iso).toLocaleTimeString(localeTags[uiLocale ?? "vi"], {
      hour: "2-digit",
      minute: "2-digit",
    });
  $: emptyHeading = searchText
    ? tr.emptySearch
    : !meeting
      ? tr.emptyReadyTitle
      : meeting.status === "recorded"
        ? tr.emptyRecordedTitle
        : meeting.status === "interrupted"
          ? tr.emptyInterruptedTitle
          : meeting.status === "completed"
            ? tr.emptyCompletedTitle
            : meeting.status === "processing"
              ? tr.emptyProcessingTitle
              : tr.emptyWaitingTitle;
  $: emptyDescription = searchText
    ? tr.emptySearchHint
    : !meeting
      ? tr.emptyReadyText
      : meeting.status === "recorded"
        ? tr.emptyRecordedText
        : meeting.status === "interrupted"
          ? tr.emptyInterruptedText
          : meeting.status === "completed"
            ? tr.emptyCompletedText
            : meeting.model === multilingualModel
              ? tr.emptyProcessingText
              : tr.emptyWaitingText;
  $: emptyDetail = `${tr.emptyDetail}`;
  $: states = {
    recording: tr.stateRecording,
    paused: tr.statePaused,
    processing: tr.stateProcessing,
    completed: tr.stateCompleted,
    recorded: tr.stateRecorded,
    interrupted: tr.stateInterrupted,
  } as Record<string, string>;
  async function refresh() {
    if (!desktop) return;
    try {
      const request = ++snapshotRequest;
      const snapshot = await invoke<Snapshot>("snapshot");
      if (request < snapshotApplied) return;
      snapshotApplied = request;
      data = snapshot;
      if (!sourcesInitialized && data.devices.length) {
        system =
          data.devices.find((d) => d.kind === "system" && d.default)?.id ?? "";
        microphone =
          data.devices.find((d) => d.kind === "microphone" && d.default)?.id ??
          "";
        sourcesInitialized = true;
      }
    } catch (e) {
      error = String(e);
    }
  }
  async function action(fn: () => Promise<void>) {
    pendingActions += 1;
    error = "";
    try {
      await fn();
      await refresh();
    } catch (e) {
      error = String(e);
    } finally {
      pendingActions -= 1;
    }
  }
  function notify(message: string) {
    toast = message;
    setTimeout(() => {
      toast = "";
    }, 3500);
  }
  async function checkForUpdate(manual = false) {
    if (
      !desktop ||
      updateChecking ||
      updatePhase === "downloading" ||
      updatePhase === "installing"
    )
      return;
    updateChecking = true;
    try {
      const found = await check({ timeout: 15000 });
      if (!found) {
        if (manual) notify(tr.updateCurrent);
        return;
      }
      if (updateInfo && updateInfo.version !== found.version) {
        void updateInfo.close().catch(() => {});
      }
      updateInfo = found;
      updatePhase = "available";
      updatePercent = 0;
    } catch {
      if (manual) notify(tr.updateFailed);
    } finally {
      updateChecking = false;
    }
  }
  async function installUpdate() {
    if (
      !updateInfo ||
      updatePhase === "downloading" ||
      updatePhase === "installing"
    )
      return;
    if (recording || testing || editing || busy) {
      error = tr.updateBlocked;
      return;
    }
    updatePhase = "downloading";
    updatePercent = 0;
    updateDownloaded = 0;
    updateTotalBytes = 0;
    try {
      await updateInfo.downloadAndInstall(
        (event) => {
          if (event.event === "Started") {
            updateTotalBytes = event.data.contentLength ?? 0;
            updatePercent = 0;
          } else if (event.event === "Progress") {
            updateDownloaded += event.data.chunkLength;
            updatePercent = updateTotalBytes
              ? Math.min(
                  100,
                  Math.round((updateDownloaded / updateTotalBytes) * 100),
                )
              : 0;
          } else if (event.event === "Finished") {
            updatePhase = "installing";
          }
        },
        { timeout: 120000, restartAfterInstall: true },
      );
    } catch {
      updatePhase = "available";
      notify(tr.updateFailed);
    }
  }
  function dismissUpdate() {
    void updateInfo?.close().catch(() => {});
    updateInfo = null;
    updatePhase = null;
    updatePercent = 0;
  }
  async function choose(s: Meeting) {
    if (testing) {
      error = tr.errorChooseTesting;
      meetingsOpen = false;
      return;
    }
    if (editing) {
      error = tr.errorChooseEditing;
      meetingsOpen = false;
      return;
    }
    error = "";
    audioRequest += 1;
    audioLoadingFor = "";
    lastExport = "";
    selected = s.id;
    page = "meeting";
    meetingsOpen = false;
    renamingId = "";
    title = s.title;
    notesDraft = s.notes;
    audioUrl = "";
    editing = "";
    searchText = "";
    smoothOpen = false;
    smoothPrompt = "";
    meetingsOpen = false;
    renamingId = "";
    summaryOpen = false;
    transcriptTab = "transcript";
    autoScroll = true;
    if (!["recording", "paused"].includes(s.status)) {
      try {
        await loadAudio(s.id);
      } catch {
        /* Partial recordings may not have a playable mix yet. */
      }
    }
  }
  function newMeeting() {
    meetingCaptions=false;meetingTranslate=false;localFallback=false;
    if (testing) {
      error = tr.errorNewTesting;
      return;
    }
    if (editing) {
      error = tr.errorNewEditing;
      return;
    }
    audioRequest += 1;
    audioLoadingFor = "";
    lastExport = "";
    selected = null;
    page = "meeting";
    title = "";
    audioUrl = "";
    notesDraft = "";
    editing = "";
    searchText = "";
    testUrl = "";
    testFeedback = "";
    testLabel = "";
    testWarnings = [];
    testQuiet = false;
    error = "";
    const preferred = taskAssigns.meeting?.transcription?.profileId ?? "";
    provider = preferred && preferred !== "__local__" ? "soniox" : "local";
    transcriptionProfileId = preferred === "__local__" ? "" : preferred;
    smoothOpen = false;
    smoothPrompt = "";
    transcriptView = "original";
    meetingsOpen = false;
    renamingId = "";
    summaryOpen = false;
    transcriptTab = "transcript";
  }
  async function start() {
    await action(async () => {
      if (transcribe && meetingCaptions && meetingTranslate && (provider !== "soniox" || !transcriptionProfileId)) {
        throw new Error("Hãy chọn cấu hình/model Soniox dùng chung cho chép lời và phụ đề ngay trong phần phụ đề cuộc họp.");
      }
      const s = await invoke<Meeting>("start", {
        options: {
          title,
          system: system || null,
          microphone: microphone || null,
          language,
          model: effectiveModel,
          speechQuality: meetingSpeechQuality,
          transcribe,
          provider,
          translationLanguage: transcribe && provider === "soniox" && meetingCaptions && meetingTranslate ? meetingTargetLang : null,
          captionsEnabled: transcribe && meetingCaptions && (provider === "soniox" || !meetingTranslate),
          localFallback: provider === "soniox" && localFallback,
          transcriptionFallback: provider === "soniox" ? taskAssigns.meeting?.transcription?.fallback ?? null : null,
          profileId:
            provider === "soniox" ? transcriptionProfileId || null : null,
        },
      });
      selected = s.id;
      await refresh();
      if (transcribe && meetingCaptions) {
        try {
        await showCaptionWindow(); captionRunning=true;
        } catch(e) { await invoke("stop_caption").catch(()=>{}); error=`Cuộc họp vẫn đang ghi, nhưng chưa bật được phụ đề: ${String(e)}`; }
      }
      selected = s.id;
      audioRequest += 1;
      audioLoadingFor = "";
      audioUrl = "";
      title = s.title;
      notesDraft = "";
    });
  }
  async function finish() {
    if (!active) return;
    stopOpen = true;
  }
  async function reallyFinish() {
    stopOpen = false;
    await action(async () => {
      await invoke("stop");
      notify(tr.notifyStopped);
    });
  }
  async function testSonioxConnection() {
    if (!sonioxKey.trim() && !data.soniox_configured) {
      error = tr.sonioxKeyRequired;
      return;
    }
    await action(async () => {
      await invoke("test_soniox_connection", {
        key: sonioxKey.trim() || null,
      });
      sonioxConnected = true;
      notify(tr.sonioxTestSuccess);
    });
  }
  async function saveSonioxKey(clear = false) {
    await action(async () => {
      try {
        await invoke("set_soniox_key", { key: clear ? "" : sonioxKey });
      } finally {
        sonioxKey = "";
      }
      data = { ...data, soniox_configured: !clear };
      if (clear) sonioxConnected = false;
      notify(clear ? tr.notifyKeyCleared : tr.notifyKeyKept);
    });
  }
  async function saveLlmKey(clear = false) {
    await action(async () => {
      try {
        await invoke("set_llm_key", { key: clear ? "" : smoothKey });
      } finally {
        smoothKey = "";
      }
      data = { ...data, llm_configured: !clear };
      if (clear) llmConnectionSignature = "";
      notify(clear ? tr.notifyAiKeyCleared : tr.notifyAiKeyKept);
    });
  }
  async function testLlmConnection() {
    const profileId = summaryOpen ? summaryProfileId : smoothProfileId;
    const chosenModel = summaryOpen ? summaryModel : smoothModel;
    const chosenProvider =
      profileOf(profileId, aiProfiles)?.provider ?? smoothProvider;
    const chosenBase =
      profileOf(profileId, aiProfiles)?.base_url ?? smoothBaseUrl;
    await action(async () => {
      if (profileId) {
        await invoke("test_ai_connection", {
          input: {
            provider: chosenProvider,
            apiKey: null,
            baseUrl: chosenBase,
            model: chosenModel,
            profileId: profileId,
          },
        });
      } else {
        if (!smoothKey.trim() && !data.llm_configured) {
          error = tr.aiKeyRequired;
          return;
        }
        await invoke("test_llm_connection", {
          provider: chosenProvider,
          model: chosenModel,
          baseUrl: chosenBase,
          key: smoothKey.trim() || null,
        });
      }
      llmConnectionSignature = llmConfigSignature;
      notify(tr.aiConnected);
    });
  }
  async function fetchModels() {
    if (!smoothKey.trim() && !data.llm_configured) {
      error = tr.aiKeyRequired;
      return;
    }
    await action(async () => {
      const models = await invoke<string[]>("fetch_llm_models", {
        provider: smoothProvider,
        baseUrl: smoothBaseUrl,
        key: smoothKey.trim() || null,
      });
      smoothModels = models;
      modelPickerSearch = "";
      modelPickerOpen = true;
      notify(fill(tr.aiModelsFetched, { count: models.length }));
    });
  }
  async function smoothTranscript() {
    if (!meeting || !canSmooth) return;
    if (!llmConnected) {
      error = tr.aiConnectFirst;
      return;
    }
    const id = meeting.id;
    const profile = profileOf(smoothProfileId, aiProfiles);
    await action(async () => {
      if (!taskAssigns.meeting?.smooth?.fallback) {
      if (profile) {
        await invoke("test_ai_connection", {
          input: {
            provider: profile.provider,
            apiKey: null,
            baseUrl: profile.base_url,
            model: smoothModel,
            profileId: profile.id,
          },
        });
      } else {
        await invoke("test_llm_connection", {
          provider: smoothProvider,
          model: smoothModel,
          baseUrl: smoothBaseUrl,
          key: smoothKey.trim() || null,
        });
      }
      }
      llmConnectionSignature = llmConfigSignature;
      const updated = await invoke<Meeting>("smooth_transcript", {
        id,
        provider: profile?.provider ?? smoothProvider,
        model: smoothModel,
        baseUrl: profile?.base_url ?? smoothBaseUrl,
        prompt: smoothPrompt,
        key: !profile ? smoothKey.trim() || null : null,
        profileId: profile?.id ?? null,
        fallback: taskAssigns.meeting?.smooth?.fallback ?? null,
      });
      if (id === meeting?.id) {
        data = {
          ...data,
          sessions: data.sessions.map((s) => (s.id === id ? updated : s)),
        };
        smoothOpen = false;
        smoothPrompt = "";
        transcriptView = "smoothed";
        notify(tr.notifySmoothed);
      }
    });
  }
  async function copySmoothed() {
    try {
      await navigator.clipboard.writeText(meeting?.smoothed?.text ?? "");
      notify(tr.smoothCopied);
    } catch (e) {
      error = String(e);
    }
  }
  async function createSummary() {
    if (!meeting || !canSmooth) return;
    if (!llmConnected) {
      error = tr.aiConnectFirst;
      return;
    }
    const id = meeting.id;
    const profile = profileOf(summaryProfileId, aiProfiles);
    await action(async () => {
      if (!taskAssigns.meeting?.summary?.fallback) {
      if (profile) {
        await invoke("test_ai_connection", {
          input: {
            provider: profile.provider,
            apiKey: null,
            baseUrl: profile.base_url,
            model: summaryModel,
            profileId: profile.id,
          },
        });
      } else {
        await invoke("test_llm_connection", {
          provider: smoothProvider,
          model: summaryModel,
          baseUrl: smoothBaseUrl,
          key: smoothKey.trim() || null,
        });
      }
      }
      llmConnectionSignature = llmConfigSignature;
      const updated = await invoke<Meeting>("summarize_transcript", {
        id,
        provider: profile?.provider ?? smoothProvider,
        model: summaryModel,
        baseUrl: profile?.base_url ?? smoothBaseUrl,
        prompt: summaryPrompt,
        style: summaryStyle,
        source: summarySource,
        key: !profile ? smoothKey.trim() || null : null,
        profileId: profile?.id ?? null,
        fallback: taskAssigns.meeting?.summary?.fallback ?? null,
      });
      if (id === meeting?.id) {
        data = {
          ...data,
          sessions: data.sessions.map((s) => (s.id === id ? updated : s)),
        };
        transcriptTab = "summary";
        summaryOpen = false;
        summaryPrompt = "";
        notify(tr.notifySummaryCreated);
      }
    });
  }
  async function deleteSummary() {
    if (!meeting?.summary) return;
    const id = meeting.id;
    await action(async () => {
      const updated = await invoke<Meeting>("delete_summary", { id });
      if (id === meeting?.id) {
        data = {
          ...data,
          sessions: data.sessions.map((s) => (s.id === id ? updated : s)),
        };
        transcriptTab = "transcript";
        notify(tr.notifySummaryDeleted);
      }
    });
  }
  async function exportSummary(format: "md" | "txt") {
    if (!meeting?.summary) return;
    const id = meeting.id;
    await action(async () => {
      const destination = await invoke<string | null>("choose_export_path", {
        id,
        format,
      });
      if (!destination) return;
      const path = await invoke<string>("export_session", {
        id,
        format,
        path: destination,
        version: "summary",
      });
      if (selected === id) {
        lastExport = path;
        notify(tr.notifyExportSaved);
      }
    });
  }
  async function copySummary() {
    try {
      await navigator.clipboard.writeText(meeting?.summary?.text ?? "");
      notify(tr.summaryCopied);
    } catch (e) {
      error = String(e);
    }
  }
  async function copyTranscript() {
    if (!meeting) return;
    const original=meeting.segments.map(s=>s.text.trim()).filter(Boolean).join("\n");
    const translated=savedTranslations.map(s=>s.text.trim()).filter(Boolean).join("\n");
    const text=translationView==="translation"?translated:translationView==="bilingual"?`BẢN GỐC\n${original}\n\nBẢN DỊCH (${visibleTranslationLanguage})\n${translated}`:original;
    if (!text) return;
    try {
      await navigator.clipboard.writeText(text);
      notify(tr.transcriptCopied);
    } catch (e) {
      error = String(e);
    }
  }
  async function togglePause() {
    if (active)
      await action(async () => {
        await invoke("pause", { paused: active!.status !== "paused" });
      });
  }
  async function download(id = effectiveModel) {
    const m = data.models.find((m) => m.id === id);
    if (!m || progress) return;
    progress = { id: m.id, bytes: 0, total: m.bytes };
    error = "";
    try {
      await invoke("download_model", { id: m.id });
      notify(tr.notifyModelReady);
      await refresh();
    } catch (e) {
      error = String(e);
    } finally {
      progress = null;
    }
  }
  async function update(fields: Record<string, unknown>) {
    if (!selected) return;
    const id = selected;
    const payload = {
      id,
      title: null,
      notes: null,
      segmentId: null,
      text: null,
      ...fields,
    };
    const operation = pendingEdits
      .catch(() => {})
      .then(() => invoke("edit_session", payload));
    pendingEdits = operation;
    await operation;
    await refresh();
  }
  async function exportText(format: "md" | "txt" | "srt") {
    if (!meeting) return;
    const id = meeting.id;
    await action(async () => {
      await pendingEdits;
      const destination = await invoke<string | null>("choose_export_path", {
        id,
        format,
      });
      if (!destination) return;
      const path = await invoke<string>("export_session", {
        id,
        format,
        path: destination,
        version: exportingVersion,
      });
      if (selected === id) {
        lastExport = path;
        notify(tr.notifyExportSaved);
      }
    });
  }
  async function exportAudio() {
    if (!meeting) return;
    const id = meeting.id;
    await action(async () => {
      const destination = await invoke<string | null>(
        "choose_audio_export_path",
        { id },
      );
      if (!destination) return;
      const path = await invoke<string>("export_audio", {
        id,
        path: destination,
      });
      if (selected === id) {
        lastExport = path;
        notify(tr.notifyExportSaved);
      }
    });
  }
  let segmentPlaybackEnd: number | null = null;
  let segmentTimer: ReturnType<typeof setTimeout> | undefined;
  function clearSegmentTimer() { if(segmentTimer) clearTimeout(segmentTimer); segmentTimer=undefined; }
  function checkSegmentPlayback() {
    if(audio && segmentPlaybackEnd!==null && audio.currentTime>=segmentPlaybackEnd) {
      const end=segmentPlaybackEnd; segmentPlaybackEnd=null; clearSegmentTimer(); audio.pause(); audio.currentTime=end;
    }
  }
  function scheduleSegmentStop() {
    clearSegmentTimer();
    if(audio && !audio.paused && segmentPlaybackEnd!==null) {
      segmentTimer=setTimeout(()=>{if(audio && segmentPlaybackEnd!==null){audio.currentTime=segmentPlaybackEnd;checkSegmentPlayback();}},Math.max(0,(segmentPlaybackEnd-audio.currentTime)*1000/audio.playbackRate));
    }
  }
  onDestroy(clearSegmentTimer);
  async function seek(seconds: number, end?: number) {
    if(audio) {
      clearSegmentTimer();segmentPlaybackEnd=end!==undefined && end>seconds ? end : null;
      audio.currentTime=Math.max(0,seconds);
      await audio.play().catch(e=>{error=String(e);});scheduleSegmentStop();
    }
  }
  async function remove() {
    const id = deleteTarget?.id ?? selected;
    if (!id) return;
    await action(async () => {
      await invoke("delete_session", { id });
      deleteOpen = false;
      deleteTarget = null;
      editing = "";
      if (selected === id) newMeeting();
      else await refresh();
    });
  }
  function startRename(s: Meeting) {
    renamingId = s.id;
    renameDraft = s.title;
    tick().then(() => document.getElementById(`rename-${s.id}`)?.focus());
  }
  async function saveRename(s: Meeting) {
    const next = renameDraft.trim();
    if (!next) {
      error = tr.titleRequired;
      return;
    }
    await action(async () => {
      await invoke("edit_session", { id: s.id, title: next });
      if (selected === s.id) title = next;
      renamingId = "";
      notify(tr.notifyRenamed);
      await refresh();
    });
  }
  function askDelete(s: Meeting) {
    deleteTarget = s;
    deleteOpen = true;
  }
  function clearTest() {
    testAudio?.pause();
    testUrl = "";
    testFeedback = "";
    testLabel = "";
    testWarnings = [];
  }
  async function test(kind: "system" | "microphone" | "both") {
    if (testing || busy || data.active_id) return;
    clearTest();
    testing = true;
    testKind = kind;
    testSeconds = 5;
    const testSystem = kind !== "microphone" ? system : "";
    const testMic = kind !== "system" ? microphone : "";
    const label =
      kind === "system"
        ? tr.audioMeeting
        : kind === "microphone"
          ? tr.micShort
          : tr.audioBoth;
    const instruction =
      kind === "microphone"
        ? tr.testPromptMic
        : kind === "system"
          ? tr.testPromptSystem
          : tr.testPromptBoth;
    testLabel = `${fill(tr.testChecking, { label })} ${instruction}`;
    const began = performance.now();
    const countdown = setInterval(() => {
      testSeconds = Math.max(
        0,
        5 - Math.floor((performance.now() - began) / 1000),
      );
    }, 200);
    try {
      await action(async () => {
        const result = await invoke<{
          path: string;
          peaks: Record<string, number>;
          warnings: string[];
        }>("test_audio", {
          system: testSystem || null,
          microphone: testMic || null,
        });
        testUrl = convertFileSrc(result.path);
        testWarnings = result.warnings;
        testLabel = fill(tr.testRecorded, { label });
        const quiet: string[] = [];
        const heard: string[] = [];
        if (testSystem)
          (result.peaks.system >= 0.001 ? heard : quiet).push(tr.audioMeeting);
        if (testMic)
          (result.peaks.microphone >= 0.001 ? heard : quiet).push(tr.micShort);
        testFeedback = heard.length
          ? fill(tr.testHeard, { sources: heard.join(" + ") })
          : "";
        if (quiet.length)
          testFeedback += ` ${fill(tr.testQuiet, { sources: quiet.join(" + ") })}`;
        testQuiet = quiet.length > 0;
        notify(tr.testDone);
        tick().then(() => {
          testAudio?.load();
        });
      });
      if (!testUrl) testLabel = tr.testNoRecording;
    } finally {
      clearInterval(countdown);
      testing = false;
    }
  }
  async function loadAudio(id: string) {
    clearSegmentTimer(); segmentPlaybackEnd=null;
    if (audioLoadingFor === id) return;
    const request = ++audioRequest;
    audioLoadingFor = id;
    try {
      const path = await invoke<string>("audio_path", { id });
      if (selected === id && request === audioRequest)
        audioUrl = convertFileSrc(path);
    } finally {
      if (request === audioRequest) audioLoadingFor = "";
    }
  }
  onMount(() => {
    processingLevels=parseProcessingLevels(localStorage.getItem(SPEECH_LEVELS_KEY));
    try {
      const saved=JSON.parse(localStorage.getItem("may.meeting.local-models")??"{}");
      if(saved&&typeof saved==="object"&&!Array.isArray(saved))localMeetingModels=Object.fromEntries(Object.entries(saved).filter(([lang,value])=>typeof value==="string"&&compatibleLocalModels(lang).includes(value))) as Record<string,string>;
    } catch { /* Invalid saved model choices use existing language defaults. */ }
    try {
      const stored = JSON.parse(
        localStorage.getItem("may.preferences") ?? "null",
      );
      if (stored) {
        system = typeof stored.system === "string" ? stored.system : "";
        microphone =
          typeof stored.microphone === "string" ? stored.microphone : "";
        language = ["vi", "en", "ko", "zh", "multi"].includes(stored.language)
          ? stored.language
          : "vi";
        transcribe =
          typeof stored.transcribe === "boolean" ? stored.transcribe : true;
        sourcesInitialized = true;
      }
    } catch {
      /* Invalid preferences fall back to available devices. */
    }
    try {
      const llm = JSON.parse(
        localStorage.getItem("may.llm_preferences") ?? "null",
      );
      if (llm) {
        if (isSmoothProvider(llm.provider)) {
          smoothProvider = llm.provider;
          smoothModel =
            typeof llm.model === "string" && llm.model.trim()
              ? llm.model
              : smoothDefaults[llm.provider as SmoothProvider];
        }
        if (typeof llm.baseUrl === "string") smoothBaseUrl = llm.baseUrl;
      }
    } catch {
      /* Invalid AI preferences fall back to defaults. */
    } finally {
      smoothPrefsInitialized = true;
    }
    let alive = true;
    let unlisten: (() => void)[] = [];
    void refresh();
    const updateTimer = setTimeout(() => {
      if (desktop) void checkForUpdate();
    }, 5000);
    const timer = setInterval(async () => {
      await refresh();
      if (
        meeting &&
        !audioUrl &&
        ["completed", "recorded", "interrupted"].includes(meeting.status)
      ) {
        try {
          await loadAudio(meeting.id);
        } catch {
          /* Keep the transcript usable if audio is unavailable. */
        }
      }
    }, 1300);
    if (desktop) {
      Promise.all([
        listen<{
          id: string;
          audit_epoch_unix_ms?: number;
          audit_emit_ms?: number;
          unavailable?: boolean;
          update?: {
            text: string;
            start: number;
            end: number;
            speech_end?: number | null;
            final_text: boolean;
            words: Segment[];
          };
        }>("live-transcript", (e) => {
          const receivedAt = Date.now();
          const { id, update, unavailable } = e.payload;
          const old = liveUpdates[id] ?? { words: [], partial: "", end: 0 };
          liveUpdates = {
            ...liveUpdates,
            [id]: unavailable
              ? { ...old, partial: "", unavailable: true }
              : update
                ? {
                    words: [
                      ...old.words.filter(
                        (w) =>
                          w.end >
                          (data.sessions.find((s) => s.id === id)?.processed ??
                            0),
                      ),
                      ...(update.final_text ? update.words : []),
                    ],
                    partial: update.final_text ? "" : update.text,
                    partialWords: update.final_text ? [] : update.words,
                    end: update.end,
                  }
                : old,
          };
          const epoch = e.payload.audit_epoch_unix_ms;
          if (typeof epoch === "number" && Number.isFinite(epoch)) {
            void (async () => {
              await tick();
              // Two frame callbacks observe the previous frame after it could paint.
              await new Promise<void>((resolve) =>
                requestAnimationFrame(() =>
                  requestAnimationFrame(() => resolve()),
                ),
              );
              if (
                meeting?.id !== id ||
                page !== "meeting" ||
                document.visibilityState !== "visible"
              )
                return;
              const text = document
                .querySelector("[data-live-caption]")
                ?.textContent?.trim();
              if (text)
                await invoke("report_live_paint", {
                  id,
                  text,
                  elapsedMs: Date.now() - epoch,
                  receivedMs: receivedAt - epoch,
                  emittedMs: e.payload.audit_emit_ms ?? receivedAt - epoch,
                  audioMs: (update?.end ?? 0) * 1000,
                  speechEndMs:
                    typeof update?.speech_end === "number"
                      ? update.speech_end * 1000
                      : null,
                  finalText: !!update?.final_text,
                });
            })().catch(() => {
              /* Optional local QA must never interrupt recording. */
            });
          }
        }),
        listen<{ kind: string; value: number }>("audio-level", (e) => {
          const value = Math.min(
            100,
            Math.sqrt(Math.max(0, e.payload.value)) * 170,
          );
          if (e.payload.kind === "system") systemLevel = value;
          else micLevel = value;
        }),
        listen<{ id: string; bytes: number; total: number }>(
          "model-progress",
          (e) => {
            progress = e.payload;
          },
        ),
        listen("close-blocked", () => {
          error = tr.errorCloseBlocked;
        }),
        listen<string>("storage-error", (e) => {
          error = e.payload;
        }),
      ]).then((fns) => {
        if (alive) unlisten = fns;
        else fns.forEach((fn) => fn());
      });
    }
    return () => {
      alive = false;
      clearTimeout(updateTimer);
      clearInterval(timer);
      unlisten.forEach((fn) => fn());
    };
  });
  // ---- AI profiles, task assignment and captions ----
  function profileOf(id: string | undefined, profiles: AiProfileSummary[]) {
    return profiles.find((p) => p.id === id);
  }
  function chatProfiles(profiles: AiProfileSummary[]) {
    return profiles.filter((p) =>
      (chatProviders as readonly string[]).includes(p.provider),
    );
  }
  function captionableProfiles(profiles: AiProfileSummary[]) {
    return profiles.filter((p) =>
      (captionProviders as readonly string[]).includes(p.provider),
    );
  }
  function sonioxProfiles(profiles: AiProfileSummary[]) {
    return profiles.filter((p) => p.provider === "soniox");
  }
  async function refreshProfiles() {
    try {
      aiProfiles = await invoke<AiProfileSummary[]>("list_ai_profiles");
    } catch (e) {
      error = String(e);
    }
  }
  function openAddProfile() {
    confGeneration++;
    confTesting = false;
    confEditingId = null;
    confName = "";
    confProvider = "openai";
    confBaseUrl = "";
    confKey = "";
    confModel = "";
    confModels = [];
    confTested = false;
    confTestSignature = "";
    confStatusText = "";
    confOpen = true;
  }
  function openEditProfile(p: AiProfileSummary) {
    confGeneration++;
    confTesting = false;
    confEditingId = p.id;
    confName = p.name;
    confProvider = p.provider;
    confBaseUrl = p.provider === "groq" ? "" : p.base_url;
    confKey = "";
    confModel = "";
    confModels = [];
    confTested = false;
    confTestSignature = "";
    confStatusText = "";
    confOpen = true;
  }
  function confSignature() {
    return [
      confEditingId ?? "new",
      confProvider,
      confBaseUrl,
      confKey,
      confModel,
    ].join("|");
  }
  async function testProfile() {
    if (!confKey.trim() && !confEditingId) {
      confStatusText = tr.aiKeyRequired;
      return;
    }
    confTesting = true;
    confTested = false;
    confStatusText = tr.aiTesting;
    const signature = confSignature();
    const generation = ++confGeneration;
    let timeout: ReturnType<typeof setTimeout>;
    try {
      const result = await Promise.race([
        invoke<string>("test_ai_connection", {
          input: {
            provider: confProvider,
            apiKey: confKey.trim(),
            baseUrl: confProvider === "groq" ? "" : confBaseUrl,
            model: "",
            profileId: confEditingId,
          },
        }),
        new Promise<never>((_, reject) => {
          timeout = setTimeout(
            () => reject(new Error(tr.aiTestTimeout)),
            38_000,
          );
        }),
      ]);
      if (generation !== confGeneration || signature !== confSignature())
        return;
      confTested = true;
      confTestSignature = signature;
      confStatusText = result === "ok" ? tr.aiConnected : result;
    } catch (e) {
      if (generation === confGeneration) {
        confStatusText = String(e);
        confTested = false;
      }
    } finally {
      clearTimeout(timeout!);
      if (generation === confGeneration) confTesting = false;
    }
  }

  async function fetchConfModels() {
    if (!confKey.trim() && !confEditingId) {
      confStatusText = tr.aiKeyRequired;
      return;
    }
    confStatusText = tr.aiFetchingModels;
    try {
      await action(async () => {
        confModels = await invoke<string[]>("fetch_llm_models", {
          provider: confProvider,
          baseUrl: confBaseUrl,
          key: confKey.trim() || null,
          profileId: confEditingId,
        });
        if(confProvider === "groq")confModels=confModels.filter(m=>["openai/gpt-oss-20b","openai/gpt-oss-120b","qwen/qwen3.8-27b","whisper-large-v3","whisper-large-v3-turbo"].includes(m));
        confStatusText = confModels.length
          ? fill(tr.aiModelsFetched, { count: confModels.length })
          : tr.aiUnusedModels;
      });
    } catch (e) {
      confStatusText = String(e);
    }
  }
  async function saveProfile() {
    if (!confTested || confTestSignature !== confSignature()) {
      confStatusText = tr.aiTestBeforeSave;
      return;
    }
    pendingActions += 1;
    try {
      await invoke("save_ai_profile", {
        input: {
          id: confEditingId,
          name: confName,
          provider: confProvider,
          baseUrl: confProvider === "groq" ? "" : confBaseUrl,
          apiKey: confKey,
        },
      });
      closeProfileDialog();
      await refreshProfiles();
      notify(confEditingId ? tr.aiProfileUpdated : tr.aiProfileAdded);
    } catch (e) {
      confStatusText = String(e);
    } finally {
      pendingActions -= 1;
    }
  }
  async function deleteProfile(p: AiProfileSummary) {
    if (editing) {
      error = tr.errorSettingsEditing;
      return;
    }
    await action(async () => {
      await invoke("delete_ai_profile", { id: p.id });
      await refreshProfiles();
      notify(fill(tr.aiProfileDeleted, { name: p.name }));
    });
  }
  onMount(() => {
    if (!desktop) return;
    const un: Array<() => void> = [];
    void refreshProfiles();
    try {
      const m = JSON.parse(
        localStorage.getItem("may.task_assign.meeting") ?? "null",
      );
      if (m) {
        taskAssigns = { ...taskAssigns, meeting: m as TaskAssign };
        const preferred = m.transcription?.profileId ?? "";
        if (preferred) {
          provider = preferred === "__local__" ? "local" : "soniox";
          transcriptionProfileId = preferred === "__local__" ? "" : preferred;
        }
      }
    } catch {
      /* ignore */
    }
    try {
      const c = JSON.parse(
        localStorage.getItem("may.task_assign.captionsonly") ?? "null",
      );
      if (c) taskAssigns = { ...taskAssigns, captionsonly: c as TaskAssign };
    } catch {
      /* ignore */
    }
    try {
      const saved = JSON.parse(
        localStorage.getItem("may.caption.standalone") ?? "null",
      );
      const savedAssign = saved ?? taskAssigns.captionsonly?.caption;
      if (savedAssign) {
        captionProfileId = savedAssign.profileId ?? "";
        captionModel = savedAssign.model ?? "";
        captionTargetLang = savedAssign.targetLang ?? "vi";
        captionDevice = savedAssign.device ?? "";
        captionTranslate = savedAssign.translate !== false;
        captionSourceLang = savedAssign.sourceLang ?? "auto";
      }
    } catch {
      /* invalid saved preferences */
    }
    let migratedLocal = false;
    for(const [mode, assignments] of Object.entries(taskAssigns)) {
      for(const task of ["transcription","caption"] as const) {
        const choice=assignments[task];
        if(choice?.profileId === "__local__" && choice.model === "phowhisper-small") {
          taskAssigns={...taskAssigns,[mode]:{...taskAssigns[mode],[task]:{...choice,model:task === "transcription" && language === "en" ? "moonshine-en" : "moonshine-vi"}}};
          migratedLocal=true;
        }
      }
    }
    if(captionProfileId === "__local__" && captionModel === "phowhisper-small") {
      captionModel=captionSourceLang === "en" ? "moonshine-en" : "moonshine-vi";
      captionSourceLang=captionModel === "moonshine-en" ? "en" : "vi";
      migratedLocal=true;
    }
    if(migratedLocal)notify(tr.modelMigrated);
    assignmentsReady = true;
    listen<{
      state: string;
      provider: string;
      sending: boolean;
      message: string;
    }>("caption-status", ({ payload }) => {
      captionStatusRevision++;
      captionProvider = payload.provider;
      captionRunning =
        payload.sending ||
        (payload.state !== "idle" && payload.state !== "error");
      captionSending = payload.sending;
      captionStatusMessage = payload.message;
      captionError = payload.state === "error" ? payload.message : "";
      if(["idle","error","connecting"].includes(payload.state)){captionPresentation.clear();applyCaptionPreview();}
    })
      .then((fn) => un.push(fn))
      .catch(() => {});
    listen<{task:string;from:string;to:string}>("ai-fallback",({payload})=>{
      captionFallbackMessage=`Đã chuyển từ ${payload.from} sang ${payload.to} theo lựa chọn dự phòng.`;
      notify(captionFallbackMessage);
    }).then(fn=>un.push(fn)).catch(()=>{});
    listen<CaptionCue>(
      "caption-text",
      ({ payload }) => {
        captionTextRevision++;
        captionPresentation.push(payload);applyCaptionPreview();
      },
    )
      .then((fn) => un.push(fn))
      .catch(() => {});
    listen<boolean>("caption-mode",({payload})=>{captionTextRevision++;captionPresentation.setMode(payload);applyCaptionPreview();}).then(fn=>un.push(fn)).catch(()=>{});
    const syncCaptionStyle=()=>{captionBilingual=readCaptionStyle().showOriginal;applyCaptionPreview();};
    listen("caption-style",syncCaptionStyle).then(fn=>un.push(fn)).catch(()=>{});
    const captionStorage=(event:StorageEvent)=>{if(event.key===CAPTION_STYLE_KEY)syncCaptionStyle();};
    window.addEventListener("storage",captionStorage);un.push(()=>window.removeEventListener("storage",captionStorage));
    const initialCaptionRevision = captionStatusRevision;
    const initialCaptionTextRevision = captionTextRevision;
    void Promise.all([
      invoke<boolean>("caption_running"),
      invoke<{status:{provider:string;sending:boolean;message:string;state:string};translating:boolean;caption:CaptionCue;paired:CaptionCue|null}>("caption_snapshot"),
    ])
      .then(([running, snapshot]) => {
        if (captionStatusRevision !== initialCaptionRevision) return;
        captionRunning = running;
        captionProvider = snapshot.status.provider;
        captionSending = snapshot.status.sending;
        captionStatusMessage = snapshot.status.message;
        captionError = snapshot.status.state === "error" ? snapshot.status.message : "";
        if (running && captionTextRevision === initialCaptionTextRevision) {
          captionPresentation.restore(snapshot);
          applyCaptionPreview();
        }
      })
      .catch(() => {})
      .finally(()=>{captionStateReady=true;});
    return () => un.forEach((fn) => fn());
  });
  function chooseConfProvider(value: string) {
    confProvider = value;
    confModel = "";
    if (
      confEditingId &&
      profileOf(confEditingId, aiProfiles)?.provider !== value
    )
      confKey = "";
    confTested = false;
    confStatusText = "";
    confModels = [];
    if (value !== "openai-compatible") confBaseUrl = "";
  }
  function useSessionProfile(profileId: string) {
    const profile = profileOf(profileId, aiProfiles);
    if (
      profile &&
      (chatProviders as readonly string[]).includes(profile.provider)
    ) {
      smoothProvider = profile.provider as SmoothProvider;
      smoothBaseUrl = profile.base_url;
      smoothModel = textModels(profileId, aiProfiles)[0] ?? "";
    }
  }
  async function testCaptionModel() {
    if(captionProfileId === "__local__")return;
    const p = profileOf(captionProfileId, aiProfiles);
    if (!p || !captionModel || captionTesting) return;
    captionTesting = true;
    captionError = "";
    captionStatusMessage = tr.aiTesting;
    try {
      captionStatusMessage = await invoke<string>("test_ai_connection", {
        input: {
          provider: p.provider,
          apiKey: "",
          baseUrl: p.base_url,
          model: captionModel,
          profileId: p.id,
        },
      });
    } catch (e) {
      captionError = String(e);
      captionStatusMessage = "";
    } finally {
      captionTesting = false;
    }
  }
  async function startCaptionSession(mode: "meeting" | "standalone") {
    captionFallbackMessage = "";
    const assign = taskAssigns[mode === "meeting" ? "meeting" : "captionsonly"];
    const profileId =
      mode === "standalone"
        ? captionProfileId
        : active?.speech && !active.speech.local_fallback ? (active.speech.profile_id ?? "") : (assign?.caption?.profileId ?? "");
    const translate = mode === "standalone" ? captionTranslate : meetingTranslate;
    const localSource = mode === "meeting" && !translate && (!active?.speech || active.speech.local_fallback);
    if (!profileId && !localSource) {
      error = mode === "meeting" ? tr.captionNeedConfig : tr.captionNeedProfile;
      return;
    }
    const targetLang =
      mode === "standalone"
        ? captionTargetLang
        : meetingTargetLang;
    const model =
      mode === "standalone"
        ? captionModel
        : active?.speech && !active.speech.local_fallback ? active.speech.model : assign?.caption?.model ||
          captionModels(profileId, aiProfiles)[0] ||
          "";
    await action(async () => {
      await invoke("start_caption", {
        options: {
          provider: "",
          apiKey: "",
          baseUrl: "",
          model,
          speechQuality: mode === "standalone" ? captionSpeechQuality : active?.speech_quality ?? meetingSpeechQuality,
          targetLanguage: targetLang,
          translate,
          sourceLanguage: mode === "standalone" ? captionSourceLang : active?.language ?? "auto",
          fallback: profileId === "__local__" || (mode === "meeting" && active?.speech && !active.speech.local_fallback) ? null : assign?.caption?.fallback ?? null,
          device: mode === "standalone" ? captionDevice || null : null,
          mode,
          sessionDir: null,
          profileId,
        },
      });
      captionRunning = true;
      captionError = "";
      try {
        await showCaptionWindow();
      } catch (e) {
        await invoke("stop_caption");
        throw e;
      }
      notify(tr.captionStarted);
    });
  }
  async function stopCaptionSession() {
    await action(async () => {
      await invoke("stop_caption");
      // The stop command signals the worker; it can still be flushing the last
      // translation. Do not offer a new session until the backend releases it.
      const deadline = Date.now() + 15000;
      while (await invoke<boolean>("caption_running")) {
        if (Date.now() >= deadline) throw new Error(tr.captionStopTimeout);
        await new Promise((resolve) => setTimeout(resolve, 100));
      }
      try { await closeCaptionWindow(); } catch { /* already closed */ }
      captionRunning = false;
      captionSending = false;
      captionText = "";
      captionOriginal = "";
      notify(tr.captionStopped);
    });
  }
  async function toggleMeetingCaption() {
    if (captionRunning) {
      await stopCaptionSession();
      return;
    }
    await startCaptionSession("meeting");
  }
  function setAssignment(
    task: keyof TaskAssign,
    value: Partial<TaskAssign[keyof TaskAssign]> | undefined,
  ) {
    const current = taskAssigns[taskMode] ?? {};
    const next: TaskAssign = { ...current };
    if (value === undefined) {
      delete next[task];
    } else {
      (next as Record<string, unknown>)[task] = value;
    }
    taskAssigns = { ...taskAssigns, [taskMode]: next };
    if (taskMode === "captionsonly" && task === "caption") {
      captionTranslate = next.caption?.translate !== false;
      captionProfileId = next.caption?.profileId ?? "";
      captionModel =
        next.caption?.model ||
        captionModels(captionProfileId, aiProfiles, captionTranslate)[0] ||
        "";
      captionTargetLang = next.caption?.targetLang ?? "vi";
    }
    persistAssignments();
  }
  function persistAssignments() {
    try {
      for (const mode of ["meeting", "captionsonly"] as const) {
        localStorage.setItem(
          `may.task_assign.${mode}`,
          JSON.stringify(taskAssigns[mode] ?? {}),
        );
      }
    } catch {
      /* best-effort */
    }
  }
</script>

<svelte:head><title>Innote</title></svelte:head>

{#if !uiLocale}
  <div class="language-gate">
    <section class="language-panel" aria-labelledby="language-title">
      <div class="language-brand">
        <span class="brand-mark"><Logo size={24} /></span>
        <div>
          <strong>Innote</strong>
          <small>{tr.appTagline}</small>
        </div>
      </div>
      <p class="language-kicker">Innote</p>
      <h1 id="language-title">{tr.gateTitle}</h1>
      <p class="language-subtitle">{tr.gateSubtitle}</p>
      <div class="language-options">
        {#each locales as value}
          <button
            class="language-option"
            onclick={() => setUiLocale(value)}
            lang={localeTags[value]}
          >
            <span class="language-native">{localeNames[value]}</span>
            <span class="language-english">{localeEnglishNames[value]}</span>
          </button>
        {/each}
      </div>
      <p class="language-note">{tr.gateNote}</p>
    </section>
  </div>
{:else}
  <div class="app-shell" lang={localeTags[uiLocale ?? "vi"]}>
    <WhatsNewDialog version={appVersion} locale={uiLocale ?? "vi"} existingInstall={hadPreviousInstallation} ready={desktop && captionStateReady && !!data.data_dir && !data.active_id && !captionRunning && !testing && !confOpen && !meetingsOpen && !summaryOpen && !smoothOpen && !modelPickerOpen && !deleteOpen && !stopOpen && updatePhase !== "downloading" && updatePhase !== "installing"}/>
    <aside class="sidebar" aria-label={tr.navAria}>
      <button
        class="brand"
        style="background:transparent;padding:0;text-align:left"
        onclick={newMeeting}
        ><span class="brand-mark"><Logo size={24} /></span><span
          >Innote<span class="brand-sub">{tr.appTagline}</span></span
        ></button
      >
      <button class="new-button" onclick={newMeeting}
        ><Plus size={18} /> {tr.newMeeting}</button
      >
      <button
        class="sidebar-nav-button"
        class:chosen={page === "caption"}
        onclick={() => {
          if (testing) {
            error = tr.errorSettingsTesting;
            return;
          }
          page = "caption";
          taskMode = "captionsonly";
        }}><Captions size={18} /> {tr.captionPage}</button
      >
      <button
        class="meetings-button"
        class:chosen={meetingsOpen}
        aria-label={tr.meetingListButton}
        onclick={() => {
          meetingsOpen = true;
        }}
      >
        <List size={18} />
        <span
          ><strong>{tr.meetingListButton}</strong><small
            >{tr.yourMeetings} · {tr.meetingListHint}</small
          ></span
        ><span class="meetings-count">{data.sessions.length}</span>
      </button>
      <div class="sidebar-author-group">
        <button class="feedback-link" onclick={() => openAuthor("feedback")}>
          <AlertCircle size={15} />
          <span>{tr.feedbackButton}</span>
        </button>
        <div class="author-card">
          <div class="author-name">
            <span class="author-label">{tr.author}</span><span>Alex Linh</span>
          </div>
          <div class="author-role">{tr.authorRole}</div>
          <div class="author-links">
            <button
              class="author-link zalo"
              title="Zalo: 0962 239 818"
              aria-label={tr.zaloContactAria}
              onclick={() => openAuthor("zalo")}
              ><svg viewBox="0 0 24 24" aria-hidden="true"
                ><path
                  fill="currentColor"
                  d="M12 2.4c-5.5 0-9.96 3.76-9.96 8.4 0 2.5 1.3 4.75 3.4 6.28V22l4.08-2.04c.8.16 1.63.24 2.48.24 5.5 0 9.96-3.76 9.96-8.4S17.5 2.4 12 2.4Z"
                /><path
                  fill="#fff"
                  d="M8.2 7.9h7.6v1.55l-4.85 4.75h4.85v1.55H8.2v-1.55l4.85-4.75H8.2V7.9Z"
                /></svg
              ></button
            ><button
              class="author-link linkedin"
              title="LinkedIn: Alex Linh"
              aria-label={tr.linkedinContactAria}
              onclick={() => openAuthor("linkedin")}
              ><svg viewBox="0 0 24 24" aria-hidden="true"
                ><path
                  fill="currentColor"
                  d="M6.94 8.5H3.56V20h3.38V8.5ZM5.25 3.5a1.96 1.96 0 1 0 0 3.92 1.96 1.96 0 0 0 0-3.92ZM20.44 20h-3.38v-5.93c0-1.42-.03-3.25-1.98-3.25-1.98 0-2.28 1.55-2.28 3.15V20H9.42V8.5h3.24v1.57h.05c.45-.86 1.55-1.77 3.19-1.77 3.41 0 4.04 2.24 4.04 5.16V20Z"
                /></svg
              ></button
            ><button
              class="author-link website"
              title="Website: alexlinh.io.vn"
              aria-label={tr.websiteContactAria}
              onclick={() => openAuthor("website")}><Globe size={16} /></button
            >
          </div>
        </div>
      </div>
      <div class="sidebar-bottom">
        <button
          class:chosen={page === "settings"}
          onclick={() => {
            if (testing) {
              error = tr.errorSettingsTesting;
              return;
            }
            if (editing) {
              error = tr.errorSettingsEditing;
              return;
            }
            page = "settings";
            taskMode = "meeting";
          }}><Settings size={18} /> {tr.settings}</button
        ><span><span class="local-dot"></span> {tr.dataOnDevice}</span>
      </div>
    </aside>

    <main>
      <header class="topbar">
        <div class="breadcrumb">
          {page === "settings"
            ? tr.settings
            : page === "caption"
              ? tr.captionPage
              : meeting
                ? tr.yourMeeting
                : tr.meetingSpace}
        </div>
        <div class="local-label">
          <CheckCheck size={16} />
          {captionRunning && captionProvider && captionProvider !== "local"
            ? fill(tr.processingCloud, { provider: captionProvider === "groq" ? "Groq" : captionProvider === "soniox" ? "Soniox" : captionProvider === "openai" ? "OpenAI" : "Gemini" })
            : captionRunning && captionProvider === "local"
              ? tr.processingLocal
              : captionRunning ? tr.captionActive
              : meeting?.provider === "soniox"
            ? tr.sonioxExternal
            : meeting?.provider === "mixed"
              ? tr.sonioxMixed
              : tr.processingLocal}
        </div>
      </header>
      {#if !desktop}<div class="notice warning">
          <AlertCircle size={18} /><span>{tr.previewNotice}</span>
        </div>{/if}
      {#if error}<div class="notice error" role="alert">
          <AlertCircle size={19} /><span>{error}</span><button
            class="icon-button"
            aria-label={tr.closeNotice}
            onclick={() => {
              error = "";
            }}><X size={16} /></button
          >
        </div>{/if}
      {#if active && (selected !== active.id || page !== "meeting")}<div
          class="notice active-session"
          role="status"
        >
          <Circle size={16} /><span
            >{active.title} · {states[active.status] ?? active.status} · {clock(
              active.duration,
            )}</span
          >
          <button class="text-button" onclick={() => active && choose(active)}
            >{tr.backToActive}</button
          >
        </div>{/if}
      {#if updateInfo}
        <div class="notice update-banner" role="status" aria-live="polite">
          <RefreshCw size={18} />
          <span>
            {#if updatePhase === "downloading"}
              {fill(tr.updateDownloading, { percent: updatePercent })}
            {:else if updatePhase === "installing"}
              {tr.updateInstalling}
            {:else}
              {fill(tr.updateAvailable, { version: updateInfo.version })}
            {/if}
          </span>
          {#if updatePhase === "available"}
            <button class="button primary small" onclick={installUpdate}>
              {tr.updateNow}
            </button>
            <button class="text-button" onclick={dismissUpdate}
              >{tr.updateLater}</button
            >
          {/if}
        </div>
      {/if}

      {#if page === "settings"}
        <section class="settings-page">
          <h1>{tr.settingsHeading}</h1>
          <p class="subtitle">
            {fill(tr.settingsSubtitle, { version: appVersion })}
          </p>
          <div class="settings-section">
            <h2>{tr.interfaceLanguage}</h2>
            <p>{tr.interfaceLanguageHint}</p>
            <label
              >{tr.interfaceLanguage}<select
                aria-label={tr.interfaceLanguage}
                value={uiLocale ?? "vi"}
                disabled={busy}
                onchange={(event) =>
                  setUiLocale(event.currentTarget.value as UiLocale)}
              >
                {#each locales as value}
                  <option {value}
                    >{localeNames[value]} — {localeEnglishNames[value]}</option
                  >
                {/each}
              </select></label
            >
          </div>
          <div class="settings-section">
            <h2>{tr.updateSection}</h2>
            <p>{fill(tr.updateDescription, { version: appVersion })}</p>
            <button
              class="button secondary"
              disabled={updateChecking ||
                updatePhase === "downloading" ||
                updatePhase === "installing"}
              onclick={() => void checkForUpdate(true)}
            >
              <RefreshCw size={17} />
              {updateChecking ? tr.updateChecking : tr.checkUpdates}
            </button>
          </div>
          <div class="settings-section ai-settings">
            <div class="soniox-heading">
              <div>
                <h2><Captions size={18} /> {tr.aiProfilesTitle}</h2>
                <p>{tr.aiProfilesDesc}</p>
              </div>
              <button
                class="button secondary"
                disabled={!!editing}
                onclick={openAddProfile}
                ><Plus size={15} /> {tr.aiProfileAdd}</button
              >
            </div>
            {#if aiProfiles.length === 0}<p class="quality-hint">
                {tr.aiProfilesEmpty}
              </p>{:else}
              <div class="ai-profiles-list">
                {#each aiProfiles as p}
                  <div class="ai-profile-row">
                    <div class="ai-profile-name">
                      <strong>{p.name}</strong>
                      <span
                        >{p.provider} · {tr.aiProfileKey} …{p.key_tail ||
                          "—"}</span
                      >
                    </div>
                    <button
                      class="text-button"
                      disabled={!!editing}
                      onclick={() => openEditProfile(p)}
                      >{tr.aiProfileEdit}</button
                    ><button
                      class="text-button danger"
                      disabled={!!editing}
                      onclick={() => deleteProfile(p)}
                      >{tr.aiProfileDelete}</button
                    >
                  </div>
                {/each}
              </div>
            {/if}
          </div>
          <div class="settings-section ai-settings">
            <div class="soniox-heading">
              <div>
                <h2><Sparkles size={18} /> {tr.taskAssignTitle}</h2>
                <p>{tr.taskAssignHint}</p>
              </div>
            </div>
            <div
              class="task-mode-tabs"
              role="tablist"
              aria-label={tr.taskAssignTitle}
            >
              <button
                type="button"
                role="tab"
                class:active={taskMode === "meeting"}
                onclick={() => (taskMode = "meeting")}
                >{tr.taskModeMeeting}</button
              >
              <button
                type="button"
                role="tab"
                class:active={taskMode === "captionsonly"}
                onclick={() => (taskMode = "captionsonly")}
                >{tr.taskModeCaptionsOnly}</button
              >
            </div>
            {#if taskMode === "captionsonly"}<label class="checkbox" style="margin: 20px 0"><input type="checkbox" aria-label="Dịch sang cho phụ đề dùng riêng" checked={taskCaptionTranslate} onchange={e=>setAssignment("caption",{...taskAssigns[taskMode]?.caption,profileId:taskAssigns[taskMode]?.caption?.profileId??"",model:taskAssigns[taskMode]?.caption?.model??"",targetLang:taskAssigns[taskMode]?.caption?.targetLang??"vi",translate:e.currentTarget.checked})} /> Dịch sang cho phụ đề dùng riêng</label>{/if}
            <div class="task-assign-grid">
              <label class="ai-field"
                >{tr.taskTranscription}<select
                  value={taskAssigns[taskMode]?.transcription?.profileId ?? ""}
                  disabled={busy}
                  onchange={(e) => {
                    const value = e.currentTarget.value;
                    if (!value) setAssignment("transcription", undefined);
                    else
                      setAssignment("transcription", {
                        ...taskAssigns[taskMode]?.transcription,
                        profileId: value,
                        model: value === "__local__" ? effectiveModel : "stt-rt-v5",
                      });
                  }}
                  ><option value="">{tr.taskNone}</option><option
                    value="__local__">{tr.providerLocal}</option
                  >{#each sonioxProfiles(aiProfiles) as p}<option value={p.id}
                      >{p.name}</option
                    >{/each}</select
                ></label
              >
              <label class="ai-field"
                >{tr.taskCaption}<select
                  value={taskAssigns[taskMode]?.caption?.profileId ?? ""}
                  disabled={busy}
                  onchange={(e) => {
                    const value = e.currentTarget.value;
                    if (!value) setAssignment("caption", undefined);
                    else
                      setAssignment("caption", {
                        ...taskAssigns[taskMode]?.caption,
                        profileId: value,
                        model: captionModels(value, aiProfiles, taskCaptionTranslate)[0] ?? "",
                        translate: taskCaptionTranslate,
                        targetLang:
                          taskAssigns[taskMode]?.caption?.targetLang ?? "vi",
                      });
                  }}
                  ><option value="">{tr.taskNone}</option
                  >{#if taskMode === "captionsonly" && !taskCaptionTranslate}<option value="__local__">Local trên máy · không cần API</option>{/if}{#each captionableProfiles(aiProfiles).filter(p=>captionModels(p.id,aiProfiles,taskCaptionTranslate).length>0) as p}<option
                      value={p.id}>{p.name} · {p.provider}</option
                    >{/each}</select
                ></label
              >
              <label class="ai-field"
                >{tr.captionTargetLang}<select
                  value={taskAssigns[taskMode]?.caption?.targetLang ?? "vi"}
                  disabled={busy}
                  onchange={(e) =>
                    setAssignment("caption", {
                      ...taskAssigns[taskMode]?.caption,
                      profileId:
                        taskAssigns[taskMode]?.caption?.profileId ?? "",
                      model: taskAssigns[taskMode]?.caption?.model ?? "",
                      targetLang: e.currentTarget.value,
                      translate: taskCaptionTranslate,
                    })}
                  ><option value="vi">{tr.langVi}</option><option value="en"
                    >{tr.langEn}</option
                  ><option value="ko">{tr.langKo}</option><option value="zh"
                    >{tr.langZh}</option
                  ><option value="ja">日本語</option></select
                ></label
              >
              <label class="ai-field"
                >{tr.taskSmooth}<select
                  value={taskAssigns[taskMode]?.smooth?.profileId ?? ""}
                  disabled={busy}
                  onchange={(e) => {
                    const value = e.currentTarget.value;
                    if (!value) setAssignment("smooth", undefined);
                    else
                      setAssignment("smooth", {
                        ...taskAssigns[taskMode]?.smooth,
                        profileId: value,
                        model: textModels(value, aiProfiles)[0] ?? "",
                      });
                  }}
                  ><option value="">{tr.taskNone}</option
                  >{#each chatProfiles(aiProfiles) as p}<option value={p.id}
                      >{p.name} · {p.provider}</option
                    >{/each}</select
                ></label
              >
              <label class="ai-field"
                >{tr.taskSummary}<select
                  value={taskAssigns[taskMode]?.summary?.profileId ?? ""}
                  disabled={busy}
                  onchange={(e) => {
                    const value = e.currentTarget.value;
                    if (!value) setAssignment("summary", undefined);
                    else
                      setAssignment("summary", {
                        ...taskAssigns[taskMode]?.summary,
                        profileId: value,
                        model: textModels(value, aiProfiles)[0] ?? "",
                      });
                  }}
                  ><option value="">{tr.taskNone}</option
                  >{#each chatProfiles(aiProfiles) as p}<option value={p.id}
                      >{p.name} · {p.provider}</option
                    >{/each}</select
                ></label
              >
            </div>
            <div class="task-assign-grid task-model-grid">
              {#if taskAssigns[taskMode]?.caption?.profileId}
                <label class="ai-field"
                  >{tr.captionModel}<select
                    aria-label={tr.captionModel}
                    value={taskAssigns[taskMode]?.caption?.model ||
                      captionModels(
                        taskAssigns[taskMode]?.caption?.profileId ?? "",
                        aiProfiles,
                        taskCaptionTranslate,
                      )[0]}
                    onchange={(e) =>
                      setAssignment("caption", {
                        ...taskAssigns[taskMode]?.caption!,
                        model: e.currentTarget.value,
                      })}
                  >
                    {#each captionModels(taskAssigns[taskMode]?.caption?.profileId ?? "", aiProfiles, taskCaptionTranslate) as m}<option
                        value={m}>{modelLabel(taskAssigns[taskMode]?.caption?.profileId,m)}</option
                      >{/each}
                  </select></label
                >
              {/if}
              {#each ["smooth", "summary"] as task}
                {@const key = task as "smooth" | "summary"}
                {#if taskAssigns[taskMode]?.[key]?.profileId}
                  <label class="ai-field"
                    >{key === "smooth" ? tr.taskSmooth : tr.taskSummary} · {tr.aiModel}
                    {#if textModels(taskAssigns[taskMode]?.[key]?.profileId??"",aiProfiles).length}
                      <select aria-label={`${key === "smooth" ? tr.taskSmooth : tr.taskSummary} · Model`} value={taskAssigns[taskMode]?.[key]?.model??""} onchange={e=>setAssignment(key,{...taskAssigns[taskMode]?.[key]!,model:e.currentTarget.value})}>
                        <option value="">Chọn model…</option>
                        {#if taskAssigns[taskMode]?.[key]?.model && !textModels(taskAssigns[taskMode]?.[key]?.profileId??"",aiProfiles).includes(taskAssigns[taskMode]?.[key]?.model??"")}<option value={taskAssigns[taskMode]?.[key]?.model}>{modelLabel(taskAssigns[taskMode]?.[key]?.profileId,taskAssigns[taskMode]?.[key]?.model??"")} · đã lưu</option>{/if}
                        {#each textModels(taskAssigns[taskMode]?.[key]?.profileId??"",aiProfiles) as m}<option value={m}>{modelLabel(taskAssigns[taskMode]?.[key]?.profileId,m)}{m === "qwen/qwen3.8-27b" ? " · Preview" : ""}</option>{/each}
                      </select>
                    {:else}<input aria-label={`${key === "smooth" ? tr.taskSmooth : tr.taskSummary} · Model ID`} value={taskAssigns[taskMode]?.[key]?.model??""} onchange={e=>setAssignment(key,{...taskAssigns[taskMode]?.[key]!,model:e.currentTarget.value})} />{/if}
                  </label>
                {/if}
              {/each}
            </div>
            <div class="task-fallbacks">
              <h3>Dự phòng khi model lỗi hoặc hết hạn mức</h3>
              <p>Chỉ chuyển một lần sang model bạn chọn; không tự tìm nhà cung cấp khác. Cấu hình dự phòng có thể gửi dữ liệu sang nhà cung cấp khác và áp dụng hạn mức/chi phí của tài khoản đó. Lựa chọn được chốt khi bắt đầu phiên.</p>
              <div class="task-assign-grid">
                {#each (taskMode === "meeting" ? ["transcription","caption","smooth","summary"] as const : ["caption"] as const) as task (task)}
                  <label class="ai-field">Dự phòng · {taskLabel(task)}
                    <select aria-label={`Dự phòng · ${taskLabel(task)}`} value={taskAssigns[taskMode]?.[task]?.fallback ? JSON.stringify(taskAssigns[taskMode]?.[task]?.fallback) : ""} disabled={busy || !taskAssigns[taskMode]?.[task]?.profileId || taskAssigns[taskMode]?.[task]?.profileId === "__local__"} onchange={e=>chooseFallback(task,e.currentTarget.value)}>
                      <option value="">Không dùng dự phòng</option>
                      {#each fallbackChoices(task,aiProfiles,taskAssigns[taskMode],taskMode,taskCaptionTranslate) as choice}<option value={JSON.stringify(choice)}>{modelLabel(choice.profileId,choice.model)}</option>{/each}
                    </select>
                  </label>
                {/each}
              </div>
              {#if taskMode === "meeting"}<small>Chép lời: hỗ trợ Soniox chính → cấu hình Soniox dự phòng. Dự phòng local vẫn bật riêng khi tạo cuộc họp. Phụ đề dùng chung Soniox đi theo dự phòng chép lời.</small>{/if}
            </div>
            <p class="fine-print">{tr.taskAssignNote}</p>
          </div>
          <div class="settings-section" id="caption-settings">
            <h2><Captions size={18} /> Cài đặt phụ đề</h2>
            <p>Áp dụng cho cả phụ đề cuộc họp và phụ đề dùng riêng. Thay đổi hiển thị có hiệu lực ngay, không khởi động lại phiên.</p>
            <CaptionControls />
          </div>
          <div class="settings-section">
            <h2>{tr.modelTitle}</h2>
            <p>{tr.modelDesc}</p>
            <p>Nemotron là model chép lời đa ngôn ngữ chạy trên máy, không dùng để dịch hay tóm tắt. Chỉ cần cho chế độ local đa ngôn ngữ hoặc dự phòng đã bật; Soniox hoạt động không cần tải Nemotron.</p>
            {#each data.models.filter((m) => m.id.startsWith("moonshine-") || m.id === multilingualModel) as m}<div
                class="model-row"
              >
                <div class="model-symbol"><FileText size={21} /></div>
                <div>
                  <strong>{m.name}</strong>
                  <p>
                    {Math.round(m.bytes / 1000000)} MB · {m.id ===
                    multilingualModel
                      ? tr.modelLangsMulti
                      : m.id === "moonshine-vi" ? tr.langVi : tr.langEn} · {tr.onDevice}
                  </p>
                </div>
                {#if m.installed}<span class="installed"
                    ><Check size={16} /> {m.id.startsWith("moonshine-") ? tr.modelBundled : tr.installed}</span
                  >{:else if m.id.startsWith("moonshine-")}<span class="test-error" role="alert">{tr.modelMissingBundled}</span>{:else}<button
                    class="button secondary"
                    disabled={!!progress || busy}
                    onclick={() => {
                      if (m.id === multilingualModel) language = "multi";
                      void download(m.id);
                    }}><Download size={16} /> {tr.downloadModel}</button
                  >{/if}
              </div>{/each}
            {#if progress}<div class="download-progress">
                <div>
                  <span>{tr.downloadingModel}</span><span
                    >{Math.round(
                      (progress.bytes / progress.total) * 100,
                    )}%</span
                  >
                </div>
                <progress max={progress.total} value={progress.bytes}
                ></progress><small
                  >{Math.round(progress.bytes / 1000000)} / {Math.round(
                    progress.total / 1000000,
                  )} MB. {tr.downloadError}</small
                >
              </div>{/if}
          </div>
          <div class="settings-section">
            <h2>{tr.dataTitle}</h2>
            <p>{tr.dataDesc}</p>
            <div class="folder-path">
              {data.data_dir || tr.dataFolderFallback}
            </div>
            <button
              class="button secondary"
              disabled={!desktop}
              onclick={() =>
                action(async () => {
                  await invoke("open_folder");
                })}><FolderOpen size={17} /> {tr.openDataFolder}</button
            >
            <p class="fine-print">
              {fill(tr.finePrint, { version: appVersion })}
            </p>
          </div>
        </section>
      {:else if page === "caption"}
        <section class="caption-page">
          <h1><Captions size={20} /> {tr.captionPage}</h1>
          <p class="subtitle">
            {tr.captionPageHint}
          </p>
          <div class="settings-section caption-setup">
            <div class="caption-intent">
              <div class="caption-intent-choice">
                <label class="checkbox"><input type="checkbox" aria-label="Dịch sang" bind:checked={captionTranslate} disabled={busy || captionRunning || captionTesting} /> Dịch sang</label>
                {#if captionTranslate}<label class="caption-intent-target">{tr.captionTargetLang}<select aria-label={tr.captionTargetLang} bind:value={captionTargetLang} disabled={busy || captionRunning || captionTesting}><option value="vi">{tr.langVi}</option><option value="en">{tr.langEn}</option><option value="ko">{tr.langKo}</option><option value="zh">{tr.langZh}</option><option value="ja">日本語</option></select></label>{/if}
              </div>
              <p>{captionTranslate ? "Hiển thị bản dịch và tùy chọn song ngữ. Âm thanh được gửi lên cấu hình đã chọn." : "Chỉ chép và hiện lời gốc đang nghe. Chọn Local trên máy để dùng Moonshine Việt/Anh có sẵn hoặc Nemotron đa ngôn ngữ không cần API, hoặc dùng Soniox/Groq."}</p>
            </div>
            <div class="caption-mode-row">
              <label>{tr.captionDevice}<select aria-label={tr.captionDevice} bind:value={captionDevice} disabled={busy || captionRunning || captionTesting}>
                <option value="">{tr.captionAutoDevice}</option>{#each data.devices.filter(d=>d.kind === "system") as d}<option value={d.id}>{d.name}{d.default ? tr.defaultDevice : ""}</option>{/each}
              </select></label>
              <label>Ngôn ngữ gốc<select aria-label="Ngôn ngữ gốc" bind:value={captionSourceLang} disabled={busy || captionRunning || captionTesting}><option value="auto">Tự nhận diện</option><option value="vi">Tiếng Việt</option><option value="en">English</option><option value="ko">한국어</option><option value="zh">中文</option><option value="ja">日本語</option></select></label>
            </div>
            <div class="caption-mode-row">
              <label
                >{tr.captionProfile}:<select
                  aria-label={tr.captionProfile}
                  value={captionProfileId}
                  disabled={busy || captionRunning || captionTesting}
                  onchange={(e) => {
                    captionProfileId = e.currentTarget.value;
                    captionModel =
                      captionModels(captionProfileId, aiProfiles, captionTranslate,captionSourceLang)[0] ?? "";
                  }}
                >
                  <option value="">{tr.captionChooseProfile}</option>
                  {#if !captionTranslate}<option value="__local__">Local trên máy · không cần API</option>{/if}
                  {#each captionableProfiles(aiProfiles).filter(p=>captionModels(p.id,aiProfiles,captionTranslate).length>0) as p}<option
                      value={p.id}>{p.name} · {p.provider}</option
                    >{/each}</select
                ></label
              >
              <label
                >{tr.captionModel}<select
                  bind:value={captionModel}
                  aria-label={tr.captionModel}
                  disabled={busy || captionRunning || captionTesting || !captionProfileId}
                  ><option value="" disabled>{tr.taskNone}</option>{#each captionProfileId === "__local__" && !captionTranslate ? ["moonshine-vi","moonshine-en",multilingualModel] : captionModels(captionProfileId, aiProfiles, captionTranslate,captionSourceLang) as m}<option
                      value={m} disabled={captionProfileId === "__local__" && (!data.models.some(model=>model.id===m&&model.installed) || !captionModels(captionProfileId,aiProfiles,captionTranslate,captionSourceLang).includes(m))}>{modelLabel(captionProfileId,m)}{captionProfileId === "__local__" ? ` · ${data.models.some(model=>model.id===m&&model.installed) ? "Đã tải" : "Chưa tải"}${!captionModels(captionProfileId,aiProfiles,captionTranslate,captionSourceLang).includes(m) ? " · Không phù hợp ngôn ngữ gốc" : ""}` : ""}</option
                    >{/each}</select
                ></label
              >
            </div>
            <SpeechQualityControl scope="Phụ đề" locale={uiLocale??"vi"} value={captionSpeechQuality} provider={captionQualityProvider} model={captionModel} disabled={busy || captionRunning || captionTesting} onchange={q=>chooseProcessingLevel(captionLevelKey,q)} />
            {#if captionProfileId && !captionModels(captionProfileId,aiProfiles,captionTranslate,captionSourceLang).includes(captionModel)}<p class="test-error">Model đã chọn không phù hợp chế độ hoặc ngôn ngữ gốc. Moonshine có model riêng cho Việt/Anh; Nemotron hỗ trợ đa ngôn ngữ. Hãy chọn lại model.</p>{/if}
            {#if captionProfileId === "__local__"}<p class="quality-hint">Chép trên máy, không cần API và không tự chuyển lên cloud. {captionModel === multilingualModel ? "Nemotron nhận audio liên tục, có chữ tạm theo lời nói; hỗ trợ Việt, Anh, Hàn và Trung. Tốc độ phụ thuộc CPU." : "Moonshine có sẵn trong Innote, chép streaming theo ngôn ngữ Việt/Anh của model đã chọn; không tự nhận diện đa ngôn ngữ. Tốc độ phụ thuộc CPU."} {captionLocalReady ? captionModel.startsWith("moonshine-") ? tr.modelBundled : "Model đã có trên máy." : captionModel.startsWith("moonshine-") ? tr.modelMissingBundled : "Model chưa tải: mở Cài đặt → Model chép lời để tải trước."}</p>{/if}
            {#if profileOf(captionProfileId,aiProfiles)?.provider === "groq"}<p class="quality-hint">Groq Whisper: độ dài đoạn và ngữ cảnh thay đổi theo Mức xử lý đã chọn; phần audio chồng lấn cũng dùng hạn mức API. Có thể trễ thêm do API, không có chữ tạm từng từ. Khi lỗi chỉ chuyển sang dự phòng bạn đã chọn, nếu có. Hiện hỗ trợ phụ đề dùng riêng.</p>{/if}
            <div class="caption-actions">
              {#if captionProfileId !== "__local__"}
              <button
                class="button secondary"
                disabled={busy ||
                  captionTesting ||
                  captionRunning ||
                  !captionProfileId ||
                  !captionModel}
                onclick={() => testCaptionModel()}
                >{captionTesting ? tr.aiTesting : tr.captionTestModel}</button
              >
              {/if}
              {#if captionRunning}<button
                  class="button stop"
                  disabled={busy}
                  onclick={() => stopCaptionSession()}
                  ><Square size={15} fill="currentColor" />
                  {tr.captionStop}</button
                ><span class="caption-live-badge"
                  ><span class="caption-dot-anim"></span>
                  {captionSending ? tr.captionSending : tr.captionActive}</span
                >{:else}<button
                  class="button primary"
                  disabled={busy ||
                    captionTesting ||
                    !captionProfileId ||
                    !captionModel ||
                    !captionModels(captionProfileId,aiProfiles,captionTranslate,captionSourceLang).includes(captionModel) ||
                    !captionLocalReady ||
                    !desktop}
                  onclick={() => startCaptionSession("standalone")}
                  ><Captions size={16} /> {tr.captionStart}</button
                >{/if}
              <CaptionWindowToggle />
            </div>
            {#if captionFallbackMessage}<div class="notice warning" role="status"><AlertCircle size={16}/><span>{captionFallbackMessage}</span></div>{/if}
            {#if captionStatusMessage && !captionError}<p
                class="quality-hint caption-status-line"
              >
                {captionStatusMessage}
              </p>{/if}
            {#if captionError}<div class="notice warning">
                <AlertCircle size={16} /><span>{captionError}</span>
              </div>{/if}
            {#if captionText}<div class="caption-preview">
                {#if captionOriginal && captionOriginal !== captionText}<div class="caption-preview-original">
                    {captionOriginal}
                  </div>{/if}
                <div class="caption-preview-text">{captionText}</div>
              </div>{/if}
          </div>
          <button class="text-button caption-settings-link" onclick={()=>{page="settings";void tick().then(()=>document.getElementById("caption-settings")?.scrollIntoView({block:"start"}));}}><Settings size={16} /> Cài đặt chữ và hộp phụ đề</button>
          <div class="settings-section">
            <h2>{tr.captionNoMicNoteTitle}</h2>
            <p>{tr.captionNoMicNote}</p>
          </div>
        </section>
      {:else}
        <section class="meeting-page" class:preparing={!meeting}>
          <div class="meeting-header">
            <div class="title-block">
              <input
                class="meeting-title"
                aria-label={tr.meetingTitleAria}
                placeholder={tr.meetingPlaceholder}
                bind:value={title}
                onblur={() => {
                  if (meeting && !title.trim()) {
                    title = meeting.title;
                    return;
                  }
                  if (meeting && title !== meeting.title)
                    void update({ title: title.trim() }).catch((e) => {
                      error = String(e);
                    });
                }}
              />
              <div class="meeting-meta">
                {#if meeting}<span
                    class:recording={meeting.status === "recording"}
                    class="status"
                    ><span></span>{states[meeting.status] ??
                      meeting.status}</span
                  ><span class="time">{clock(meeting.duration)}</span
                  >{:else}<span>{readyMessage}</span>{/if}
              </div>
            </div>
            <div class="record-controls">
              {#if active && selected === active.id && recording}<button
                  class="button secondary"
                  class:caption-on={captionRunning}
                  disabled={busy}
                  title={tr.captionToggleTitle}
                  onclick={() => toggleMeetingCaption()}
                  ><Captions size={17} />
                  {captionRunning ? tr.captionOn : tr.captionOff}</button
                ><button
                  class="button secondary"
                  disabled={busy}
                  onclick={togglePause}
                  >{#if active.status === "paused"}<Play size={17} />
                    {tr.continue}{:else}<Pause size={17} />
                    {tr.pause}{/if}</button
                ><button class="button stop" disabled={busy} onclick={finish}
                  ><Square size={15} fill="currentColor" /> {tr.stop}</button
                >
              {:else if active && selected === active.id && active.status === "processing"}<span
                  class="processing-label"
                  ><RefreshCw size={15} /> {tr.stateProcessing}…</span
                >
              {:else if !meeting}<button
                  class="button primary"
                  disabled={busy ||
                    !!data.active_id ||
                    !desktop ||
                    (!system && !microphone) ||
                    systemMissing ||
                    microphoneMissing ||
                    (transcribe && meetingCaptions && meetingTranslate && (provider !== "soniox" || !transcriptionProfileId)) ||
                    (transcribe && (provider === "local" || localFallback) && !currentModel?.installed)}
                  onclick={start}
                  ><Circle size={15} fill="currentColor" />{testing
                    ? tr.startChecking
                    : busy
                      ? tr.startPreparing
                      : tr.start}</button
                >
              {:else}<button
                  class="icon-button"
                  title={tr.deleteMeeting}
                  aria-label={tr.deleteMeeting}
                  disabled={busy || selected === data.active_id}
                  onclick={() => {
                    deleteTarget = meeting;
                    deleteOpen = true;
                  }}><Trash2 size={18} /></button
                >{/if}
            </div>
          </div>

          {#if active && selected === active.id && recording}<div class="meeting-caption-controls">
            <CaptionWindowToggle />
            <label class="checkbox"><input type="checkbox" aria-label="Dịch sang khi bật phụ đề" bind:checked={meetingTranslate} disabled={captionRunning || busy} /> Dịch sang</label>
            {#if meetingTranslate}<label>Ngôn ngữ dịch đích<select aria-label="Ngôn ngữ dịch đích cuộc họp" bind:value={meetingTargetLang} disabled={captionRunning || busy}><option value="vi">Tiếng Việt</option><option value="en">English</option><option value="ko">한국어</option><option value="zh">中文</option><option value="ja">日本語</option></select></label>{/if}
            <button class="text-button" onclick={()=>{page="settings";void tick().then(()=>document.getElementById("caption-settings")?.scrollIntoView({block:"start"}));}}><Settings size={15} /> Cài đặt phụ đề</button>
          </div>{/if}
          {#if !meeting}
            <div class="setup-panel">
              <div class="setup-heading">
                <h2>{tr.sourceTitle}</h2>
                <button
                  class="text-button"
                  disabled={busy || !desktop}
                  onclick={() =>
                    action(async () => {
                      await invoke("refresh_devices");
                    })}><RefreshCw size={14} /> {tr.refresh}</button
                >
              </div>
              <div class="source-row">
                <span class="source-icon"><Headphones size={21} /></span><label
                  ><strong>{tr.systemAudio}</strong><select
                    aria-label={tr.systemAudioAria}
                    bind:value={system}
                    onchange={clearTest}
                    disabled={busy}
                    ><option value="">{tr.noCapture}</option
                    >{#if systemMissing}<option value={system} disabled
                        >{tr.deviceMissing}</option
                      >{/if}{#each data.devices.filter((d) => d.kind === "system") as d}<option
                        value={d.id}
                        >{d.name}{d.default ? tr.defaultDevice : ""}</option
                      >{/each}</select
                  ></label
                >
                <div class="level" title={tr.systemLevel}>
                  <span style:width={`${busy ? systemLevel : 0}%`}></span>
                </div>
                <button
                  class="button secondary source-test"
                  disabled={busy ||
                    !!data.active_id ||
                    !desktop ||
                    !system ||
                    systemMissing}
                  onclick={() => test("system")}
                  ><Headphones size={15} />{testing && testKind === "system"
                    ? fill(tr.recordingTest, { seconds: testSeconds })
                    : tr.testSystem}</button
                >
              </div>
              <div class="source-row">
                <span class="source-icon"><Mic size={21} /></span><label
                  ><strong>{tr.mic}</strong><select
                    aria-label={tr.micAria}
                    bind:value={microphone}
                    onchange={clearTest}
                    disabled={busy}
                    ><option value="">{tr.noCapture}</option
                    >{#if microphoneMissing}<option value={microphone} disabled
                        >{tr.micMissing}</option
                      >{/if}{#each data.devices.filter((d) => d.kind === "microphone") as d}<option
                        value={d.id}
                        >{d.name}{d.default ? tr.defaultDevice : ""}</option
                      >{/each}</select
                  ></label
                >
                <div class="level" title={tr.micLevel}>
                  <span style:width={`${busy ? micLevel : 0}%`}></span>
                </div>
                <button
                  class="button secondary source-test"
                  disabled={busy ||
                    !!data.active_id ||
                    !desktop ||
                    !microphone ||
                    microphoneMissing}
                  onclick={() => test("microphone")}
                  ><Mic size={15} />{testing && testKind === "microphone"
                    ? fill(tr.recordingTest, { seconds: testSeconds })
                    : tr.testMic}</button
                >
              </div>
              <div class="source-footer">
                <span>{tr.sourceFooter}</span><button
                  class="text-button"
                  disabled={busy ||
                    !!data.active_id ||
                    !desktop ||
                    (!system && !microphone) ||
                    systemMissing ||
                    microphoneMissing}
                  onclick={() => test("both")}
                  ><Play size={14} />{testing
                    ? tr.testing
                    : tr.testSource}</button
                >
              </div>
              {#if testLabel}<p class="test-label" role="status">
                  {testLabel}{#if testing}
                    {testSeconds > 0
                      ? fill(tr.testCount, { seconds: testSeconds })
                      : tr.savingTest}{/if}
                </p>{/if}
              {#if testUrl}
                {#key testUrl}
                  <audio
                    bind:this={testAudio}
                    class="test-audio"
                    controls
                    preload="auto"
                    src={testUrl}
                    aria-label={tr.testAudio}
                  ></audio>
                {/key}
              {/if}
              {#if testFeedback}<p
                  class="test-feedback"
                  class:quiet={testQuiet}
                  role="status"
                >
                  {testFeedback}
                </p>{/if}
              {#if testWarnings.length}<details class="warnings test-warnings">
                  <summary
                    >{fill(tr.testWarnings, {
                      count: testWarnings.length,
                    })}</summary
                  >{#each groupedTestWarnings as warning}<p>
                      {warning.message}{warning.count > 1
                        ? fill(tr.times, { count: warning.count })
                        : ""}
                    </p>{/each}
                </details>{/if}
              {#if systemMissing || microphoneMissing}<p
                  class="source-warning"
                  role="status"
                >
                  {tr.sourceUnavailable}
                </p>{/if}
              <div class="preferences">
                <label
                  >{tr.transcriptionLanguage}<select
                    aria-label={tr.transcriptionLanguage}
                    bind:value={language}
                    disabled={busy}
                    ><option value="vi">{tr.langVi}</option><option value="en"
                      >{tr.langEn}</option
                    ><option value="ko">{tr.langKo}</option><option value="zh"
                      >{tr.langZh}</option
                    ><option value="multi">{tr.langMulti}</option></select
                  ></label
                >{#if multilingual && provider === "local"}<span class="fixed-model"
                    >{tr.fixedModel}</span
                  >{/if}
                <label class="checkbox"
                  ><input
                    type="checkbox"
                    bind:checked={transcribe}
                    disabled={busy}
                  />
                  {tr.transcribe}</label
                >
              </div>
              {#if transcribe}
                <div class="provider-choice">
                  <label for="transcription-provider">{tr.provider}</label>
                  <select
                    id="transcription-provider"
                    aria-label={tr.provider}
                    bind:value={provider}
                    disabled={busy}
                  >
                    <option value="local">{tr.providerLocal}</option>
                    <option value="soniox">{tr.providerSoniox}</option>
                  </select>
                  {#if provider === "local" || localFallback}
                    <label class="ai-field ai-field-full">{provider === "local" ? tr.localSpeechModel : tr.localFallbackModel}
                      <select aria-label={provider === "local" ? tr.localSpeechModel : tr.localFallbackModel} value={effectiveModel} disabled={busy} onchange={e=>chooseLocalMeetingModel(e.currentTarget.value)}>
                        {#each compatibleLocalModels(language) as id}<option value={id}>{modelLabel("__local__",id)} · {data.models.some(m=>m.id===id&&m.installed)?tr.installed:tr.downloadModel}</option>{/each}
                      </select>
                    </label>
                  {/if}
                  {#if provider === "soniox"}
                    <label class="ai-field ai-field-full"
                      >{tr.sonioxProfileLabel}<select
                        aria-label={tr.sonioxProfileLabel}
                        bind:value={transcriptionProfileId}
                        disabled={busy}
                        ><option value="">{tr.sonioxProfileDefault}</option
                        >{#each sonioxProfiles(aiProfiles) as p}<option
                            value={p.id}
                            >{p.name} · {p.provider} · …{p.key_tail}</option
                          >{/each}</select
                      ></label
                    >
                    <label class="ai-field">Model chép lời Soniox
                      <select aria-label="Model chép lời Soniox" disabled={busy}><option value="stt-rt-v5">{modelLabel(transcriptionProfileId,"stt-rt-v5")}</option></select>
                    </label>
                    <p class="quality-hint">Soniox lưu bản chép gốc. Khi bật phụ đề, dùng cùng luồng âm thanh; không chạy local song song.</p>
                    <label class="checkbox"><input type="checkbox" aria-label="Dự phòng local khi Soniox lỗi" bind:checked={localFallback} disabled={busy} /> Dự phòng local khi Soniox lỗi</label>
                    {#if localFallback}<p class="quality-hint">Chỉ chuyển local nếu Soniox lỗi. Khi đó tiếp tục chép lời, dịch tạm dừng. Moonshine Việt/Anh có sẵn; chỉ cần tải Nemotron nếu chọn đa ngôn ngữ/Hàn/Trung.</p>{/if}
                  {/if}
                </div>
              {/if}
              {#if transcribe}<SpeechQualityControl scope="Cuộc họp" locale={uiLocale??"vi"} value={meetingSpeechQuality} provider={provider} model={provider === "local" ? effectiveModel : "stt-rt-v5"} disabled={busy} onchange={q=>chooseProcessingLevel(meetingLevelKey,q)} />{/if}
              {#if transcribe}<div class="meeting-caption-setup">
                <label class="checkbox"><input type="checkbox" aria-label="Bật phụ đề khi bắt đầu cuộc họp" bind:checked={meetingCaptions} disabled={busy} /> Bật phụ đề khi bắt đầu cuộc họp</label>
                <p class="quality-hint">Lời gốc hiển thị trực tiếp để theo dõi hoặc luyện nghe. Bản chép cuộc họp vẫn được lưu khi tắt phụ đề.</p>
                {#if meetingCaptions}<div class="meeting-caption-options">
                  <label class="checkbox"><input type="checkbox" aria-label="Dịch sang trong cuộc họp" bind:checked={meetingTranslate} disabled={busy} /> Dịch sang</label>
                  {#if meetingTranslate}<label class="ai-field">Ngôn ngữ bản dịch<select aria-label="Ngôn ngữ bản dịch" bind:value={meetingTargetLang} disabled={busy}><option value="vi">Tiếng Việt</option><option value="en">English</option><option value="ko">한국어</option><option value="zh">中文</option><option value="ja">日本語</option></select></label>{/if}
                </div>
                <div class="meeting-shared-model">
                  <label class="ai-field">{meetingTranslate ? "Model dùng chung chép lời & dịch phụ đề" : "Model dùng chung chép lời & phụ đề"}
                    <select aria-label="Model dùng chung cho cuộc họp" value={meetingSharedChoice} disabled={busy} onchange={e=>chooseMeetingSharedModel(e.currentTarget.value)}>
                      <option value="" disabled>Chọn cấu hình/model dùng chung…</option>
                      {#if !meetingTranslate}{#each compatibleLocalModels(language) as id}<option value={JSON.stringify({profileId:"__local__",model:id})}>{modelLabel("__local__",id)}</option>{/each}{/if}
                      {#each sonioxProfiles(aiProfiles) as p}<option value={JSON.stringify({profileId:p.id,model:"stt-rt-v5"})}>{modelLabel(p.id,"stt-rt-v5")}</option>{/each}
                    </select>
                  </label>
                  {#if provider === "soniox" && transcriptionProfileId}<p class="quality-hint">Chép lời và phụ đề cùng dùng {modelLabel(transcriptionProfileId,"stt-rt-v5")}. Một kết nối Soniox giữ bản gốc{meetingTranslate ? " và bản dịch" : ""}; không chạy local song song. Audio được gửi lên cấu hình này.</p>
                  {:else if meetingTranslate}<p class="test-error" role="status">Hãy chọn Soniox ở đây; phần Cách chép lời sẽ đổi sang cùng cấu hình/model. Local không hỗ trợ dịch.</p>{/if}
                  {#if meetingTranslate}<small>Hiện chỉ Soniox hỗ trợ chép lời và dịch dùng chung trong cuộc họp. OpenAI/Gemini/Groq chưa dùng được cho luồng chung này. Quản lý API key trong Cài đặt.</small>{/if}
                </div>{/if}
              </div>{/if}
              {#if !transcribe}<p class="quality-hint">
                  {tr.transcribeOnly}
                </p>{/if}
              {#if multilingual && provider === "local"}<p class="quality-hint">
                  {language === "multi"
                    ? tr.multiHint
                    : language === "ko"
                      ? tr.koHint
                      : tr.zhHint}
                  {language === "multi" ? "" : tr.multiSwitchHint}
                </p>{/if}
              {#if !multilingual && transcribe && provider === "local"}<p class="quality-hint">
                  {tr.turboHint}
                </p>{/if}
              {#if transcribe && (provider === "local" || localFallback) && !currentModel?.installed && effectiveModel.startsWith("moonshine-")}<p class="test-error" role="alert">{tr.modelMissingBundled}</p>{/if}
              {#if transcribe && (provider === "local" || localFallback) && !currentModel?.installed && !effectiveModel.startsWith("moonshine-")}<div
                  class="model-download"
                >
                  <div>
                    <Download size={18} /><span
                      >{#if progress}{tr.downloading}
                        {Math.round(
                          (progress.bytes / progress.total) * 100,
                        )}%{:else}{tr.downloadOnce}{/if}</span
                    >
                  </div>
                  <button
                    class="button secondary small"
                    disabled={!!progress || !desktop || busy}
                    onclick={() => download()}
                    >{progress
                      ? tr.downloading
                      : fill(tr.downloadSize, {
                          size: Math.round(
                            (currentModel?.bytes ??
                              (multilingual ? 741548352 : 574041195)) / 1000000,
                          ),
                        })}</button
                  >{#if progress}<progress
                      max={progress.total}
                      value={progress.bytes}
                    ></progress>{/if}
                </div>{/if}
            </div>
          {/if}

          {#if meeting && selected === data.active_id && recording}<div
              class="recording-sources"
            >
              <span
                ><Headphones size={15} />
                {tr.systemAudio}
                <span class="level"
                  ><span style:width={`${systemLevel}%`}></span></span
                ></span
              ><span
                ><Mic size={15} />
                {tr.mic}
                <span class="level"
                  ><span style:width={`${micLevel}%`}></span></span
                ></span
              ><span>{tr.audioBeingSaved}</span>
            </div>{/if}
          {#if fallbackNotice}<div class="notice warning" role="status">
              <AlertCircle size={18} /><span>{fallbackNotice}</span>
            </div>{/if}
          {#if meeting?.error}<div class="notice warning">
              <AlertCircle size={18} /><span>{meeting.error}</span>
            </div>{/if}
          {#if meeting?.warnings.length}<details class="warnings">
              <summary
                >{fill(tr.sessionWarnings, {
                  count: meeting.warnings.length,
                })}</summary
              >{#each meeting.warnings as warning}<p>{warning}</p>{/each}
            </details>{/if}

          <div class="transcript-heading">
            <div>
              {#if meeting?.smoothed || meeting?.summary}
                <div
                  class="transcript-tabs"
                  role="tablist"
                  aria-label={tr.transcript}
                >
                  <button
                    type="button"
                    role="tab"
                    class="transcript-tab"
                    class:active={transcriptView === "original" &&
                      transcriptTab === "transcript"}
                    aria-selected={transcriptView === "original" &&
                      transcriptTab === "transcript"}
                    onclick={() => {
                      transcriptView = "original";
                      transcriptTab = "transcript";
                    }}><FileText size={13} /> {tr.summaryTabTranscript}</button
                  >
                  {#if meeting?.smoothed}<button
                      type="button"
                      role="tab"
                      class="transcript-tab"
                      class:active={transcriptView === "smoothed"}
                      aria-selected={transcriptView === "smoothed"}
                      onclick={() => {
                        transcriptView = "smoothed";
                        transcriptTab = "transcript";
                      }}><Sparkles size={13} /> {tr.smoothViewTab}</button
                    >{/if}
                  {#if meeting?.summary}<button
                      type="button"
                      role="tab"
                      class="transcript-tab"
                      class:active={transcriptTab === "summary"}
                      aria-selected={transcriptTab === "summary"}
                      onclick={() => {
                        transcriptTab = "summary";
                      }}
                      ><ClipboardList size={14} />
                      {tr.summaryTabSummary}{#if summaryStale}<span
                          class="tab-dot"
                          title={tr.summaryStale}
                        ></span>{/if}</button
                    >{/if}
                </div>
              {:else}
                <h2>{tr.transcript}</h2>
              {/if}
              {#if refinementPending}<span class="refining-note" role="status"
                  >{tr.exportWaitRefinement}</span
                >{/if}
              {#if meeting && ["recording", "paused", "processing"].includes(meeting.status)}<span
                  >{fill(tr.processedProgress, {
                    status:
                      (meeting?.speech || meeting?.model === multilingualModel)
                        ? tr.processedCommitted
                        : tr.processedCorrected,
                    processed: clock(meeting.processed),
                    duration: clock(meeting.duration),
                  })}</span
                >{/if}
            </div>
            {#if meeting?.translations?.length}
              <label class="transcript-language-view">Nội dung hiển thị
                <select aria-label="Nội dung hiển thị" bind:value={translationView} onchange={()=>{transcriptView="original";transcriptTab="transcript";}}>
                  <option value="original">Bản gốc</option><option value="translation">Bản dịch</option><option value="bilingual">Song ngữ</option>
                </select>
              </label>
              {#if translationLanguages.length>1}<label class="transcript-language-view">Ngôn ngữ bản dịch<select aria-label="Ngôn ngữ bản dịch đã lưu" bind:value={translationLanguage}>{#each translationLanguages as lang}<option value={lang}>{lang}</option>{/each}</select></label>{/if}
            {/if}
            <div class="transcript-tools">
              {#if canSmooth}<button
                  class="button secondary small"
                  title={meeting?.smoothed ? tr.smoothAgain : tr.smoothButton}
                  disabled={busy}
                  onclick={() => {
                    smoothProfileId =
                      taskAssigns.meeting?.smooth?.profileId ?? "";
                    useSessionProfile(smoothProfileId);
                    smoothModel =
                      taskAssigns.meeting?.smooth?.model ||
                      textModels(smoothProfileId, aiProfiles)[0] ||
                      "";
                    smoothOpen = true;
                  }}
                  ><Sparkles size={14} />
                  {meeting?.smoothed ? tr.smoothAgain : tr.smoothButton}</button
                >{/if}
              {#if canSmooth}<button
                  class="button secondary small"
                  title={meeting?.summary
                    ? tr.summaryRegenerate
                    : tr.summaryButton}
                  disabled={busy}
                  onclick={() => {
                    summaryProfileId =
                      taskAssigns.meeting?.summary?.profileId ?? "";
                    summaryModel =
                      taskAssigns.meeting?.summary?.model ||
                      textModels(summaryProfileId, aiProfiles)[0] ||
                      "";
                    summaryOpen = true;
                  }}
                  ><ClipboardList size={14} />
                  {meeting?.summary
                    ? tr.summaryRegenerate
                    : tr.summaryButton}</button
                >{/if}
              {#if meeting && (meeting.segments.length || showLive || audioUrl)}<div
                  class="export-controls compact"
                >
                  {#if meeting?.segments.length > 0}<button
                      class="button secondary small"
                      title={tr.copyTranscriptTitle}
                      disabled={busy}
                      onclick={() => copyTranscript()}
                      ><Copy size={14} /> {tr.copyTranscript}</button
                    >{/if}
                  {#if meeting.segments.length || showLive}<button
                      class="button secondary small"
                      title={refinementPending
                        ? tr.exportWaitRefinement
                        : tr.exportMdTitle}
                      disabled={busy || refinementPending}
                      onclick={() => exportText("md")}
                      ><Download size={14} /> {tr.exportMd}</button
                    ><button
                      class="button secondary small"
                      title={refinementPending
                        ? tr.exportWaitRefinement
                        : tr.exportTxtTitle}
                      disabled={busy || refinementPending}
                      onclick={() => exportText("txt")}
                      ><Download size={14} /> {tr.exportTxt}</button
                    ><button
                      class="button secondary small"
                      title={translationView !== "original"
                        ? "Bản dịch chưa có timestamp riêng. Hãy tải TXT/Markdown hoặc chọn bản gốc để tải SRT."
                        : refinementPending ? tr.exportWaitRefinement : tr.exportSrtTitle}
                      disabled={busy || refinementPending || translationView !== "original"}
                      onclick={() => exportText("srt")}
                      ><Download size={14} /> {tr.exportSrt}</button
                    >{/if}{#if audioUrl}<button
                      class="button secondary small"
                      title={tr.exportMp3Title}
                      disabled={busy}
                      onclick={() => exportAudio()}
                      ><Download size={14} /> {tr.exportMp3}</button
                    >{/if}
                </div>{/if}
            </div>
          </div>
          <div class="workspace">
            {#if transcriptTab !== "summary" && transcriptView === "smoothed" && meeting?.smoothed}
              <div
                class="smooth-view"
                role="tabpanel"
                aria-label={tr.smoothHeading}
              >
                <div class="smooth-view-bar">
                  <span class="smooth-meta">
                    <Sparkles size={14} />
                    {fill(tr.smoothResultBy, {
                      provider: meeting.smoothed.provider,
                      model: meeting.smoothed.model,
                    })}
                  </span>
                  <div class="smooth-bar-actions">
                    <button
                      class="text-button"
                      title={tr.smoothCopy}
                      onclick={() => copySmoothed()}
                      ><Check size={14} /> {tr.smoothCopy}</button
                    >
                    <button
                      class="text-button"
                      onclick={() => {
                        transcriptView = "original";
                      }}><FileText size={14} /> {tr.smoothBackOriginal}</button
                    >
                  </div>
                </div>
                <div class="smooth-result">
                  {#if smoothSegments.length}
                    {#each smoothSegments as seg (seg.id)}
                      <article class="segment smooth-segment">
                        <button
                          type="button"
                          class="timestamp"
                          title={tr.playFromHere}
                          disabled={!audioUrl}
                          onclick={() => seek(seg.start)}
                          >{clock(seg.start)}</button
                        >
                        <div class="segment-body">
                          <p>{seg.text}</p>
                        </div>
                      </article>
                    {/each}
                  {:else}
                    <p>{meeting.smoothed.text}</p>
                  {/if}
                </div>
              </div>
            {:else if transcriptTab === "summary" && meeting?.summary}
              <div
                class="summary-view"
                role="tabpanel"
                aria-label={tr.summaryTitle}
              >
                {#if summaryStale}
                  <div class="summary-stale-banner" role="status">
                    <AlertCircle size={16} />
                    <span>{tr.summaryStale}</span>
                    <button
                      class="button secondary small"
                      disabled={busy}
                      onclick={() => {
                        summaryOpen = true;
                      }}>{tr.summaryRegenerate}</button
                    >
                  </div>
                {/if}
                <div class="summary-meta-bar">
                  <span class="summary-meta-text">
                    {fill(tr.summaryMeta, {
                      source:
                        meeting.summary.source === "smoothed"
                          ? tr.summarySourceSmoothed
                          : tr.summarySourceOriginal,
                      provider: meeting.summary.provider,
                      model: meeting.summary.model,
                    })}
                  </span>
                  <div class="summary-toolbar">
                    <button
                      class="button secondary small"
                      title={tr.summaryCopy}
                      onclick={() => copySummary()}
                      ><Copy size={13} /> {tr.summaryCopy}</button
                    >
                    <button
                      class="button secondary small"
                      title={tr.summaryDownload}
                      onclick={() => exportSummary("md")}
                      ><Download size={13} /> {tr.summaryDownload}</button
                    >
                    <button
                      class="button secondary small"
                      title={tr.summaryRegenerate}
                      disabled={busy}
                      onclick={() => {
                        summaryOpen = true;
                      }}><RefreshCw size={13} /> {tr.summaryRegenerate}</button
                    >
                    <button
                      class="icon-button danger small"
                      title={tr.summaryDelete}
                      aria-label={tr.summaryDelete}
                      disabled={busy}
                      onclick={() => deleteSummary()}
                      ><Trash2 size={14} /></button
                    >
                  </div>
                </div>
                <div class="summary-card">
                  {#each parseSummarySections(meeting.summary.text) as section}
                    {#if section.title}
                      <h3 class="summary-section-title">{section.title}</h3>
                    {/if}
                    {#if section.text}
                      <p class="summary-paragraph">{section.text}</p>
                    {/if}
                    {#if section.items.length}
                      <ul class="summary-list">
                        {#each section.items as item}
                          <li>{item}</li>
                        {/each}
                      </ul>
                    {/if}
                  {/each}
                </div>
              </div>
            {:else}
              <div
                class="transcript"
                class:bilingual-transcript={translationView === "bilingual"}
                bind:this={transcript}
                onscroll={() => {
                  if (transcript)
                    autoScroll =
                      transcript.scrollHeight -
                        transcript.scrollTop -
                        transcript.clientHeight <
                      80;
                }}
              >
                <div class="original-track">
                {#if translationView === "bilingual"}<h3 class="track-heading">{continuousTranscript ? "Bản gốc" : "Bản gốc · bấm mốc để nghe từng đoạn"}</h3>{/if}
                {#if translationView === "translation"}<p class="track-note">Bản dịch được lưu riêng, không tự đổi khi sửa bản gốc và không có timestamp riêng. Chuyển sang Bản gốc để nghe lại đúng đoạn.</p>{/if}
                {#if continuousTranscript && (segments.length || (liveSegment && translationView !== "translation"))}
                  <article class="recording-transcript" aria-label={tr.liveAria}>
                    <p>{flowingFinalText}{#if liveSegment && translationView !== "translation"}<span class="recording-partial" data-live-caption>{flowingFinalText ? " " : ""}{liveSegment.text}</span>{/if}</p>
                  </article>
                {:else}
                {#each segments as s (s.id)}
                  <article class="segment">
                    {#if translationView !== "translation"}<button
                      class="timestamp"
                      title="Nghe đoạn này"
                      disabled={!audioUrl || s.end <= s.start}
                      onclick={() => seek(s.start, s.end)}>{clock(s.start)}</button
                    >{/if}
                    <div class="segment-body">
                      {#if editing === s.id}<textarea
                          aria-label={tr.editTranscriptAria}
                          bind:value={draft}></textarea>
                        <div class="edit-actions">
                          <button
                            class="text-button"
                            onclick={() => {
                              editing = "";
                            }}
                          >
                            {tr.editCancel}</button
                          ><button
                            class="button primary small"
                            onclick={() =>
                              action(async () => {
                                await update({ segmentId: s.id, text: draft });
                                editing = "";
                              })}>{tr.saveEdit}</button
                          >
                        </div>{:else}<p>{s.text}</p>
                        {#if s.edited}<small>{tr.edited}</small>{/if}{/if}
                    </div>
                    {#if translationView !== "translation"}<button
                      class="icon-button edit-button"
                      title={tr.editSegment}
                      aria-label={tr.editSegmentAria.replace(
                        "{time}",
                        clock(s.start),
                      )}
                      onclick={() => {
                        editing = s.id;
                        draft = s.text;
                      }}><Pencil size={14} /></button
                    >{/if}
                  </article>
                {:else}
                  {#if !showLive}
                    <div class="empty-transcript">
                      <span class="empty-mark"
                        ><Logo size={36} strokeWidth={2.8} /></span
                      >
                      <h3>{emptyHeading}</h3>
                      <p>{emptyDescription}</p>
                      {#if !meeting}<span class="empty-detail"
                          ><Mic size={14} /> {tr.emptyDetail}</span
                        >{/if}
                    </div>
                  {/if}
                {/each}
                {#if liveSegment && translationView !== "translation"}
                  <article class="segment pending" aria-label={tr.liveAria}>
                    <button class="timestamp" title={tr.playFromHere} disabled={!audioUrl} onclick={() => seek(liveSegment.start)}>{clock(liveSegment.start)}</button>
                    <div class="segment-body"><p data-live-caption>{liveSegment.text}</p></div>
                  </article>
                {/if}
                {/if}
                </div>
                {#if translationView === "bilingual"}<section class="translation-track" aria-label="Bản dịch đã lưu">
                  <h3 class="track-heading">Bản dịch · {visibleTranslationLanguage}</h3>
                  <p class="track-note">Hai luồng có thể chia/gộp câu khác nhau. Mốc nghe lại thuộc bản gốc. Bản dịch không tự đổi khi sửa bản gốc.</p>
                  {#if continuousTranscript}<p class="saved-translation">{savedTranslations.map(t=>t.text).join(" ")}</p>
                  {:else}{#each savedTranslations as t}<p class="saved-translation">{t.text}</p>{/each}{/if}
                </section>{/if}
              </div>
            {/if}
            {#if showNotes && meeting}<aside class="notes-panel">
                <h3>{tr.notesTitle}</h3>
                <p>{tr.notesHint}</p>
                <textarea
                  aria-label={tr.notesTitle}
                  placeholder={tr.notesPlaceholder}
                  bind:value={notesDraft}
                  onblur={() => {
                    if (notesDraft !== meeting?.notes)
                      void update({ notes: notesDraft }).catch((e) => {
                        error = String(e);
                      });
                  }}></textarea><small>{tr.notesAutosave}</small>
              </aside>{/if}
          </div>
          {#if meeting}<div class="transcript-footer-tools">
              <label class="inline-search"
                ><Search size={15} /><input
                  placeholder={tr.searchContent}
                  aria-label={tr.searchTranscript}
                  bind:value={searchText}
                /></label
              >
              <button
                class:active={showNotes}
                class="icon-button"
                title={tr.notesTitle}
                aria-label={tr.openNotes}
                onclick={() => {
                  showNotes = !showNotes;
                }}><NotebookPen size={18} /></button
              >
            </div>{/if}
          {#if !autoScroll && recording}<button
              class="jump-button"
              onclick={() => {
                autoScroll = true;
                transcript.scrollTop = transcript.scrollHeight;
              }}><ArrowDown size={14} /> {tr.jumpLatest}</button
            >{/if}
          {#if audioUrl}
            {#key audioUrl}
              <audio
                class="playback"
                controls
                preload="auto"
                bind:this={audio}
                src={audioUrl}
                ontimeupdate={checkSegmentPlayback}
                onplay={scheduleSegmentStop}
                onpause={clearSegmentTimer}
                onratechange={scheduleSegmentStop}
                onended={()=>{clearSegmentTimer();segmentPlaybackEnd=null;}}
                aria-label={tr.playbackAria}
              ></audio>
            {/key}
          {/if}
          <footer class="meeting-footer">
            <div class="save-state">
              <Check size={15} /><span
                >{meeting
                  ? ["recording", "paused"].includes(meeting.status)
                    ? meeting.status === "paused"
                      ? tr.savePaused
                      : tr.saveRecording
                    : tr.saveLocal
                  : tr.saveNotice}</span
              >
            </div>
            <div class="export-controls">
              {#if lastExport}<button
                  class="text-button"
                  title={lastExport}
                  onclick={() =>
                    action(async () => {
                      await invoke("open_exports_folder");
                    })}><FolderOpen size={15} /> {tr.openExportFolder}</button
                >{/if}
            </div>
          </footer>
        </section>
      {/if}
    </main>
  </div>
  {#if toast}<div class="toast" role="status">
      <Check size={17} />{toast}
    </div>{/if}
  {#if confOpen}
    <dialog
      class="profile-modal"
      bind:this={profileDialog}
      aria-label={confEditingId ? tr.aiProfileEdit : tr.aiProfileAdd}
      onclose={() => { confOpen = false; }}
      onkeydown={profileDialogKeydown}
      onclick={(e) => {
        if (e.target !== e.currentTarget) return;
        const bounds = e.currentTarget.getBoundingClientRect();
        if (e.clientX < bounds.left || e.clientX > bounds.right || e.clientY < bounds.top || e.clientY > bounds.bottom) closeProfileDialog();
      }}
    >
        <div class="profile-modal-head">
          <h3>{confEditingId ? tr.aiProfileEdit : tr.aiProfileAdd}</h3>
          <button
            class="icon-button"
            aria-label={tr.editCancel}
            onclick={() => {
              closeProfileDialog();
            }}><X size={17} /></button
          >
        </div>
        <fieldset class="ai-grid profile-fields" disabled={confTesting}>
          <label class="ai-field"
            >{tr.aiProfileName}<input
              bind:value={confName}
              aria-label={tr.aiProfileName}
              placeholder={fill(tr.aiProfileNameHint, {
                provider: confProvider,
              })}
              autocomplete="off"
              spellcheck="false"
            /></label
          >
          <label class="ai-field"
            >{tr.aiProvider}<select
              aria-label={tr.aiProvider}
              value={confProvider}
              onchange={(e) => chooseConfProvider(e.currentTarget.value)}
            >
              <option value="soniox">{tr.providerSoniox}</option>
              <option value="groq">Groq · Free theo hạn mức tài khoản</option>
              <option value="openai">{tr.aiProviderOpenai}</option>
              <option value="deepseek">{tr.aiProviderDeepseek}</option>
              <option value="gemini">{tr.aiProviderGemini}</option>
              <option value="openai-compatible"
                >{tr.aiProviderCompatible}</option
              >
            </select></label
          >
          {#if confProvider === "openai-compatible"}<label class="ai-field ai-field-full">{tr.aiBaseUrl}<input aria-label={tr.aiBaseUrl} bind:value={confBaseUrl} placeholder="https://your-provider.example/v1" oninput={()=>{confTested=false;confStatusText="";}} /></label>{/if}
          {#if confProvider === "groq"}<p class="ai-key-note ai-field-full">Dùng endpoint mặc định của Groq: https://api.groq.com/openai/v1. Chọn model sau khi lưu, tại phần Gán cấu hình cho tác vụ.</p>{/if}
          <label class="ai-field ai-field-full"
            >{tr.aiProfileKey}<input
              type="password"
              bind:value={confKey}
              aria-label={tr.aiProfileKey}
              placeholder={confEditingId ? tr.aiKeyKeepPlaceholder : ""}
              autocomplete="off"
              spellcheck="false"
              oninput={() => {
                confTested = false;
                confStatusText = "";
                confTestSignature = "";
              }}
            /></label
          >
        </fieldset>
        <div class="ai-actions">
          <button
            type="button"
            class="button secondary"
            disabled={confTesting || (!confKey.trim() && !confEditingId)}
            onclick={() => testProfile()}
            >{confTesting ? tr.aiTesting : tr.aiCheckConnection}</button
          >
          <span
            class="ai-key-state"
            class:test-success={confTested}
            class:test-error={!confTested && !confTesting && !!confStatusText}
            role="status"
            aria-live="polite">{confStatusText}</span
          >
          <button
            type="button"
            class="button primary"
            disabled={busy ||
              confTesting ||
              !confName.trim() ||
              !confTested ||
              confTestSignature !== confSignature()}
            onclick={() => saveProfile()}
            >{confEditingId ? tr.aiProfileSave : tr.aiProfileAdd}</button
          >
        </div>
        <p class="ai-key-note">{tr.aiTestBeforeSave}</p>
        <p class="ai-key-note">Chỉ kiểm tra kết nối và API key. Model được chọn riêng cho từng tác vụ; quyền dùng model có thể khác nhau.</p>
    </dialog>
  {/if}
  {#if meetingsOpen}
    <dialog
      class="confirm-dialog meetings-dialog"
      bind:this={meetingsDialog}
      aria-labelledby="meetings-title"
      onclose={() => {
        meetingsOpen = false;
      }}
      oncancel={() => {
        meetingsOpen = false;
      }}
    >
      <div class="meetings-dialog-head">
        <h2 id="meetings-title">{tr.yourMeetings}</h2>
        <button
          class="icon-button"
          title={tr.closeMeetings}
          aria-label={tr.closeMeetings}
          onclick={() => {
            meetingsOpen = false;
          }}><X size={17} /></button
        >
      </div>
      <label class="sidebar-search"
        ><Search size={15} /><input
          aria-label={tr.searchMeetings}
          placeholder={tr.searchPlaceholder}
          bind:value={query}
        /></label
      >
      <div class="meetings-list">
        {#each history as s (s.id)}
          <article class="meeting-card" class:selected={selected === s.id}>
            <button class="meeting-card-main" onclick={() => choose(s)}>
              <span class="meeting-card-icon"><FileText size={18} /></span>
              <span class="meeting-card-text"
                ><strong>{s.title}</strong><small
                  >{day(s.created)} · {time(s.created)}{s.duration
                    ? ` · ${clock(s.duration)}`
                    : ""}</small
                ></span
              >
              {#if s.id === data.active_id}<span
                  class="live-dot"
                  aria-label={tr.activeSession}
                ></span>{/if}
            </button>
            <div class="meeting-card-actions">
              <button
                class="card-action"
                title={tr.renameMeeting}
                aria-label={`${tr.renameMeeting}: ${s.title}`}
                onclick={() => startRename(s)}
                ><Pencil size={14} /><span>{tr.renameShort}</span></button
              >
              <button
                class="card-action danger"
                title={tr.deleteMeeting}
                aria-label={`${tr.deleteMeeting}: ${s.title}`}
                onclick={() => askDelete(s)}
                ><Trash2 size={14} /><span>{tr.deleteShort}</span></button
              >
            </div>
            {#if renamingId === s.id}
              <div class="meeting-rename">
                <input
                  id={`rename-${s.id}`}
                  bind:value={renameDraft}
                  aria-label={tr.renamePlaceholder}
                  onkeydown={(event) => {
                    if (event.key === "Enter") {
                      event.preventDefault();
                      void saveRename(s);
                    } else if (event.key === "Escape") {
                      renamingId = "";
                    }
                  }}
                />
                <button
                  class="icon-button"
                  title={tr.renameSave}
                  aria-label={tr.renameSave}
                  disabled={busy}
                  onclick={() => saveRename(s)}><Check size={15} /></button
                >
                <button
                  class="icon-button"
                  title={tr.editCancel}
                  aria-label={tr.editCancel}
                  onclick={() => {
                    renamingId = "";
                  }}><X size={15} /></button
                >
              </div>
            {/if}
          </article>
        {:else}
          <p class="history-empty">
            {query ? tr.noMeetings : tr.savedMeetings}
          </p>
        {/each}
      </div>
      <div class="meetings-dialog-footer">
        <button
          class="button primary"
          onclick={() => {
            meetingsOpen = false;
            newMeeting();
          }}><Plus size={16} /> {tr.newMeeting}</button
        >
      </div>
    </dialog>
  {/if}
  {#if smoothOpen}
    <dialog
      class="confirm-dialog smooth-dialog"
      bind:this={smoothDialog}
      aria-labelledby="smooth-title"
      onclose={() => {
        smoothOpen = false;
      }}
      oncancel={() => {
        smoothOpen = false;
      }}
    >
      <h2 id="smooth-title">{tr.smoothHeading}</h2>
      <p>{tr.smoothIntro}</p>
      <label class="summary-field"
        ><span>{tr.smoothConfigPick}</span><select
          value={smoothProfileId}
          disabled={busy}
          onchange={(e) => {
            smoothProfileId = e.currentTarget.value;
            useSessionProfile(smoothProfileId);
            llmConnectionSignature = "";
          }}
        >
          <option value="">{tr.smoothConfigManual}</option>
          {#each chatProfiles(aiProfiles) as p}<option value={p.id}
              >{p.name} · {p.provider}</option
            >{/each}
        </select></label
      >
      <label class="summary-field"
        >{tr.aiModel}<input
          bind:value={smoothModel}
          disabled={busy}
          list="smooth-task-models"
          oninput={() => (llmConnectionSignature = "")}
        /></label
      >
      <datalist id="smooth-task-models"
        >{#each textModels(smoothProfileId, aiProfiles) as m}<option value={m} label={modelLabel(smoothProfileId,m)}></option>{/each}</datalist
      >
      {#if smoothProfileId}<p class="quality-hint">
          {fill(tr.smoothUsingConfig, {
            name: profileOf(smoothProfileId, aiProfiles)?.name ?? "",
          })}
        </p>{/if}
      <p class="smooth-config">
        {fill(tr.smoothUsing, {
          provider: smoothProfileId
            ? (profileOf(smoothProfileId, aiProfiles)?.provider ?? "")
            : smoothProvider,
          model: smoothModel || smoothDefaults[smoothProvider],
        })}
      </p>
      {#if llmConnected}
        <p class="smooth-connected" role="status">{tr.aiConnected}</p>
      {:else}
        <p class="smooth-connect-hint">{tr.aiConnectRequired}</p>
        {#if !data.llm_configured}<p class="smooth-connect-hint">
            {tr.aiNeedKey}
          </p>{/if}
        <button
          class="button secondary small"
          disabled={busy}
          onclick={() => testLlmConnection()}>{tr.aiTestConnection}</button
        >
        {#if !data.llm_configured}<button
            class="text-button"
            disabled={busy}
            onclick={() => {
              smoothOpen = false;
              page = "settings";
            }}>{tr.aiOpenSettings}</button
          >{/if}
      {/if}
      {#if meeting?.smoothed}
        <label class="smooth-prompt"
          >{tr.smoothPromptLabel}<textarea
            bind:value={smoothPrompt}
            placeholder={tr.smoothPromptPlaceholder}
            aria-label={tr.smoothPromptLabel}></textarea>
        </label>
      {/if}
      <div>
        <button
          class="button secondary"
          onclick={() => {
            smoothOpen = false;
          }}>{tr.editCancel}</button
        ><button
          class="button primary"
          disabled={busy || !llmConnected}
          onclick={() => smoothTranscript()}
          >{busy
            ? tr.smoothRunning
            : meeting?.smoothed
              ? tr.smoothAgain
              : tr.smoothButton}</button
        >
      </div>
      <p class="smooth-note">{tr.smoothOriginalKept}</p>
    </dialog>
  {/if}
  {#if summaryOpen}
    <dialog
      class="confirm-dialog summary-dialog"
      bind:this={summaryDialog}
      aria-labelledby="summary-dialog-title"
      onclose={() => {
        summaryOpen = false;
      }}
      oncancel={() => {
        summaryOpen = false;
      }}
    >
      <h2 id="summary-dialog-title">
        <ClipboardList size={18} />
        {tr.summaryTitle}
      </h2>
      <div class="summary-form">
        {#if meeting?.smoothed}
          <label class="summary-field"
            ><span>{tr.summarySource}</span><select
              bind:value={summarySource}
              disabled={busy}
              aria-label={tr.summarySource}
            >
              <option value="original">{tr.summarySourceOriginal}</option>
              <option value="smoothed">{tr.summarySourceSmoothed}</option>
            </select></label
          >
        {/if}
        <label class="summary-field"
          ><span>{tr.summaryStyle}</span><select
            bind:value={summaryStyle}
            disabled={busy}
            aria-label={tr.summaryStyle}
          >
            <option value="overview">{tr.summaryStyleOverview}</option>
            <option value="decisions">{tr.summaryStyleDecisions}</option>
            <option value="full">{tr.summaryStyleFull}</option>
          </select></label
        >
        <label class="summary-field"
          ><span>{tr.summaryPromptLabel}</span><textarea
            bind:value={summaryPrompt}
            placeholder={tr.summaryPromptPlaceholder}
            aria-label={tr.summaryPromptLabel}
            rows="3"></textarea></label
        >
        <p class="smooth-config">
          {fill(tr.smoothUsing, {
            provider: summaryProfileId
              ? (profileOf(summaryProfileId, aiProfiles)?.provider ?? "")
              : smoothProvider,
            model: summaryModel,
          })}
        </p>
        <label class="summary-field"
          ><span>{tr.smoothConfigPick}</span><select
            value={summaryProfileId}
            disabled={busy}
            onchange={(e) => {
              summaryProfileId = e.currentTarget.value;
              summaryModel = textModels(summaryProfileId, aiProfiles)[0] ?? "";
              llmConnectionSignature = "";
            }}
          >
            <option value="">{tr.smoothConfigManual}</option>
            {#each chatProfiles(aiProfiles) as p}<option value={p.id}
                >{p.name} · {p.provider}</option
              >{/each}
          </select></label
        >
        <label class="summary-field"
          >{tr.aiModel}<input
            bind:value={summaryModel}
            disabled={busy}
            list="summary-task-models"
            oninput={() => (llmConnectionSignature = "")}
          /></label
        >
        <datalist id="summary-task-models"
          >{#each textModels(summaryProfileId, aiProfiles) as m}<option
              value={m}
              label={modelLabel(summaryProfileId,m)}
            ></option>{/each}</datalist
        >
        {#if summaryProfileId}<p class="quality-hint">
            {fill(tr.smoothUsingConfig, {
              name: profileOf(summaryProfileId, aiProfiles)?.name ?? "",
            })}
          </p>{/if}
        {#if llmConnected}
          <p class="smooth-connected" role="status">{tr.aiConnected}</p>
        {:else}
          <p class="smooth-connect-hint">{tr.aiConnectRequired}</p>
          {#if !data.llm_configured}<p class="smooth-connect-hint">
              {tr.aiNeedKey}
            </p>{/if}
          <div class="summary-connect-row">
            <button
              class="button secondary small"
              disabled={busy}
              onclick={() => testLlmConnection()}>{tr.aiTestConnection}</button
            >
            {#if !data.llm_configured}<button
                class="text-button"
                disabled={busy}
                onclick={() => {
                  summaryOpen = false;
                  page = "settings";
                }}>{tr.aiOpenSettings}</button
              >{/if}
          </div>
        {/if}
      </div>
      <div class="summary-dialog-footer">
        <button
          class="button secondary"
          disabled={busy}
          onclick={() => {
            summaryOpen = false;
          }}>{tr.editCancel}</button
        >
        <button
          class="button primary"
          disabled={busy || !llmConnected}
          onclick={() => createSummary()}
        >
          {busy
            ? tr.summaryCreating
            : meeting?.summary
              ? tr.summaryRegenerate
              : tr.summaryCreate}
        </button>
      </div>
    </dialog>
  {/if}
  {#if modelPickerOpen}
    <dialog
      class="confirm-dialog model-picker-dialog"
      bind:this={modelPickerDialog}
      aria-labelledby="model-picker-title"
      onclose={() => {
        modelPickerOpen = false;
      }}
      oncancel={() => {
        modelPickerOpen = false;
      }}
    >
      <div class="model-picker-head">
        <div>
          <h2 id="model-picker-title">
            <List size={18} />
            {tr.modelPickerTitle}
          </h2>
          <p class="model-picker-subtitle">
            {fill(tr.modelPickerCount, {
              count: filteredModels.length,
              total: smoothModels.length,
            })}
          </p>
        </div>
        <button
          type="button"
          class="icon-button"
          aria-label={tr.closeNotice}
          title={tr.closeNotice}
          onclick={() => {
            modelPickerOpen = false;
          }}
        >
          <X size={16} />
        </button>
      </div>

      <div class="model-picker-search-bar">
        <Search size={15} />
        <input
          type="text"
          bind:value={modelPickerSearch}
          placeholder={tr.modelPickerSearchPlaceholder}
          aria-label={tr.modelPickerSearchPlaceholder}
          autocomplete="off"
          spellcheck="false"
        />
        {#if modelPickerSearch}
          <button
            type="button"
            class="icon-button small"
            aria-label={tr.clearSearch}
            onclick={() => {
              modelPickerSearch = "";
            }}
          >
            <X size={13} />
          </button>
        {/if}
      </div>

      <div class="model-picker-list" role="listbox">
        {#each filteredModels as m}
          <button
            type="button"
            class="model-picker-item"
            class:selected={smoothModel === m}
            role="option"
            aria-selected={smoothModel === m}
            onclick={() => {
              if (smoothModel !== m) {
                smoothModel = m;
                llmConnectionSignature = "";
              }
              modelPickerOpen = false;
            }}
          >
            <span class="model-picker-name">{m}</span>
            {#if smoothModel === m}
              <span class="model-picker-badge"
                ><Check size={14} /> {tr.modelSelected}</span
              >
            {/if}
          </button>
        {:else}
          <div class="model-picker-empty">
            <p>{tr.modelPickerEmpty}</p>
          </div>
        {/each}
      </div>

      <div class="model-picker-footer">
        <button
          type="button"
          class="button secondary"
          onclick={() => {
            modelPickerOpen = false;
          }}
        >
          {tr.editCancel}
        </button>
      </div>
    </dialog>
  {/if}
  {#if deleteOpen}
    <dialog
      class="confirm-dialog"
      bind:this={deleteDialog}
      aria-labelledby="delete-title"
      onclose={() => {
        deleteOpen = false;
        deleteTarget = null;
      }}
      oncancel={() => {
        deleteOpen = false;
        deleteTarget = null;
      }}
    >
      <h2 id="delete-title">{tr.deleteTitle}</h2>
      <p>
        {fill(tr.deleteBody, {
          title: deleteTarget?.title ?? meeting?.title ?? "",
        })}
      </p>
      <div>
        <button
          class="button secondary"
          onclick={() => {
            deleteOpen = false;
            deleteTarget = null;
          }}>{tr.keep}</button
        ><button class="button stop" disabled={busy} onclick={remove}
          >{tr.deleteConfirm}</button
        >
      </div>
    </dialog>
  {/if}
  {#if stopOpen}
    <dialog
      class="confirm-dialog"
      bind:this={stopDialog}
      aria-labelledby="stop-title"
      onclose={() => {
        stopOpen = false;
      }}
      oncancel={() => {
        stopOpen = false;
      }}
    >
      <h2 id="stop-title">{tr.stopConfirmTitle}</h2>
      <p>{tr.stopConfirmBody}</p>
      <div>
        <button
          class="button secondary"
          onclick={() => {
            stopOpen = false;
          }}>{tr.stopKeep}</button
        ><button class="button stop" disabled={busy} onclick={reallyFinish}
          >{tr.stopConfirm}</button
        >
      </div>
    </dialog>
  {/if}
{/if}
