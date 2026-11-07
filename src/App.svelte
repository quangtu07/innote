<script lang="ts">
  import { onMount, onDestroy, tick } from "svelte";
  import { version as appVersion } from "../package.json";
  import { SONIOX_RT_MODEL, sonioxV6Preferences } from "./lib/soniox-model";
  import { invoke, convertFileSrc } from "@tauri-apps/api/core";
  import { listen, emit } from "@tauri-apps/api/event";
  import { check, type Update } from "@tauri-apps/plugin-updater";
  import type { Segment } from "./types";
  import Logo from "./Logo.svelte";
  import MeetingStorage from "./MeetingStorage.svelte";
  import SpeechQualityControl from "./SpeechQualityControl.svelte";
  import WhatsNewDialog from "./WhatsNewDialog.svelte";
  import {SPEECH_LEVELS_KEY,parseProcessingLevels,qualityKey,qualitySupported,type SpeechQuality} from "./lib/speech-quality";
  import CaptionControls from "./CaptionControls.svelte";
  import CaptionWindowToggle from "./CaptionWindowToggle.svelte";
  import { CaptionPresentation, type CaptionCue } from "./lib/caption-presentation";
  import { readCaptionStyle, CAPTION_STYLE_KEY, type CaptionStyle } from "./lib/caption-style";
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
    PanelLeftClose,
    PanelLeftOpen,
  } from "@lucide/svelte";
  import type { Snapshot, Meeting } from "./types";
  import {PRESETS_KEY,parsePresets,serializePresets,copyPreset,realtimeAssignments,writePresetPreferences,type ModelChoice,type WorkflowPreset,type PresetMode,type TaskAssignments} from "./lib/workflow-presets";
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
  let sidebarCollapsed=false;
  try{sidebarCollapsed=localStorage.getItem("innote.sidebar.collapsed")==="true";}catch{}
  function toggleSidebar(){sidebarCollapsed=!sidebarCollapsed;try{localStorage.setItem("innote.sidebar.collapsed",String(sidebarCollapsed));}catch{}}
  let page = "meeting";
  $: captionNavLabel=uiLocale === "en" ? "Captions only" : uiLocale === "ko" ? "자막만" : uiLocale === "zh" ? "仅字幕" : "Chỉ phụ đề";
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
    const mode=JSON.parse(key)[0] as PresetMode;
    if(mode === "meeting" || mode === "captionsonly")markPresetCustom(mode);
    processingLevels={...processingLevels,[key]:quality};
    localStorage.setItem(SPEECH_LEVELS_KEY,JSON.stringify(processingLevels));
  }
  let sourcesInitialized = false;
  let transcribe = true;
  let meetingCaptions = true;
  let meetingTranslate = true;
  let meetingStyleOpen=false;
  let meetingTargetLang = "vi";
  let localFallback = false;
  let translationView: "original" | "translation" | "bilingual" = "original";
  let translationLanguage = "";
  let translationMeeting = "";
  $: if ((selected ?? "") !== translationMeeting) { translationMeeting=selected ?? ""; translationView="original"; translationLanguage=""; }
  $: translationLanguages = [...new Set((meeting?.translations ?? []).map(t=>t.language))];
  $: visibleTranslationLanguage = translationLanguage || meeting?.speech?.translation_language || translationLanguages[0] || "vi";
  $: savedTranslations = (meeting?.translations ?? []).filter(t=>t.language===visibleTranslationLanguage);
  let meetingTranslationPreview: Snapshot["translation_preview"] = null;
  let translationPreviewRevision = 0;
  $: liveTranslation = continuousTranscript && meetingTranslationPreview?.id === meeting?.id
    && meetingTranslationPreview?.language === visibleTranslationLanguage
    && meetingTranslationPreview?.committed === savedTranslations.length
    ? meetingTranslationPreview.text : "";
  $: if(active?.speech) {
    meetingTranslate=!!active.speech.translation_language;
    if(active.speech.translation_language) meetingTargetLang=active.speech.translation_language;
  }

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
  type TaskAssign = TaskAssignments;
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
  let summaryModel = "";
  let summaryFallback:ModelChoice|undefined;
  let summaryError="";
  const realtimeModels: Record<string, string[]> = {
    soniox: [SONIOX_RT_MODEL],
    openai: ["gpt-realtime-translate"],
    gemini: ["gemini-3.5-live-translate-preview"],
  };
  function captionModels(id: string, profiles: AiProfileSummary[], translate = true, source = "auto") {
    if(id === "__local__")return translate ? [] : source === "vi" ? ["moonshine-vi",multilingualModel] : source === "en" ? ["moonshine-en",multilingualModel] : ["multi","ko","zh"].includes(source) ? [multilingualModel] : ["","auto"].includes(source) ? ["moonshine-vi","moonshine-en",multilingualModel] : [];
    const provider=profileOf(id,profiles)?.provider??"";
    if(!translate)return provider === "groq" ? ["whisper-large-v3-turbo","whisper-large-v3"] : provider === "soniox" ? [SONIOX_RT_MODEL] : [];
    return realtimeModels[provider] ?? [];
  }
  const groqSpeechModels=["whisper-large-v3-turbo","whisper-large-v3"];
  function meetingSpeechModels(id:string,profiles:AiProfileSummary[]){const p=profileOf(id,profiles)?.provider;return p==="groq"?groqSpeechModels:p==="soniox"?[SONIOX_RT_MODEL]:[];}
  function selectableCaptionModels(id:string,profiles:AiProfileSummary[],translate:boolean,source="auto"){return profileOf(id,profiles)?.provider==="groq"?groqSpeechModels:captionModels(id,profiles,translate,source);}
  function speechProfiles(profiles:AiProfileSummary[]){return profiles.filter(p=>["groq","soniox"].includes(p.provider));}
  function correctionModels(id:string,profiles:AiProfileSummary[],source=meeting?.language??language){if(id==="__local__")return compatibleLocalModels(source==="auto"?"multi":source).filter(id=>data.models.some(m=>m.id===id&&m.installed));const provider=profileOf(id,profiles)?.provider;return provider==="groq"?groqSpeechModels:provider==="soniox"?["stt-async-v6"]:[];}
  function correctionProfiles(profiles:AiProfileSummary[]){return profiles.filter(p=>p.has_key&&correctionModels(p.id,profiles).length);}
  function defaultCorrectionModel(source:string){return source === "vi" ? "moonshine-vi" : source === "en" ? "moonshine-en" : ["multi","auto","ko","zh"].includes(source) ? multilingualModel : "";}
  let correctionProfileId="",correctionModel="";
  let correctionPendingId="",correctionError="";
  let correctionFallback:ModelChoice|undefined;
  $: correctionJob=data.corrections?.find(j=>j.id===meeting?.id);
  $: correctionRunning=!!correctionJob || (!!meeting&&correctionPendingId===meeting.id);
  $: canCorrect=!!meeting&&!['recording','paused','processing'].includes(meeting.status)&&!!audioUrl;
  function fallbackChoices(task:keyof TaskAssign, profiles:AiProfileSummary[], assignment:TaskAssign|undefined, mode:string, translate:boolean,source?:string) {
    const choices=profiles.filter(p=>p.has_key).flatMap(p=>{
      const primaryProvider=profileOf(assignment?.[task]?.profileId,profiles)?.provider;
      const models=task === "correction" ? correctionModels(p.id,profiles,source) : task === "transcription" || task === "caption" && mode === "meeting" ? (p.provider===primaryProvider?meetingSpeechModels(p.id,profiles):[]) : task === "caption" ? captionModels(p.id,profiles,translate) : chatProviders.includes(p.provider as any) ? textModels(p.id,profiles) : [];
      return models.map(model=>({profileId:p.id,model}));
    });
    if(task === "correction")choices.push(...correctionModels("__local__",profiles,source).map(model=>({profileId:"__local__",model})));
    return choices.filter(choice=>choice.profileId!==assignment?.[task]?.profileId || choice.model!==assignment?.[task]?.model);
  }
  function taskLabel(task:keyof TaskAssign){return task === "transcription" ? tr.taskTranscription : task === "caption" ? tr.taskCaption : task === "correction" ? "Hiệu chỉnh từ bản thu" : tr.taskSummary;}
  function chooseFallback(task:keyof TaskAssign,value:string){
    const primary=taskAssigns[taskMode]?.[task];if(!primary)return;
    setAssignment(task,{...primary,fallback:value ? JSON.parse(value) : undefined});
  }
  function textModels(id: string, profiles: AiProfileSummary[],_source?:string) {
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
  let transcriptionModel="whisper-large-v3-turbo";
  let profilesReady=false,meetingChoiceHydrated=false;
  let rememberedMeetingChoice:ModelChoice|undefined;
  $: currentMeetingModel=provider === "local" ? effectiveModel : provider === "groq" ? transcriptionModel : SONIOX_RT_MODEL;
  $: if(assignmentsReady&&profilesReady&&!meetingChoiceHydrated){
    const choice=rememberedMeetingChoice??taskAssigns.meeting?.transcription;
    const p=profileOf(choice?.profileId,aiProfiles);
    if(choice?.profileId==="__local__"){provider="local";transcriptionProfileId="";}
    else if(p&&["groq","soniox"].includes(p.provider)&&meetingSpeechModels(p.id,aiProfiles).includes(choice?.model??"")){provider=p.provider;transcriptionProfileId=p.id;transcriptionModel=choice!.model;}
    meetingChoiceHydrated=true;
  }
  function changeMeetingProvider(value:string){
    provider=value;
    if(value==="local"){transcriptionProfileId="";return;}
    const profiles=speechProfiles(aiProfiles).filter(p=>p.provider===value);
    if(!profiles.some(p=>p.id===transcriptionProfileId))transcriptionProfileId=profiles.length===1?profiles[0].id:"";
    if(value==="groq"&&!groqSpeechModels.includes(transcriptionModel))transcriptionModel=groqSpeechModels[0];
    if(value==="groq"&&localFallback){localFallback=false;notify("Groq dùng API dự phòng chọn trong mẫu; dự phòng local hiện chỉ áp dụng cho Soniox.");}
  }
  $: meetingSharedChoice = provider !== "local" && transcriptionProfileId
    ? JSON.stringify({profileId:transcriptionProfileId,model:currentMeetingModel})
    : provider === "local" && !meetingTranslate ? JSON.stringify({profileId:"__local__",model:effectiveModel}) : "";
  function chooseMeetingSharedModel(value:string) {
    let choice:ModelChoice;try{choice=JSON.parse(value);}catch{return;}
    if(choice.profileId === "__local__" && !meetingTranslate){chooseLocalMeetingModel(choice.model);provider="local";transcriptionProfileId="";return;}
    const profile=profileOf(choice.profileId,aiProfiles);
    if(!profile||!meetingSpeechModels(profile.id,aiProfiles).includes(choice.model)){error="Model chưa hỗ trợ chép lời cuộc họp.";return;}
    changeMeetingProvider(profile.provider);transcriptionProfileId=profile.id;transcriptionModel=choice.model;error="";
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
  let correctionOpen = false;
  let correctionDialog: HTMLDialogElement;
  let transcriptView: "original" | "corrected" = "original";
  let smoothModels: string[] = [];
  let llmConnectionSignature = "";
  type SummaryStyle = "overview" | "decisions" | "full";
  let summaryStyle: SummaryStyle = "overview";
  let summarySource: "original" | "corrected" = "original";
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
  $: if (correctionOpen)
    tick().then(() => {
      if (correctionDialog && !correctionDialog.open) correctionDialog.showModal();
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
  $: canSummarize =
    !!meeting &&
    (meeting.segments.length > 0 || !!meeting.corrected) &&
    !["recording", "paused", "processing"].includes(meeting.status);
  $: llmConfigSignature = JSON.stringify([
    summaryOpen ? summaryProfileId : smoothProfileId,
    summaryOpen ? summaryModel : smoothModel,
    smoothProvider,
    smoothBaseUrl,
    aiProfiles.map((p) => [p.id, p.provider, p.base_url, p.key_tail]),
  ]);
  $: llmConnected = llmConfigSignature === llmConnectionSignature;
  $: summaryReady=!!profileOf(summaryProfileId,aiProfiles)?.has_key && chatProfiles(aiProfiles).some(p=>p.id===summaryProfileId) && !!summaryModel.trim();
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
      meeting.summary.source === "corrected"
        ? (meeting.corrected?.text ?? "")
        : meeting.summary.source === "smoothed" ? (meeting.smoothed?.text ?? "")
        : meeting.segments.map((s) => s.text).join("\n");
    return summaryHash(source);
  })();
  $: summaryStale =
    !!meeting?.summary?.source_hash &&
    summaryCurrentHash !== meeting.summary.source_hash;
  $: correctedSegments = meeting?.corrected?.segments ?? [];
  $: exportingVersion =
    transcriptView === "corrected" && meeting?.corrected
      ? "corrected"
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
  $: meetingLevelKey=qualityKey("meeting",provider==="local"?"__local__":transcriptionProfileId,currentMeetingModel,language,meetingTranslate);
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
  $: if(liveTranslation) void followLive("",liveTranslation,autoScroll);
  $: segments =
    (translationView === "translation" ? savedTranslations.map(t=>({...t,edited:false})) : meeting?.segments ?? []).filter((s) =>
      s.text
        .toLocaleLowerCase("vi")
        .includes(searchText.toLocaleLowerCase("vi")),
    );
  $: continuousTranscript = !!meeting && ["recording", "paused"].includes(meeting.status);
  $: flowingFinalText = segments.map(s=>s.text).join(" ");
  $: if (sourcesInitialized && meetingChoiceHydrated) {
    try {
      localStorage.setItem(
        "may.preferences",
        JSON.stringify({ system, microphone, language, model, transcribe, meetingCaptions, meetingTranslate, meetingTargetLang, localFallback, speechSelection:{profileId:provider==="local"?"__local__":transcriptionProfileId,model:currentMeetingModel} }),
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
      const previewRevision = translationPreviewRevision;
      const snapshot = await invoke<Snapshot>("snapshot");
      if (request < snapshotApplied) return;
      snapshotApplied = request;
      data = snapshot;
      if(previewRevision === translationPreviewRevision) meetingTranslationPreview=snapshot.translation_preview ?? null;
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
    correctionOpen = false;
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
    selectedPresets={...selectedPresets,meeting:""};pendingPresets={...pendingPresets,meeting:""};
    meetingCaptions=true;meetingTranslate=true;localFallback=false;meetingStyleOpen=false;
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
    provider = preferred && preferred !== "__local__" ? profileOf(preferred,aiProfiles)?.provider??"local" : "local";
    transcriptionModel=taskAssigns.meeting?.transcription?.model??"whisper-large-v3-turbo";
    transcriptionProfileId = preferred === "__local__" ? "" : preferred;
    correctionOpen = false;
    smoothPrompt = "";
    transcriptView = "original";
    meetingsOpen = false;
    renamingId = "";
    summaryOpen = false;
    transcriptTab = "transcript";
  }
  async function start() {
    await action(async () => {
      if(meetingPresetPending)throw new Error("Mẫu cuộc họp chưa được áp dụng. Hãy bấm Áp mẫu hoặc chọn Tùy chỉnh.");
      if (transcribe && meetingTranslate && (provider !== "soniox" || !transcriptionProfileId)) {
        throw new Error("Hãy chọn cấu hình/model Soniox dùng chung cho chép lời và phụ đề ngay trong phần phụ đề cuộc họp.");
      }
      const s = await invoke<Meeting>("start", {
        options: {
          title,
          system: system || null,
          microphone: microphone || null,
          language,
          model: provider === "groq" ? transcriptionModel : effectiveModel,
          speechQuality: meetingSpeechQuality,
          transcribe,
          provider,
          translationLanguage: transcribe && provider === "soniox" && meetingTranslate ? meetingTargetLang : null,
          captionsEnabled: transcribe && meetingCaptions && (provider === "soniox" || !meetingTranslate),
          localFallback: provider === "soniox" && localFallback,
          transcriptionFallback: provider !== "local" ? taskAssigns.meeting?.transcription?.fallback ?? null : null,
          profileId:
            provider !== "local" ? transcriptionProfileId || null : null,
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
  async function correctTranscript() {
    if(!meeting||!canCorrect||correctionRunning)return;
    const id=meeting.id;
    if(!correctionModels(correctionProfileId,aiProfiles).includes(correctionModel)){correctionError="Hãy chọn cấu hình và model nhận âm thanh để hiệu chỉnh.";return;}
    correctionPendingId=id;correctionError="";
    try {
      const updated=await invoke<Meeting>("correct_transcript",{id,profileId:correctionProfileId,model:correctionModel,fallback:correctionFallback??null});
      data={...data,sessions:data.sessions.map(s=>s.id===id?updated:s)};
      if(selected===id){correctionOpen=false;transcriptView="corrected";transcriptTab="transcript";translationView="original";}
      notify("Đã tạo bản hiệu chỉnh từ file ghi âm. Bản gốc được giữ nguyên.");
    } catch(e) {correctionError=String(e);if(!correctionOpen)notify(correctionError);}
    finally {correctionPendingId="";await refresh();}
  }
  async function cancelCorrection(){if(meeting)await invoke("cancel_correction",{id:meeting.id});await refresh();}
  async function copyCorrected() {
    try {
      await navigator.clipboard.writeText(meeting?.corrected?.text ?? "");
      notify(tr.smoothCopied);
    } catch (e) {
      error = String(e);
    }
  }
  async function createSummary() {
    if (!meeting || !canSummarize || busy) return;
    summaryError="";
    const profile = profileOf(summaryProfileId, aiProfiles);
    if(!summaryReady || !profile){summaryError="Hãy chọn cấu hình AI và model tóm tắt.";return;}
    const id=meeting.id;
    pendingActions++;
    try {
      const updated = await invoke<Meeting>("summarize_transcript", {
        id, provider:profile.provider,model:summaryModel.trim(),baseUrl:profile.base_url,
        prompt:summaryPrompt,style:summaryStyle,source:summarySource,key:null,
        profileId:profile.id,fallback:summaryFallback??null,
      });
      if(id===meeting?.id){
        data={...data,sessions:data.sessions.map(s=>s.id===id?updated:s)};
        transcriptTab="summary";summaryOpen=false;summaryPrompt="";notify(tr.notifySummaryCreated);
      }
    } catch(e){summaryError=String(e);}
    finally{pendingActions--;}
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
    const text=transcriptView==="corrected"&&meeting.corrected ? meeting.corrected.text : translationView==="translation"?translated:translationView==="bilingual"?`BẢN GỐC\n${original}\n\nBẢN DỊCH (${visibleTranslationLanguage})\n${translated}`:original;
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
    try {
      const upgrade=sonioxV6Preferences(localStorage);
      if(Object.keys(upgrade).length){
        writePresetPreferences(localStorage,upgrade);
        notify("Đã nâng cấu hình và mẫu Soniox lên stt-rt-v6. Bản chép đã lưu và API key được giữ nguyên.");
      }
    } catch {
      error="Chưa lưu được cấu hình Soniox v6 trên máy. Hãy kiểm tra dung lượng/quyền lưu rồi mở lại Innote; cấu hình cũ được giữ lại.";
    }
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
        meetingCaptions=stored.meetingCaptions!==false;meetingTranslate=stored.meetingTranslate!==false;
        meetingTargetLang=["vi","en","ko","zh","ja"].includes(stored.meetingTargetLang)?stored.meetingTargetLang:"vi";localFallback=stored.localFallback===true;rememberedMeetingChoice=stored.speechSelection;
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
      profilesReady=true;
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
    const savedPresets=localStorage.getItem(PRESETS_KEY);
    workflowPresets=parsePresets(savedPresets);
    if(savedPresets===null)persistPresets([capturePreset("meeting"),capturePreset("captionsonly")]);
    else if(workflowPresets.some(p=>!p.captionStyle))persistPresets(workflowPresets.map(p=>({...p,captionStyle:p.captionStyle??copyPreset(readCaptionStyle())})));
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
    listen<Snapshot["translation_preview"]>("meeting-translation-preview", ({payload})=>{
      translationPreviewRevision++;
      meetingTranslationPreview=payload;
    }).then(fn=>un.push(fn)).catch(()=>{});
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
    if(mode==="standalone" && captionPresetPending){error="Mẫu phụ đề chưa được áp dụng. Hãy bấm Áp mẫu hoặc chọn Tùy chỉnh.";return;}
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
  async function changeMeetingTranslation(enabled: boolean, target=meetingTargetLang) {
    await action(async()=>{
      await invoke("set_meeting_translation",{target:enabled?target:null});
      meetingTranslationPreview=null;
      await refresh();
    });
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
  let workflowPresets:WorkflowPreset[]=[];
  let selectedPresets:Record<PresetMode,string>={meeting:"",captionsonly:""};
  let pendingPresets:Record<PresetMode,string>={meeting:"",captionsonly:""};
  $: meetingPresetPending=!!pendingPresets.meeting&&pendingPresets.meeting!==selectedPresets.meeting;
  $: captionPresetPending=!!pendingPresets.captionsonly&&pendingPresets.captionsonly!==selectedPresets.captionsonly;
  let presetEditing=false,presetEditingId="",presetName="",presetError="",presetDeleteId="";
  let presetSource="vi",presetTranslate=false,presetCaptions=false,presetLocalFallback=false;
  let presetQuality:SpeechQuality="balanced";
  let presetCaptionStyle:CaptionStyle=readCaptionStyle();
  let presetAssignments:Record<string,TaskAssign>={};
  function capturePreset(mode:PresetMode):WorkflowPreset {
    const tasks=realtimeAssignments(taskAssigns[mode]??{});
    if(mode==="meeting"){
      tasks.transcription={...tasks.transcription,profileId:provider==="local"?"__local__":transcriptionProfileId,model:currentMeetingModel};
      if(meetingCaptions||meetingTranslate)tasks.caption={...tasks.caption,...tasks.transcription,targetLang:meetingTargetLang,translate:meetingTranslate};
    }else tasks.caption={...tasks.caption,profileId:captionProfileId,model:captionModel,targetLang:captionTargetLang,translate:captionTranslate};
    return {id:crypto.randomUUID(),name:mode==="meeting"?"Cuộc họp hiện tại":"Phụ đề hiện tại",mode,tasks,source:mode==="meeting"?language:captionSourceLang,quality:mode==="meeting"?meetingSpeechQuality:captionSpeechQuality,captions:mode==="meeting"&&meetingCaptions,translate:mode==="meeting"?meetingTranslate:captionTranslate,target:mode==="meeting"?meetingTargetLang:captionTargetLang,transcribe:mode==="meeting"?transcribe:true,localFallback:mode==="meeting"&&localFallback,captionStyle:copyPreset(readCaptionStyle())};
  }
  function persistPresets(next:WorkflowPreset[]){try{localStorage.setItem(PRESETS_KEY,serializePresets(next));workflowPresets=next;return true;}catch{presetError="Không lưu được mẫu trên máy. Hãy kiểm tra dung lượng hoặc quyền lưu dữ liệu.";return false;}}
  function editPreset(p?:WorkflowPreset){
    const draft=copyPreset(p??capturePreset(taskMode));presetEditingId=p?.id??"";presetName=p?.name??"";presetSource=draft.source;presetQuality=draft.quality;presetCaptionStyle=copyPreset(draft.captionStyle??readCaptionStyle());presetTranslate=draft.translate;presetCaptions=draft.captions;presetLocalFallback=draft.localFallback;presetAssignments={[taskMode]:realtimeAssignments(draft.tasks)};presetError="";presetEditing=true;
  }
  function setPresetAssignment(task:keyof TaskAssign,value:any){
    const next={...presetAssignments[taskMode]};
    if(value===undefined)delete next[task];else (next as any)[task]=value;
    if(taskMode==="meeting"&&(presetCaptions||presetTranslate)&&value?.profileId){
      if(task==="caption")next.transcription={profileId:value.profileId,model:value.model,fallback:value.fallback??next.transcription?.fallback};
      if(task==="transcription")next.caption={...next.caption,profileId:value.profileId,model:value.model,targetLang:next.caption?.targetLang??"vi",translate:presetTranslate};
    }
    presetAssignments={...presetAssignments,[taskMode]:next};
  }
  function choosePresetFallback(task:keyof TaskAssign,value:string){const c=presetAssignments[taskMode]?.[task];if(c)setPresetAssignment(task,{...c,fallback:value?JSON.parse(value):undefined});}
  function presetProblem(p:WorkflowPreset){
    if(!p.name.trim())return "Hãy đặt tên cho mẫu.";
    if(!(p.mode==="meeting"?["vi","en","ko","zh","multi","auto"]:["vi","en","ko","zh","ja","auto"]).includes(p.source))return "Ngôn ngữ gốc của mẫu chưa được hỗ trợ trong chế độ này.";
    if(p.translate&&!["vi","en","ko","zh","ja"].includes(p.target))return "Ngôn ngữ đích của mẫu không hợp lệ.";
    const primary=p.mode==="meeting"?p.tasks.transcription:p.tasks.caption;
    if(p.translate&&profileOf(primary?.profileId,aiProfiles)?.provider==="groq")return "Groq Whisper chỉ dùng chép lời gốc trong Innote; hãy bỏ tích Dịch sang.";
    if(p.mode==="meeting"&&p.localFallback&&profileOf(primary?.profileId,aiProfiles)?.provider==="groq")return "Groq dùng cấu hình API dự phòng; bỏ tùy chọn dự phòng local dành cho Soniox.";
    if(p.mode==="meeting"&&(p.captions||p.translate)&&p.tasks.caption?.profileId&&(p.tasks.caption.profileId!==p.tasks.transcription?.profileId||p.tasks.caption.model!==p.tasks.transcription?.model))return "Mẫu cuộc họp phải dùng cùng cấu hình/model cho chép lời và phụ đề.";
    for(const [task,c] of Object.entries(realtimeAssignments(p.tasks))){
      if(!c?.profileId)continue;
      if(c.profileId!=="__local__"&&!profileOf(c.profileId,aiProfiles))return `Cấu hình API cho ${taskLabel(task as keyof TaskAssign)} không còn tồn tại. Hãy chọn lại.`;
      if(c.profileId!=="__local__"&&!profileOf(c.profileId,aiProfiles)?.has_key)return "Cấu hình API chưa có key. Hãy kiểm tra và lưu key trong Cài đặt.";
      if(!c.model)return "Hãy chọn model cụ thể cho tác vụ đã bật.";
      if(c.fallback&&c.fallback.profileId!=="__local__"&&!profileOf(c.fallback.profileId,aiProfiles)?.has_key)return "Cấu hình dự phòng không còn tồn tại. Hãy chọn lại.";
      if(c.fallback&&!fallbackChoices(task as keyof TaskAssign,aiProfiles,p.tasks,p.mode,p.translate,p.source).some(x=>x.profileId===c.fallback!.profileId&&x.model===c.fallback!.model))return "Model dự phòng không phù hợp tác vụ hoặc trùng model chính. Hãy chọn lại.";
      if(task === "transcription" && c.profileId!=="__local__" && !meetingSpeechModels(c.profileId,aiProfiles).includes(c.model))return "Model chép lời cuộc họp không được hỗ trợ. Hãy chọn Local, Soniox hoặc Groq Whisper.";
      if(task === "caption" && (p.mode === "captionsonly" || p.captions || p.translate) && !captionModels(c.profileId,aiProfiles,p.translate,p.source).includes(c.model))return "Model phụ đề không phù hợp chế độ/ngôn ngữ đã chọn.";
    }
    const c=p.mode==="meeting"?p.tasks.transcription:p.tasks.caption;
    if(!c?.profileId||!c.model)return "Hãy chọn cấu hình và model chép lời/phụ đề.";
    if(p.quality!=="balanced"&&!qualitySupported(c.profileId==="__local__"?"local":profileOf(c.profileId,aiProfiles)?.provider??""))return "API này chỉ có mức mặc định đã được xác minh. Hãy chọn Cân bằng.";
    if(c.profileId==="__local__"&&!(p.mode==="meeting"?compatibleLocalModels(p.source==="auto"?"multi":p.source):captionModels("__local__",aiProfiles,false,p.source)).includes(c.model))return "Model local không phù hợp ngôn ngữ gốc. Hãy chọn lại model hoặc ngôn ngữ.";
    if(p.translate && (p.mode==="meeting"?profileOf(c.profileId,aiProfiles)?.provider!=="soniox":!captionModels(c.profileId,aiProfiles,true,p.source).includes(c.model)))return "Model này không hỗ trợ dịch trong chế độ đã chọn. Cuộc họp dùng chung cần Soniox.";
    return "";
  }
  function savePreset(){
    const tasks=realtimeAssignments(presetAssignments[taskMode]??{});
    if(taskMode==="meeting"&&(presetCaptions||presetTranslate)&&tasks.caption?.profileId){tasks.transcription={profileId:tasks.caption.profileId,model:tasks.caption.model,fallback:tasks.caption.fallback??tasks.transcription?.fallback};}
    if(tasks.caption)tasks.caption={...tasks.caption,translate:presetTranslate};
    const draft:WorkflowPreset={id:presetEditingId||crypto.randomUUID(),name:presetName.trim(),mode:taskMode,tasks,source:presetSource,quality:presetQuality,captions:presetCaptions,translate:presetTranslate,target:tasks.caption?.targetLang??"vi",transcribe:true,localFallback:presetLocalFallback,captionStyle:copyPreset(presetCaptionStyle)};
    presetError=presetProblem(draft);if(presetError)return;
    if(persistPresets([...workflowPresets.filter(p=>p.id!==draft.id),draft])){if(selectedPresets[taskMode]===draft.id)selectedPresets={...selectedPresets,[taskMode]:""};presetEditing=false;notify("Đã lưu mẫu cấu hình.");}
  }
  function removePreset(id:string){if(persistPresets(workflowPresets.filter(p=>p.id!==id))){presetDeleteId="";pendingPresets={meeting:pendingPresets.meeting===id?"":pendingPresets.meeting,captionsonly:pendingPresets.captionsonly===id?"":pendingPresets.captionsonly};selectedPresets={meeting:selectedPresets.meeting===id?"":selectedPresets.meeting,captionsonly:selectedPresets.captionsonly===id?"":selectedPresets.captionsonly};}}
  function choosePreset(mode:PresetMode,id:string){
    if(data.active_id||captionRunning)return;
    if(!id){selectedPresets={...selectedPresets,[mode]:""};return;}
    const p=workflowPresets.find(p=>p.id===id&&p.mode===mode);if(!p)return;
    const problem=presetProblem(p);if(problem){error=problem;return;}
    const c=(mode==="meeting"?p.tasks.transcription:p.tasks.caption)!;
    const source=mode==="meeting"&&p.source==="auto"?"multi":p.source;
    const levels={...processingLevels,[qualityKey(mode,c.profileId,c.model,source,p.translate)]:p.quality};
    const localModels=mode==="meeting"&&c.profileId==="__local__"?{...localMeetingModels,[source]:c.model}:localMeetingModels;
    const entries:Record<string,string>={[`may.task_assign.${mode}`]:JSON.stringify(realtimeAssignments(p.tasks)),[SPEECH_LEVELS_KEY]:JSON.stringify(levels)};
    if(p.captionStyle)entries[CAPTION_STYLE_KEY]=JSON.stringify(p.captionStyle);
    if(mode==="meeting"){
      entries["may.meeting.local-models"]=JSON.stringify(localModels);
      entries["may.preferences"]=JSON.stringify({system,microphone,language:source,model,transcribe:p.transcribe,meetingCaptions:p.captions,meetingTranslate:p.translate,meetingTargetLang:p.target,localFallback:p.localFallback,speechSelection:{profileId:c.profileId,model:c.model}});
    }else entries["may.caption.standalone"]=JSON.stringify({profileId:c.profileId,model:c.model,targetLang:p.target,translate:p.translate,sourceLang:source,device:captionDevice});
    try{writePresetPreferences(localStorage,entries);}catch{error="Không lưu được cấu hình mẫu trên máy. Chưa áp mẫu; hãy kiểm tra dung lượng hoặc quyền lưu dữ liệu.";return;}
    taskAssigns={...taskAssigns,[mode]:realtimeAssignments(p.tasks)};processingLevels=levels;
    if(mode==="meeting"){
      localMeetingModels=localModels;language=source;transcribe=p.transcribe;meetingCaptions=p.captions;meetingTranslate=p.translate;meetingTargetLang=p.target;localFallback=p.localFallback;
      provider=c.profileId==="__local__"?"local":profileOf(c.profileId,aiProfiles)!.provider;transcriptionProfileId=provider==="local"?"":c.profileId;transcriptionModel=c.model;
    }else{captionProfileId=c.profileId;captionModel=c.model;captionSourceLang=source;captionTranslate=p.translate;captionTargetLang=p.target;}
    if(p.captionStyle)void emit("caption-style",p.captionStyle).catch(()=>{});
    selectedPresets={...selectedPresets,[mode]:id};pendingPresets={...pendingPresets,[mode]:id};error="";return true;
  }
  function markPresetCustom(mode:PresetMode){
    selectedPresets={...selectedPresets,[mode]:""};pendingPresets={...pendingPresets,[mode]:""};
  }
  function applyPendingPreset(mode:PresetMode){
    const id=pendingPresets[mode];if(!id)return;
    if(choosePreset(mode,id))notify(`Áp mẫu thành công: ${workflowPresets.find(p=>p.id===id)?.name??""}`);
  }
  function usePreset(p:WorkflowPreset){
    if(data.active_id||captionRunning)return;
    const problem=presetProblem(p);if(problem){error=problem;return;}
    if(p.mode==="meeting")newMeeting();
    if(choosePreset(p.mode,p.id)){page=p.mode==="meeting"?"meeting":"caption";notify(`Áp mẫu thành công: ${p.name}`);}
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
    <WhatsNewDialog version={appVersion} locale={uiLocale ?? "vi"} existingInstall={hadPreviousInstallation} ready={desktop && captionStateReady && !!data.data_dir && !data.active_id && !captionRunning && !testing && !confOpen && !meetingsOpen && !summaryOpen && !correctionOpen && !modelPickerOpen && !deleteOpen && !stopOpen && updatePhase !== "downloading" && updatePhase !== "installing"}/>
    <aside class="sidebar" class:collapsed={sidebarCollapsed} id="sidebar-navigation" aria-label={tr.navAria}>
      <div class="sidebar-header">
      <button
        class="brand"
        title="Innote" aria-label="Innote"
        style="background:transparent;padding:0;text-align:left"
        onclick={newMeeting}
        ><span class="brand-mark"><Logo size={24} /></span><span class="brand-copy"
          >Innote<span class="brand-sub">{tr.appTagline}</span></span
        ></button
      >
      <button class="sidebar-toggle" type="button" aria-label={sidebarCollapsed ? "Mở rộng thanh bên" : "Thu gọn thanh bên"} title={sidebarCollapsed ? "Mở rộng thanh bên" : "Thu gọn thanh bên"} aria-expanded={!sidebarCollapsed} aria-controls="sidebar-navigation" onclick={toggleSidebar}>{#if sidebarCollapsed}<PanelLeftOpen size={18}/>{:else}<PanelLeftClose size={18}/>{/if}</button>
      </div>
      <button class="new-button" aria-label={tr.newMeeting} title={tr.newMeeting} onclick={newMeeting}
        ><Plus size={18} /> <span class="nav-label">{tr.newMeeting}</span></button
      >
      <button
        class="sidebar-nav-button"
        aria-label={captionNavLabel} title={captionNavLabel}
        class:chosen={page === "caption"}
        onclick={() => {
          if (testing) {
            error = tr.errorSettingsTesting;
            return;
          }
          page = "caption";
          taskMode = "captionsonly";
        }}><Captions size={18} /> <span class="nav-label">{captionNavLabel}</span></button
      >
      <button class="sidebar-nav-button" aria-label="Mẫu cấu hình" title="Mẫu cấu hình" class:chosen={page === "presets"} onclick={()=>{page="presets";presetEditing=false;presetDeleteId="";void tick().then(()=>document.querySelector(".preset-page")?.scrollTo({top:0}));}}><NotebookPen size={18}/> <span class="nav-label">Mẫu cấu hình</span></button>
      <button
        class="meetings-button"
        class:chosen={meetingsOpen}
        aria-label={tr.meetingListButton}
        title={`${tr.meetingListButton} · ${data.sessions.length}`}
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
        <button class="feedback-link" aria-label={tr.feedbackButton} title={tr.feedbackButton} onclick={() => openAuthor("feedback")}>
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
          aria-label={tr.settings} title={tr.settings}
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
          }}><Settings size={18} /> <span class="nav-label">{tr.settings}</span></button
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
              : page === "presets" ? "Mẫu cấu hình"
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
              : meeting?.provider === "groq" ? "Groq · dịch vụ ngoài"
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

      {#if page === "presets"}
        <section class="settings-page preset-page">
          <h1><NotebookPen size={22}/> Mẫu cấu hình</h1>
          <p class="subtitle">Chuẩn bị sẵn model và cách xử lý cho từng nhu cầu. API key được quản lý trong Cài đặt.</p>
          <div class="task-mode-tabs" role="tablist" aria-label="Loại mẫu cấu hình">
            <button role="tab" aria-selected={taskMode === "meeting"} class:active={taskMode === "meeting"} disabled={presetEditing} onclick={()=>taskMode="meeting"}>Cuộc họp</button>
            <button role="tab" aria-selected={taskMode === "captionsonly"} class:active={taskMode === "captionsonly"} disabled={presetEditing} onclick={()=>taskMode="captionsonly"}>Phụ đề</button>
          </div>
          {#if !presetEditing}
            <div class="preset-library-heading"><h2>{taskMode === "meeting" ? "Mẫu cho cuộc họp" : "Mẫu cho phụ đề"}</h2><button class="button primary" onclick={()=>editPreset()}><Plus size={16}/> Tạo mẫu</button></div>
            {#each workflowPresets.filter(p=>p.mode===taskMode) as preset (preset.id)}
              <div class="preset-row"><div><h3>{preset.name}</h3><p>{preset.source === "auto" ? "Tự nhận diện" : preset.source.toUpperCase()} · {preset.translate ? `Dịch sang ${preset.target.toUpperCase()}` : "Chỉ chép lời gốc"} · {modelLabel((preset.mode === "meeting" ? preset.tasks.transcription : preset.tasks.caption)?.profileId,(preset.mode === "meeting" ? preset.tasks.transcription : preset.tasks.caption)?.model??"Chưa chọn model")}</p></div><div class="preset-row-actions"><button class="button secondary small" disabled={!!data.active_id || captionRunning} onclick={()=>usePreset(preset)}>Sử dụng</button><button class="text-button" onclick={()=>editPreset(preset)}>Sửa</button><button class="text-button danger" onclick={()=>presetDeleteId=preset.id}>Xóa</button></div></div>
              {#if presetDeleteId===preset.id}<div class="notice warning" role="alert"><span>Xóa mẫu “{preset.name}”? API key và cuộc họp đã lưu vẫn được giữ.</span><button class="text-button danger" onclick={()=>removePreset(preset.id)}>Xóa mẫu</button><button class="text-button" onclick={()=>presetDeleteId=""}>Hủy</button></div>{/if}
            {:else}<p class="quality-hint">Chưa có mẫu. Tạo mẫu để không phải chọn lại từng model.</p>{/each}
          {:else}
            <div class="preset-editor-header"><h2>{presetEditingId ? "Sửa mẫu" : "Tạo mẫu"}</h2><p>Chỉ lưu khi bấm Lưu mẫu. Phiên đang chạy không bị ảnh hưởng.</p></div>
            <div class="settings-section preset-basics">
              <label class="ai-field">Tên mẫu<input aria-label="Tên mẫu" maxlength="80" bind:value={presetName}/></label>
              <label class="ai-field">Ngôn ngữ gốc<select aria-label="Ngôn ngữ gốc của mẫu" bind:value={presetSource}><option value="auto">Tự nhận diện</option><option value="vi">Tiếng Việt</option><option value="en">English</option><option value="zh">中文</option><option value="ko">한국어</option>{#if taskMode !== "meeting"}<option value="ja">日本語</option>{/if}</select></label>
              <label class="ai-field">Mức xử lý<select aria-label="Mức xử lý của mẫu" bind:value={presetQuality}><option value="fast">Nhanh</option><option value="balanced">Cân bằng</option><option value="accurate">Chính xác</option></select></label>
              {#if taskMode === "meeting"}<label class="checkbox"><input type="checkbox" bind:checked={presetCaptions}/> Bật phụ đề khi bắt đầu</label><label class="checkbox"><input type="checkbox" bind:checked={presetLocalFallback}/> Dự phòng local khi Soniox lỗi</label>{/if}
            </div>
          <div class="settings-section ai-settings">
            <div class="soniox-heading">
              <div>
                <h2>Model và tác vụ</h2>
                <p>Chọn model chép lời và phụ đề khi đang ghi. Hiệu chỉnh và tóm tắt được chọn riêng sau cuộc họp.</p>
              </div>
            </div>
            <label class="checkbox preset-translate"><input type="checkbox" aria-label="Dịch sang trong mẫu" bind:checked={presetTranslate}/> Dịch sang</label>
            {#if profileOf((taskMode === "meeting" ? presetAssignments.meeting?.transcription : presetAssignments.captionsonly?.caption)?.profileId,aiProfiles)?.provider === "groq"}
              <p class="quality-hint" role="status">Groq Whisper chép lời đa ngôn ngữ theo đoạn và tạo phụ đề lời gốc. Chọn model văn bản Groq khi bấm Tóm tắt sau cuộc họp.{#if presetTranslate} <button class="text-button" onclick={()=>presetTranslate=false}>Chỉ chép lời gốc</button>{/if}</p>
            {/if}
            <div class="task-assign-grid">
              {#if taskMode === "meeting"}
              <label class="ai-field"
                >{tr.taskTranscription}<select aria-label={tr.taskTranscription}
                  value={presetAssignments[taskMode]?.transcription?.profileId ?? ""}
                  disabled={busy}
                  onchange={(e) => {
                    const value = e.currentTarget.value;
                    if (!value) setPresetAssignment("transcription", undefined);
                    else
                      setPresetAssignment("transcription", {
                        ...presetAssignments[taskMode]?.transcription,
                        profileId: value,
                        model: value === "__local__" ? (compatibleLocalModels(presetSource==="auto"?"multi":presetSource)[0] ?? "moonshine-vi") : meetingSpeechModels(value,aiProfiles)[0],
                      });
                  }}
                  ><option value="">{tr.taskNone}</option><option
                    value="__local__">{tr.providerLocal}</option
                  >{#each speechProfiles(aiProfiles) as p}<option value={p.id}
                      >{p.name}</option
                    >{/each}</select
                ></label
              >
              {/if}
              <label class="ai-field"
                >{tr.taskCaption}<select aria-label={tr.taskCaption}
                  value={presetAssignments[taskMode]?.caption?.profileId ?? ""}
                  disabled={busy}
                  onchange={(e) => {
                    const value = e.currentTarget.value;
                    if (!value) setPresetAssignment("caption", undefined);
                    else
                      setPresetAssignment("caption", {
                        ...presetAssignments[taskMode]?.caption,
                        profileId: value,
                        model: selectableCaptionModels(value, aiProfiles, presetTranslate, presetSource)[0] ?? "",
                        translate: presetTranslate,
                        targetLang:
                          presetAssignments[taskMode]?.caption?.targetLang ?? "vi",
                      });
                  }}
                  ><option value="">{tr.taskNone}</option
                  >{#if !presetTranslate}<option value="__local__">Local trên máy · không cần API</option>{/if}{#each captionableProfiles(aiProfiles).filter(p=>(taskMode !== "meeting" || ["soniox","groq"].includes(p.provider)) && selectableCaptionModels(p.id,aiProfiles,presetTranslate,presetSource).length>0) as p}<option
                      value={p.id}>{p.name} · {p.provider}</option
                    >{/each}</select
                ></label
              >
              <label class="ai-field"
                >{tr.captionTargetLang}<select aria-label={tr.captionTargetLang}
                  value={presetAssignments[taskMode]?.caption?.targetLang ?? "vi"}
                  disabled={busy}
                  onchange={(e) =>
                    setPresetAssignment("caption", {
                      ...presetAssignments[taskMode]?.caption,
                      profileId:
                        presetAssignments[taskMode]?.caption?.profileId ?? "",
                      model: presetAssignments[taskMode]?.caption?.model ?? "",
                      targetLang: e.currentTarget.value,
                      translate: presetTranslate,
                    })}
                  ><option value="vi">{tr.langVi}</option><option value="en"
                    >{tr.langEn}</option
                  ><option value="ko">{tr.langKo}</option><option value="zh"
                    >{tr.langZh}</option
                  ><option value="ja">日本語</option></select
                ></label
              >
            </div>
            <div class="task-assign-grid task-model-grid">
              {#if taskMode === "meeting" && profileOf(presetAssignments.meeting?.transcription?.profileId,aiProfiles)?.provider === "groq"}
                <label class="ai-field">Chép lời · Model<select aria-label="Chép lời · Model" value={presetAssignments.meeting?.transcription?.model} onchange={e=>setPresetAssignment("transcription",{...presetAssignments.meeting!.transcription!,model:e.currentTarget.value})}>{#each groqSpeechModels as m}<option value={m}>{modelLabel(presetAssignments.meeting?.transcription?.profileId,m)}</option>{/each}</select></label>
              {/if}
              {#if presetAssignments[taskMode]?.caption?.profileId}
                <label class="ai-field"
                  >{tr.captionModel}<select
                    aria-label={tr.captionModel}
                    value={presetAssignments[taskMode]?.caption?.model ||
                      selectableCaptionModels(
                        presetAssignments[taskMode]?.caption?.profileId ?? "",
                        aiProfiles,
                        presetTranslate, presetSource,
                      )[0]}
                    onchange={(e) =>
                      setPresetAssignment("caption", {
                        ...presetAssignments[taskMode]?.caption!,
                        model: e.currentTarget.value,
                      })}
                  >
                    {#each selectableCaptionModels(presetAssignments[taskMode]?.caption?.profileId ?? "", aiProfiles, presetTranslate,presetSource) as m}<option
                        value={m}>{modelLabel(presetAssignments[taskMode]?.caption?.profileId,m)}</option
                      >{/each}
                  </select></label
                >
              {/if}

            </div>
            <div class="task-fallbacks">
              <h3>Dự phòng khi model lỗi hoặc hết hạn mức</h3>
              <p>Chỉ chuyển một lần sang model bạn chọn; không tự tìm nhà cung cấp khác. Cấu hình dự phòng có thể gửi dữ liệu sang nhà cung cấp khác và áp dụng hạn mức/chi phí của tài khoản đó. Lựa chọn được chốt khi bắt đầu phiên.</p>
              <div class="task-assign-grid">
                {#each (taskMode === "meeting" ? ["transcription","caption"] as const : ["caption"] as const) as task (task)}
                  <label class="ai-field">Dự phòng · {taskLabel(task)}
                    <select aria-label={`Dự phòng · ${taskLabel(task)}`} value={presetAssignments[taskMode]?.[task]?.fallback ? JSON.stringify(presetAssignments[taskMode]?.[task]?.fallback) : ""} disabled={busy || !presetAssignments[taskMode]?.[task]?.profileId || presetAssignments[taskMode]?.[task]?.profileId === "__local__"} onchange={e=>choosePresetFallback(task,e.currentTarget.value)}>
                      <option value="">Không dùng dự phòng</option>
                      {#each fallbackChoices(task,aiProfiles,presetAssignments[taskMode],taskMode,presetTranslate,presetSource) as choice}<option value={JSON.stringify(choice)}>{modelLabel(choice.profileId,choice.model)}</option>{/each}
                    </select>
                  </label>
                {/each}
              </div>
              {#if taskMode === "meeting"}<small>Chép lời: hỗ trợ Soniox chính → cấu hình Soniox dự phòng. Dự phòng local vẫn bật riêng khi tạo cuộc họp. Phụ đề dùng chung Soniox đi theo dự phòng chép lời.</small>{/if}
            </div>
            <p class="fine-print">{tr.taskAssignNote}</p>
          </div>

            {#if taskMode === "meeting" && presetAssignments[taskMode]?.transcription?.profileId === "__local__"}<label class="ai-field">Model chép lời local của mẫu<select aria-label="Model chép lời local của mẫu" value={presetAssignments[taskMode]?.transcription?.model} onchange={e=>setPresetAssignment("transcription",{...presetAssignments[taskMode].transcription!,model:e.currentTarget.value})}>{#each compatibleLocalModels(presetSource==="auto"?"multi":presetSource) as model}<option value={model}>{modelLabel("__local__",model)}</option>{/each}</select></label>{/if}
            <div class="settings-section preset-caption-style" id="caption-settings">
              <h2><Captions size={18}/> Hiển thị phụ đề của mẫu</h2>
              <p>Cỡ chữ, màu, nền và song ngữ được lưu cùng mẫu này. Chỉnh ở đây chưa thay đổi phiên đang chạy; chọn mẫu để áp dụng.</p>
              <CaptionControls bind:style={presetCaptionStyle} persist={false} visibilityControl={false} expanded={true}/>
            </div>
            {#if presetError}<div class="notice warning" role="alert">{presetError}</div>{/if}
            <div class="preset-save"><button class="button primary" onclick={savePreset}>Lưu mẫu</button><button class="button secondary" onclick={()=>{presetEditing=false;presetError="";}}>Hủy chỉnh sửa</button></div>
          {/if}
          <div class="settings-section" id="local-model-settings">
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

        </section>
      {:else if page === "settings"}
        <section class="settings-page">
          <h1>{tr.settingsHeading}</h1>
          <p class="subtitle">Quản lý kết nối AI, ngôn ngữ giao diện, cập nhật và dữ liệu. Mẫu sử dụng và kiểu hiển thị phụ đề nằm tại Mẫu cấu hình.</p>
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
          <MeetingStorage mode="settings" {desktop}/>
                  </section>
      {:else if page === "caption"}
        <section class="caption-page">
          <h1><Captions size={20} /> {tr.captionPage}</h1>
          {#if active}
          <p class="subtitle">Phụ đề đang dùng âm thanh và model của cuộc họp hiện tại.</p>
          <div class="settings-section caption-setup active-meeting-caption" aria-label="Phụ đề cuộc họp đang ghi">
            <h2>{active.title}</h2>
            <p class="quality-hint">{active.speech && !active.speech.local_fallback ? modelLabel(active.speech.profile_id ?? undefined,active.speech.model) : `Local/${active.model}`} · {active.status === "paused" ? "Cuộc họp đang tạm dừng" : "Cuộc họp đang ghi"}. Bản chép và bản dịch được lưu trong cuộc họp.</p>
            <div class="meeting-caption-options">
              <label class="checkbox"><input type="checkbox" aria-label="Dịch sang trong phiên đang ghi" checked={meetingTranslate} disabled={busy || !active.speech || active.speech.local_fallback || active.provider === "groq"} onchange={e=>{const input=e.currentTarget;void changeMeetingTranslation(input.checked).finally(()=>input.checked=meetingTranslate);}} /> Dịch sang</label>
              {#if meetingTranslate}<label class="ai-field">Ngôn ngữ bản dịch<select aria-label="Ngôn ngữ bản dịch phiên đang ghi" value={meetingTargetLang} disabled={busy} onchange={e=>{const input=e.currentTarget;void changeMeetingTranslation(true,input.value).finally(()=>input.value=meetingTargetLang);}}><option value="vi">Tiếng Việt</option><option value="en">English</option><option value="ko">한국어</option><option value="zh">中文</option><option value="ja">日本語</option></select></label>{/if}
            </div>
            <div class="caption-actions">
              <button class="button secondary" disabled={busy} onclick={()=>toggleMeetingCaption()}><Captions size={16}/>{captionRunning ? "Tắt phụ đề cuộc họp" : "Bật phụ đề cuộc họp"}</button>
              <CaptionWindowToggle />
              <button class="text-button" onclick={()=>choose(active!)}>Về cuộc họp đang ghi</button>
            </div>
            {#if captionError}<p class="test-error" role="alert">{captionError}</p>{/if}
            {#if captionText || (meetingTranslationPreview?.id === active.id && meetingTranslationPreview.text)}<div class="caption-preview"><div class="caption-preview-text">{meetingTranslationPreview?.id === active.id && meetingTranslate ? meetingTranslationPreview.text || captionText : captionText}</div></div>{/if}
          </div>
          {:else}
          <p class="subtitle">
            {tr.captionPageHint}
          </p>
          <div class="preset-launch"><label class="ai-field">Mẫu cấu hình phụ đề<select aria-label="Mẫu cấu hình phụ đề" value={pendingPresets.captionsonly} disabled={busy || !!data.active_id || captionRunning} onchange={e=>{pendingPresets={...pendingPresets,captionsonly:e.currentTarget.value};if(!e.currentTarget.value)selectedPresets={...selectedPresets,captionsonly:""};}}><option value="">Tùy chỉnh</option>{#each workflowPresets.filter(p=>p.mode==="captionsonly") as preset}<option value={preset.id}>{preset.name}</option>{/each}</select></label><button class="button primary" disabled={busy || !!data.active_id || captionRunning || !pendingPresets.captionsonly} onclick={()=>applyPendingPreset("captionsonly")}>Áp mẫu</button>{#if pendingPresets.captionsonly}<p aria-live="polite">{pendingPresets.captionsonly ? selectedPresets.captionsonly===pendingPresets.captionsonly ? "Đã áp dụng mẫu. Bạn có thể tùy chỉnh cho phiên này; mẫu gốc được giữ nguyên." : "Đã chọn mẫu. Bấm Áp mẫu để đưa thiết lập vào phiên này." : "Tùy chỉnh các mục bên dưới cho phiên này, hoặc chọn một mẫu đã lưu."}</p>{/if}<button class="text-button" disabled={!!data.active_id || captionRunning} onclick={()=>{taskMode="captionsonly";editPreset();page="presets";}}>Lưu thành mẫu</button></div>
          <div class="settings-section caption-setup" onchange={()=>markPresetCustom("captionsonly")}>
            <div class="caption-intent">
              <div class="caption-intent-choice">
                <label class="checkbox"><input type="checkbox" aria-label="Dịch sang" bind:checked={captionTranslate} disabled={busy || captionRunning || captionTesting} /> Dịch sang</label>
                {#if captionTranslate}<label class="caption-intent-target">{tr.captionTargetLang}<select aria-label={tr.captionTargetLang} bind:value={captionTargetLang} disabled={busy || captionRunning || captionTesting}><option value="vi">{tr.langVi}</option><option value="en">{tr.langEn}</option><option value="ko">{tr.langKo}</option><option value="zh">{tr.langZh}</option><option value="ja">日本語</option></select></label>{/if}
              </div>
              <p>{captionTranslate ? "Dịch âm thanh đang nghe sang ngôn ngữ đã chọn. Âm thanh được gửi lên nhà cung cấp." : "Hiện lời gốc từ loa/tai nghe. Chọn Local để xử lý trên máy, hoặc dùng cấu hình cloud."}</p>
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
                      selectableCaptionModels(captionProfileId, aiProfiles, captionTranslate,captionSourceLang)[0] ?? "";
                  }}
                >
                  <option value="">{tr.captionChooseProfile}</option>
                  {#if !captionTranslate}<option value="__local__">Local trên máy · không cần API</option>{/if}
                  {#each captionableProfiles(aiProfiles).filter(p=>selectableCaptionModels(p.id,aiProfiles,captionTranslate,captionSourceLang).length>0) as p}<option
                      value={p.id}>{p.name} · {p.provider}</option
                    >{/each}</select
                ></label
              >
              <label
                >{tr.captionModel}<select
                  bind:value={captionModel}
                  aria-label={tr.captionModel}
                  disabled={busy || captionRunning || captionTesting || !captionProfileId}
                  ><option value="" disabled>{tr.taskNone}</option>{#each captionProfileId === "__local__" && !captionTranslate ? ["moonshine-vi","moonshine-en",multilingualModel] : selectableCaptionModels(captionProfileId, aiProfiles, captionTranslate,captionSourceLang) as m}<option
                      value={m} disabled={captionProfileId === "__local__" && (!data.models.some(model=>model.id===m&&model.installed) || !captionModels(captionProfileId,aiProfiles,captionTranslate,captionSourceLang).includes(m))}>{modelLabel(captionProfileId,m)}{captionProfileId === "__local__" ? ` · ${data.models.some(model=>model.id===m&&model.installed) ? "Đã tải" : "Chưa tải"}${!captionModels(captionProfileId,aiProfiles,captionTranslate,captionSourceLang).includes(m) ? " · Không phù hợp ngôn ngữ gốc" : ""}` : ""}</option
                    >{/each}</select
                ></label
              >
            </div>
            <SpeechQualityControl compact scope="Phụ đề" locale={uiLocale??"vi"} value={captionSpeechQuality} provider={captionQualityProvider} model={captionModel} disabled={busy || captionRunning || captionTesting} onchange={q=>chooseProcessingLevel(captionLevelKey,q)} />
            {#if captionTranslate && profileOf(captionProfileId,aiProfiles)?.provider === "groq"}<p class="test-error" role="status">Groq Whisper hiện chỉ chép lời gốc trong Innote. Hãy tắt Dịch sang để dùng model này. <button class="text-button" onclick={()=>{captionTranslate=false;markPresetCustom("captionsonly");}}>Chỉ chép lời gốc</button></p>{/if}
            {#if captionProfileId && !(captionTranslate && profileOf(captionProfileId,aiProfiles)?.provider === "groq") && !captionModels(captionProfileId,aiProfiles,captionTranslate,captionSourceLang).includes(captionModel)}<p class="test-error">Model đã chọn không phù hợp chế độ hoặc ngôn ngữ gốc. Moonshine có model riêng cho Việt/Anh; Nemotron hỗ trợ đa ngôn ngữ. Hãy chọn lại model.</p>{/if}
            {#if captionProfileId === "__local__" && !captionLocalReady}<p class="test-error" role="status">{captionModel.startsWith("moonshine-") ? tr.modelMissingBundled : "Model chưa tải: mở Mẫu cấu hình → Model chép lời để tải trước."}</p>{/if}
            {#if captionProfileId === "__local__" || aiProfiles.find(p=>p.id===captionProfileId)?.provider === "groq"}<details class="workflow-help caption-engine-guide"><summary>Thông tin xử lý và hạn mức</summary>
            {#if captionProfileId === "__local__"}<p class="quality-hint">Chép trên máy, không cần API và không tự chuyển lên cloud. {captionModel === multilingualModel ? "Nemotron nhận audio liên tục, có chữ tạm theo lời nói; hỗ trợ Việt, Anh, Hàn và Trung. Tốc độ phụ thuộc CPU." : "Moonshine có sẵn trong Innote, chép streaming theo ngôn ngữ Việt/Anh của model đã chọn; không tự nhận diện đa ngôn ngữ. Tốc độ phụ thuộc CPU."} {captionLocalReady ? captionModel.startsWith("moonshine-") ? tr.modelBundled : "Model đã có trên máy." : ""}</p>{/if}
            {#if profileOf(captionProfileId,aiProfiles)?.provider === "groq"}<p class="quality-hint">Groq Whisper: độ dài đoạn và ngữ cảnh thay đổi theo Mức xử lý đã chọn; phần audio chồng lấn cũng dùng hạn mức API. Có thể trễ thêm do API, không có chữ tạm từng từ. Khi lỗi chỉ chuyển sang dự phòng bạn đã chọn, nếu có. Chép lời cuộc họp và phụ đề dùng riêng đều dùng Whisper theo đoạn.</p>{/if}
            </details>{/if}
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
                    captionPresetPending ||
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
          {/if}
          <section class="caption-inline-style" id="standalone-caption-style" aria-labelledby="standalone-caption-style-title">
            <h2 id="standalone-caption-style-title"><Settings size={16} /> Cài đặt chữ và hộp phụ đề</h2>
            <p class="quality-hint">Kiểu đang áp dụng cho phiên này. Dùng Lưu thành mẫu nếu muốn giữ các thay đổi trong một mẫu.</p>
            <CaptionControls visibilityControl={false} collapsible={false} onstylechange={()=>markPresetCustom(active?"meeting":"captionsonly")}/>
          </section>

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
                  >{:else if transcribe && meetingTranslate && (provider !== "soniox" || !transcriptionProfileId)}<button class="text-button setup-recovery" onclick={()=>document.getElementById("meeting-shared-model")?.focus()}>{tr.readyTranslationModel}</button>{:else}<span>{readyMessage}</span>{/if}
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
                    meetingPresetPending ||
                    !!data.active_id ||
                    !desktop ||
                    (!system && !microphone) ||
                    systemMissing ||
                    microphoneMissing ||
                    (transcribe && meetingTranslate && (provider !== "soniox" || !transcriptionProfileId)) ||
                    (transcribe && provider === "groq" && (!profileOf(transcriptionProfileId,aiProfiles)?.has_key || !groqSpeechModels.includes(transcriptionModel))) ||
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

          {#if meeting}<MeetingStorage mode="meeting" id={meeting.id} {desktop}/>{:else}<MeetingStorage mode="hint" {desktop} openSettings={()=>page="settings"}/>{/if}
          {#if active && selected === active.id && recording}<div class="meeting-caption-controls">
            <CaptionWindowToggle />
            <label class="checkbox"><input type="checkbox" aria-label="Dịch sang khi bật phụ đề" checked={meetingTranslate} onchange={e=>{const input=e.currentTarget;void changeMeetingTranslation(input.checked).finally(()=>input.checked=meetingTranslate);}} disabled={busy || !active.speech || active.speech.local_fallback || active.provider === "groq"} /> Dịch sang</label>
            {#if meetingTranslate}<label>Ngôn ngữ dịch đích<select aria-label="Ngôn ngữ dịch đích cuộc họp" value={meetingTargetLang} onchange={e=>{const input=e.currentTarget;void changeMeetingTranslation(true,input.value).finally(()=>input.value=meetingTargetLang);}} disabled={busy}><option value="vi">Tiếng Việt</option><option value="en">English</option><option value="ko">한국어</option><option value="zh">中文</option><option value="ja">日本語</option></select></label>{/if}
            <button class="text-button" aria-expanded={meetingStyleOpen} aria-controls="meeting-caption-style" onclick={()=>{meetingStyleOpen=true;void tick().then(()=>document.getElementById("meeting-caption-style")?.scrollIntoView({block:"nearest"}));}}><Settings size={15} /> Cài đặt phụ đề</button>
          </div>{/if}
          {#if active && selected === active.id && recording && meetingStyleOpen}<section class="session-caption-style" id="meeting-caption-style"><h3>Cấu hình hiển thị phụ đề của phiên</h3><CaptionControls visibilityControl={false} collapsible={false} onstylechange={()=>markPresetCustom("meeting")}/></section>{/if}
          {#if !meeting}
            <div class="preset-launch"><label class="ai-field">Mẫu cấu hình cuộc họp<select aria-label="Mẫu cấu hình cuộc họp" value={pendingPresets.meeting} disabled={busy || !!data.active_id || captionRunning} onchange={e=>{pendingPresets={...pendingPresets,meeting:e.currentTarget.value};if(!e.currentTarget.value)selectedPresets={...selectedPresets,meeting:""};}}><option value="">Tùy chỉnh</option>{#each workflowPresets.filter(p=>p.mode==="meeting") as preset}<option value={preset.id}>{preset.name}</option>{/each}</select></label><button class="button primary" disabled={busy || !!data.active_id || captionRunning || !pendingPresets.meeting} onclick={()=>applyPendingPreset("meeting")}>Áp mẫu</button>{#if pendingPresets.meeting}<p aria-live="polite">{pendingPresets.meeting ? selectedPresets.meeting===pendingPresets.meeting ? "Đã áp dụng mẫu. Bạn có thể tùy chỉnh cho phiên này; mẫu gốc được giữ nguyên." : "Đã chọn mẫu. Bấm Áp mẫu để đưa thiết lập vào phiên này." : "Tùy chỉnh các mục bên dưới cho phiên này, hoặc chọn một mẫu đã lưu."}</p>{/if}<button class="text-button" disabled={!!data.active_id || captionRunning} onclick={()=>{taskMode="meeting";editPreset();page="presets";}}>Lưu thành mẫu</button></div>
            <div class="setup-panel" onchange={()=>markPresetCustom("meeting")}>
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
                <button
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
                  <div class="provider-primary"><label for="transcription-provider">{tr.provider}</label>
                  <select
                    id="transcription-provider"
                    aria-label={tr.provider}
                    value={provider}
                    onchange={e=>changeMeetingProvider(e.currentTarget.value)}
                    disabled={busy}
                  >
                    <option value="local">{tr.providerLocal}</option>
                    <option value="soniox">{tr.providerSoniox}</option>
                    <option value="groq">Groq · Whisper · chép lời theo đoạn</option>
                  </select></div>
                  {#if provider === "local" || localFallback}
                    <label class="ai-field ai-field-full">{provider === "local" ? tr.localSpeechModel : tr.localFallbackModel}
                      <select aria-label={provider === "local" ? tr.localSpeechModel : tr.localFallbackModel} value={effectiveModel} disabled={busy} onchange={e=>chooseLocalMeetingModel(e.currentTarget.value)}>
                        {#each compatibleLocalModels(language) as id}<option value={id}>{modelLabel("__local__",id)} · {data.models.some(m=>m.id===id&&m.installed)?tr.installed:tr.downloadModel}</option>{/each}
                      </select>
                    </label>
                  {/if}
                  {#if provider === "groq"}
                    <label class="ai-field">Cấu hình Groq<select aria-label="Cấu hình Groq" bind:value={transcriptionProfileId} disabled={busy}><option value="">Chọn cấu hình Groq…</option>{#each aiProfiles.filter(p=>p.provider === "groq") as p}<option value={p.id}>{p.name} · groq</option>{/each}</select></label>
                    <label class="ai-field">Model chép lời Groq<select aria-label="Model chép lời Groq" bind:value={transcriptionModel} disabled={busy || !transcriptionProfileId}>{#each groqSpeechModels as m}<option value={m}>{modelLabel(transcriptionProfileId,m)}</option>{/each}</select></label>
                    <p class="quality-hint">Groq Whisper chép lời theo đoạn, gửi cùng âm thanh loa/micro đã chọn cho cuộc họp. Phụ đề dùng lại bản chép này; không mở luồng gửi audio thứ hai. Hạn mức theo tài khoản Groq Free. API dự phòng được chọn trong mẫu.</p>
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
                      <select aria-label="Model chép lời Soniox" disabled={busy}><option value={SONIOX_RT_MODEL}>{modelLabel(transcriptionProfileId,SONIOX_RT_MODEL)}</option></select>
                    </label>
                    <p class="quality-hint">Soniox lưu bản chép gốc. Khi bật phụ đề, dùng cùng luồng âm thanh; không chạy local song song.</p>
                    <label class="checkbox"><input type="checkbox" aria-label="Dự phòng local khi Soniox lỗi" bind:checked={localFallback} disabled={busy} /> Dự phòng local khi Soniox lỗi</label>
                    {#if localFallback}<p class="quality-hint">Chỉ chuyển local nếu Soniox lỗi. Khi đó tiếp tục chép lời, dịch tạm dừng. Moonshine Việt/Anh có sẵn; chỉ cần tải Nemotron nếu chọn đa ngôn ngữ/Hàn/Trung.</p>{/if}
                  {/if}
                </div>
              {/if}
              {#if transcribe}<SpeechQualityControl compact scope="Cuộc họp" locale={uiLocale??"vi"} value={meetingSpeechQuality} provider={provider} model={currentMeetingModel} disabled={busy} onchange={q=>chooseProcessingLevel(meetingLevelKey,q)} />{/if}
              {#if transcribe}<div class="meeting-caption-setup">
                <h2>Phụ đề &amp; bản dịch</h2>
                <div class="meeting-caption-switches"><label class="checkbox"><input type="checkbox" aria-label="Bật phụ đề khi bắt đầu cuộc họp" bind:checked={meetingCaptions} disabled={busy} /> Bật phụ đề khi bắt đầu cuộc họp</label>
                <div class="meeting-caption-options">
                  <label class="checkbox"><input type="checkbox" aria-label="Dịch sang trong cuộc họp" bind:checked={meetingTranslate} disabled={busy} /> Dịch sang</label>
                  {#if meetingTranslate}<label class="ai-field">Ngôn ngữ bản dịch<select aria-label="Ngôn ngữ bản dịch" bind:value={meetingTargetLang} disabled={busy}><option value="vi">Tiếng Việt</option><option value="en">English</option><option value="ko">한국어</option><option value="zh">中文</option><option value="ja">日本語</option></select></label>{/if}
                </div>
                </div>
                <p class="quality-hint">Có thể bật/tắt hộp khi đang ghi. Bản chép và bản dịch vẫn được lưu.</p>
                {#if provider === "groq" && meetingTranslate}<p class="test-error" role="status">Groq Whisper chỉ chép lời gốc trong Innote; hãy tắt Dịch sang hoặc chọn Soniox để dịch. <button class="text-button" onclick={()=>{meetingTranslate=false;markPresetCustom("meeting");}}>Chỉ chép lời gốc</button></p>{/if}
                {#if meetingCaptions || meetingTranslate}<div class="meeting-shared-model">
                  <label class="ai-field">{meetingTranslate ? "Model dùng chung chép lời & dịch phụ đề" : "Model dùng chung chép lời & phụ đề"}
                    <select id="meeting-shared-model" aria-label="Model dùng chung cho cuộc họp" value={meetingSharedChoice} disabled={busy} onchange={e=>chooseMeetingSharedModel(e.currentTarget.value)}>
                      <option value="" disabled>Chọn cấu hình/model dùng chung…</option>
                      {#if !meetingTranslate}{#each compatibleLocalModels(language) as id}<option value={JSON.stringify({profileId:"__local__",model:id})}>{modelLabel("__local__",id)}</option>{/each}{/if}
                      {#each speechProfiles(aiProfiles) as p}{#each meetingSpeechModels(p.id,aiProfiles) as m}<option value={JSON.stringify({profileId:p.id,model:m})}>{modelLabel(p.id,m)}{p.provider === "groq" ? " · Chỉ lời gốc" : ""}</option>{/each}{/each}
                    </select>
                  </label>
                  {#if provider === "soniox" && transcriptionProfileId}<p class="quality-hint">{modelLabel(transcriptionProfileId,SONIOX_RT_MODEL)} · Chép lời{meetingTranslate ? " và dịch" : ""} trên một kết nối. Âm thanh gửi lên Soniox.</p>
                  {:else if meetingTranslate}<p class="test-error" role="status">Hãy chọn Soniox ở đây; phần Cách chép lời sẽ đổi sang cùng cấu hình/model. Local không hỗ trợ dịch.</p>{/if}
                  {#if meetingTranslate}<details class="workflow-help"><summary>Khả năng dịch trong cuộc họp</summary><p>Soniox dùng chung chép lời và dịch; Groq Whisper dùng chung chép lời và phụ đề gốc theo đoạn. OpenAI/Gemini chưa dùng cho luồng chung cuộc họp này. Quản lý API key trong Cài đặt.</p></details>{/if}
                </div>{/if}
              <section class="meeting-caption-appearance" id="meeting-caption-style"><h3>Hiển thị phụ đề</h3><CaptionControls visibilityControl={false} collapsible={false} onstylechange={()=>markPresetCustom("meeting")}/></section>
              </div>{/if}
              {#if !transcribe}<p class="quality-hint">
                  {tr.transcribeOnly}
                </p>{/if}
              {#if transcribe && provider === "local"}<details class="workflow-help engine-guide"><summary>Thông tin xử lý âm thanh</summary>
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
              </details>{/if}
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
              {#if meeting?.corrected || meeting?.summary}
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
                  {#if meeting?.corrected}<button
                      type="button"
                      role="tab"
                      class="transcript-tab"
                      class:active={transcriptView === "corrected"}
                      aria-selected={transcriptView === "corrected"}
                      onclick={() => {
                        transcriptView = "corrected";translationView="original";
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
            {#if transcriptView !== "corrected" && (meeting?.translations?.length || meeting?.speech?.translation_language)}
              <label class="transcript-language-view">Nội dung hiển thị
                <select aria-label="Nội dung hiển thị" bind:value={translationView} onchange={()=>{transcriptView="original";transcriptTab="transcript";}}>
                  <option value="original">Bản gốc</option><option value="translation">Bản dịch</option><option value="bilingual">Song ngữ</option>
                </select>
              </label>
              {#if translationLanguages.length>1}<label class="transcript-language-view">Ngôn ngữ bản dịch<select aria-label="Ngôn ngữ bản dịch đã lưu" bind:value={translationLanguage}>{#each translationLanguages as lang}<option value={lang}>{lang}</option>{/each}</select></label>{/if}
            {/if}
            <div class="transcript-tools">
              {#if canCorrect}<button
                  class="button secondary small"
                  title={meeting?.corrected ? tr.smoothAgain : tr.smoothButton}
                  disabled={busy}
                  onclick={() => {
                    if(!correctionRunning){correctionProfileId="__local__";correctionModel=defaultCorrectionModel(meeting?.language??"");correctionFallback=undefined;correctionError="";}
                    correctionOpen = true;
                  }}
                  ><Sparkles size={14} />
                  {correctionRunning ? tr.smoothRunning : meeting?.corrected ? tr.smoothAgain : tr.smoothButton}</button
                >{/if}
              {#if canSummarize}<button
                  class="button secondary small"
                  title={meeting?.summary
                    ? tr.summaryRegenerate
                    : tr.summaryButton}
                  disabled={busy}
                  onclick={() => {
                    summaryProfileId="";summaryModel="";summaryFallback=undefined;summaryError="";summaryStyle="overview";summaryPrompt="";
                    summarySource=transcriptView === "corrected" || !meeting?.segments.length ? "corrected" : "original";
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
                  {#if meeting && (meeting.segments.length > 0 || !!meeting.corrected)}<button
                      class="button secondary small"
                      title={tr.copyTranscriptTitle}
                      disabled={busy}
                      onclick={() => copyTranscript()}
                      ><Copy size={14} /> {tr.copyTranscript}</button
                    >{/if}
                  {#if meeting.segments.length || meeting.corrected || showLive}<button
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
          {#if correctionRunning}<div class="notice" role="status"><span>{(correctionJob?.model??correctionModel).startsWith("moonshine-") || (correctionJob?.model??correctionModel)==="nemotron-3.5-q8" ? "Hiệu chỉnh bản thu trên máy" : "Hiệu chỉnh bản thu qua cloud"} · {correctionJob?.model??correctionModel} · {correctionJob?.stage??"Đang chuẩn bị"}</span><button class="text-button" onclick={()=>correctionOpen=true}>Xem tiến trình</button></div>{/if}
          <div class="workspace">
            {#if transcriptTab !== "summary" && transcriptView === "corrected" && meeting?.corrected}
              <div
                class="smooth-view"
                role="tabpanel"
                aria-label={tr.smoothHeading}
              >
                <div class="smooth-view-bar">
                  <span class="smooth-meta">
                    <Sparkles size={14} />
                    {fill(tr.smoothResultBy, {
                      provider: meeting.corrected.provider,
                      model: meeting.corrected.model,
                    })}
                  </span>
                  <div class="smooth-bar-actions">
                    <button
                      class="text-button"
                      title={tr.smoothCopy}
                      onclick={() => copyCorrected()}
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
                  {#if correctedSegments.length}
                    {#each correctedSegments as seg (seg.id)}
                      <article class="segment smooth-segment">
                        <button
                          type="button"
                          class="timestamp"
                          title={tr.playFromHere}
                          disabled={!audioUrl}
                          onclick={() => seek(seg.start,seg.end)}
                          >{clock(seg.start)}</button
                        >
                        <div class="segment-body">
                          <p>{seg.text}</p>
                        </div>
                      </article>
                    {/each}
                  {:else}
                    <p>{meeting.corrected.text}</p>
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
                        summarySource=transcriptView === "corrected" || !meeting?.segments.length ? "corrected" : "original";
                    summaryOpen = true;
                      }}>{tr.summaryRegenerate}</button
                    >
                  </div>
                {/if}
                <div class="summary-meta-bar">
                  <span class="summary-meta-text">
                    {fill(tr.summaryMeta, {
                      source:
                        meeting.summary.source === "corrected"
                          ? "Bản hiệu chỉnh"
                          : meeting.summary.source === "smoothed" ? "Bản biên tập cũ" : tr.summarySourceOriginal,
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
                        summarySource=transcriptView === "corrected" || !meeting?.segments.length ? "corrected" : "original";
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
                {#if continuousTranscript && (segments.length || (liveSegment && translationView !== "translation") || (translationView === "translation" && liveTranslation))}
                  <article class="recording-transcript" aria-label={tr.liveAria}>
                    <p>{flowingFinalText}{#if translationView === "translation" && liveTranslation}<span class="recording-partial" data-live-translation>{flowingFinalText ? " " : ""}{liveTranslation}</span>{:else if liveSegment && translationView !== "translation"}<span class="recording-partial" data-live-caption>{flowingFinalText ? " " : ""}{liveSegment.text}</span>{/if}</p>
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
                  {#if continuousTranscript}<p class="saved-translation">{savedTranslations.map(t=>t.text).join(" ")}{#if liveTranslation}<span class="recording-partial" data-live-translation>{savedTranslations.length ? " " : ""}{liveTranslation}</span>{/if}</p>
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
                      await invoke("open_exports_folder",{path:lastExport});
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
          {#if confProvider === "groq"}<p class="ai-key-note ai-field-full">Dùng endpoint mặc định của Groq: https://api.groq.com/openai/v1. Sau khi lưu, chọn model tại Mẫu cấu hình ở thanh bên.</p>{/if}
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
  {#if correctionOpen}
    <dialog class="confirm-dialog correction-dialog" bind:this={correctionDialog} aria-labelledby="correction-title" onclose={()=>correctionOpen=false} oncancel={()=>correctionOpen=false}>
      <h2 id="correction-title">Hiệu chỉnh từ file ghi âm</h2>
      <p>Model nghe lại bản thu để tạo <strong>Bản hiệu chỉnh</strong> có mốc thời gian riêng. Bản gốc và bản dịch được giữ nguyên.</p>
      <div class="summary-form">
      <label class="summary-field"><span>Cấu hình AI hiệu chỉnh</span><select aria-label="Cấu hình AI hiệu chỉnh" bind:value={correctionProfileId} disabled={correctionRunning} onchange={()=>{correctionModel="";correctionFallback=undefined;correctionError="";}}>
        <option value="">Chọn cấu hình…</option><option value="__local__">Local trên máy · không cần API</option>{#each correctionProfiles(aiProfiles) as p}<option value={p.id}>{p.name} · {p.provider}</option>{/each}
      </select></label>
      <label class="summary-field"><span>Model hiệu chỉnh</span><select aria-label="Model hiệu chỉnh" bind:value={correctionModel} onchange={()=>correctionFallback=undefined} disabled={correctionRunning||!correctionProfileId}>
        <option value="">Chọn model…</option>{#if correctionProfileId === "__local__" && correctionModel && !correctionModels(correctionProfileId,aiProfiles).includes(correctionModel)}<option value={correctionModel} disabled>{modelLabel(correctionProfileId,correctionModel)} · Chưa sẵn sàng</option>{/if}{#each correctionModels(correctionProfileId,aiProfiles) as m}<option value={m}>{modelLabel(correctionProfileId,m)}</option>{/each}
      </select></label>
      </div>
      {#if correctionProfileId === "__local__"}<p class="quality-hint">Nghe lại file trên máy, không cần API và không tự chuyển lên cloud. Chỉ dùng model đã tải, đúng ngôn ngữ bản thu. Dùng lại cùng model không bảo đảm chính xác hơn bản trực tiếp; tác vụ có thể dùng nhiều CPU. Mốc nghe lại của model local có thể chỉ gần đúng.</p>
      {#if !correctionModels("__local__",aiProfiles).length}<p class="test-error">Chưa có model local đã tải phù hợp ngôn ngữ bản thu. Tải model trong Mẫu cấu hình hoặc chọn cấu hình cloud.</p>{/if}
      {#if correctionModel && correctionModels("__local__",aiProfiles).length && !correctionModels("__local__",aiProfiles).includes(correctionModel)}<p class="test-error">Model local đã chọn chưa có trên máy hoặc không phù hợp ngôn ngữ bản thu. Chọn model khác hoặc tải model trong Mẫu cấu hình.</p>{/if}
      {:else}<p class="quality-hint">File ghi âm sẽ được gửi lên nhà cung cấp bạn chọn và dùng thêm hạn mức/phí API. Hiệu chỉnh nhận lại lời gốc, không dịch và không bảo đảm đúng 100%.</p>{/if}
      {#if correctionProfileId && correctionModel}<details class="action-options">
        <summary>Dự phòng (tùy chọn)</summary>
        <label class="summary-field"><span>Model hiệu chỉnh dự phòng</span><select aria-label="Model hiệu chỉnh dự phòng" disabled={correctionRunning} value={correctionFallback?JSON.stringify(correctionFallback):""} onchange={e=>correctionFallback=e.currentTarget.value?JSON.parse(e.currentTarget.value):undefined}>
          <option value="">Không dùng dự phòng</option>{#each fallbackChoices("correction",aiProfiles,{correction:{profileId:correctionProfileId,model:correctionModel}},"meeting",false) as choice}<option value={JSON.stringify(choice)}>{modelLabel(choice.profileId,choice.model)}</option>{/each}
        </select></label>
        <p class="quality-hint">Chỉ chuyển một lần sang model bạn chọn. Dự phòng cloud có thể gửi bản thu sang nhà cung cấp khác và dùng thêm hạn mức/phí API.</p>
      </details>{/if}
      {#if correctionModel === "stt-async-v6"}<p class="correction-meta">Hủy sẽ dừng nhận và lưu kết quả tại Innote; tác vụ Soniox đã nhận có thể vẫn xử lý và dùng hạn mức.</p>{/if}
      {#if !correctionProfiles(aiProfiles).length && !correctionModels("__local__",aiProfiles).length}<p class="test-error">Chưa có cấu hình nhận âm thanh. Thêm API trong Cài đặt hoặc tải model local phù hợp.</p>{/if}
      {#if correctionRunning}<p role="status" aria-live="polite">{correctionJob?.stage??"Đang bắt đầu hiệu chỉnh…"} · {correctionJob?.model??correctionModel}{#if correctionJob?.processed} · {clock(correctionJob.processed)} / {clock(correctionJob.total)}{/if}</p>{/if}
      {#if correctionError}<p class="test-error" role="alert">{correctionError}</p>{/if}
      <div><button class="button secondary" onclick={()=>correctionOpen=false}>{correctionRunning ? "Đóng, tiếp tục xử lý" : "Đóng"}</button>
        {#if correctionRunning}<button class="button secondary" disabled={correctionJob?.cancelling} onclick={()=>cancelCorrection()}>Hủy hiệu chỉnh</button>
        {:else}<button class="button primary" disabled={!canCorrect||!correctionModels(correctionProfileId,aiProfiles).includes(correctionModel)} onclick={()=>correctTranscript()}>{meeting?.corrected ? "Hiệu chỉnh lại" : "Bắt đầu hiệu chỉnh"}</button>{/if}
      </div>
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
      <p>Chọn cấu hình AI và model cho lần tóm tắt này. Model đọc bản chép bạn chọn; không gửi file ghi âm.</p>
      <div class="summary-form">
        <label class="summary-field"><span>Cấu hình AI tóm tắt</span><select aria-label="Cấu hình AI tóm tắt" bind:value={summaryProfileId} disabled={busy} onchange={()=>{summaryModel="";summaryFallback=undefined;summaryError="";}}>
          <option value="">Chọn cấu hình…</option>{#each chatProfiles(aiProfiles).filter(p=>p.has_key) as p}<option value={p.id}>{p.name} · {p.provider}</option>{/each}
        </select></label>
        <label class="summary-field"><span>Model tóm tắt</span>
          {#if profileOf(summaryProfileId,aiProfiles)?.provider === "openai-compatible"}<input aria-label="Model tóm tắt" placeholder="Nhập model ID" bind:value={summaryModel} disabled={busy} oninput={()=>summaryFallback=undefined}/>
          {:else}<select aria-label="Model tóm tắt" bind:value={summaryModel} disabled={busy||!summaryProfileId} onchange={()=>summaryFallback=undefined}>
            <option value="">Chọn model…</option>{#each summaryProfileId?textModels(summaryProfileId,aiProfiles):[] as m}<option value={m}>{modelLabel(summaryProfileId,m)}{m === "qwen/qwen3.8-27b" ? " · Preview" : ""}</option>{/each}
          </select>{/if}
        </label>
        <div class="summary-options-row">
          {#if meeting?.corrected}<label class="summary-field"><span>{tr.summarySource}</span><select bind:value={summarySource} disabled={busy} aria-label={tr.summarySource}>
            {#if meeting.segments.length}<option value="original">{tr.summarySourceOriginal}</option>{/if}<option value="corrected">Bản hiệu chỉnh</option>
          </select></label>{/if}
          <label class="summary-field"><span>{tr.summaryStyle}</span><select bind:value={summaryStyle} disabled={busy} aria-label={tr.summaryStyle}>
            <option value="overview">{tr.summaryStyleOverview}</option><option value="decisions">{tr.summaryStyleDecisions}</option><option value="full">{tr.summaryStyleFull}</option>
          </select></label>
        </div>
        <details class="action-options"><summary>Tùy chọn thêm</summary>
          <label class="summary-field"><span>{tr.summaryPromptLabel}</span><textarea bind:value={summaryPrompt} disabled={busy} placeholder={tr.summaryPromptPlaceholder} aria-label={tr.summaryPromptLabel} rows="3"></textarea></label>
          <label class="summary-field"><span>Model tóm tắt dự phòng</span><select aria-label="Model tóm tắt dự phòng" disabled={busy||!summaryReady} value={summaryFallback?JSON.stringify(summaryFallback):""} onchange={e=>summaryFallback=e.currentTarget.value?JSON.parse(e.currentTarget.value):undefined}>
            <option value="">Không dùng dự phòng</option>{#each fallbackChoices("summary",aiProfiles,{summary:{profileId:summaryProfileId,model:summaryModel}},"meeting",false) as choice}<option value={JSON.stringify(choice)}>{modelLabel(choice.profileId,choice.model)}</option>{/each}
          </select></label>
          <p class="quality-hint">Chỉ chuyển một lần sang model bạn chọn; bản chép có thể được gửi sang nhà cung cấp dự phòng và dùng hạn mức/phí của tài khoản đó.</p>
        </details>
        {#if !chatProfiles(aiProfiles).some(p=>p.has_key)}<p class="test-error">Chưa có cấu hình AI hỗ trợ tóm tắt. Thêm và kiểm tra API trong Cài đặt.</p><button class="text-button" disabled={busy} onclick={()=>{summaryOpen=false;page="settings";}}>{tr.aiOpenSettings}</button>{/if}
        <p class="quality-hint">Bản chép sẽ được gửi lên nhà cung cấp bạn chọn và dùng hạn mức/phí API.</p>
        {#if summaryError}<p class="test-error" role="alert">{summaryError}</p>{/if}
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
          disabled={busy || !summaryReady}
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
