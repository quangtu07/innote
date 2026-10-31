<script lang="ts">
  import { onMount, tick } from "svelte";
  import { version as appVersion } from "../package.json";
  import { invoke, convertFileSrc } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { check, type Update } from "@tauri-apps/plugin-updater";
  import type { Segment } from "./types";
  import Logo from "./Logo.svelte";
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
  const model = "phowhisper-small";
  let sourcesInitialized = false;
  let transcribe = true;
  let provider = "local";
  let sonioxKey = "";
  type SmoothProvider = "openai" | "deepseek" | "gemini" | "openai-compatible";
  const smoothDefaults: Record<SmoothProvider, string> = {
    openai: "gpt-4o-mini",
    deepseek: "deepseek-chat",
    gemini: "gemini-2.5-flash",
    "openai-compatible": "",
  };
  const isSmoothProvider = (value: unknown): value is SmoothProvider =>
    ["openai", "deepseek", "gemini", "openai-compatible"].includes(String(value));
  let smoothProvider: SmoothProvider = "openai";
  let smoothModel = smoothDefaults.openai;
  let smoothBaseUrl = "";
  let smoothKey = "";
  let smoothPrompt = "";
  let smoothPanelOpen = false;
  let smoothOpen = false;
  let smoothDialog: HTMLDialogElement;
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
  let exportVersion: "original" | "smoothed" | "summary" = "original";
  let refining = false;
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
      if (modelPickerDialog && !modelPickerDialog.open) modelPickerDialog.showModal();
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
  $: llmConfigSignature = `${smoothProvider}|${smoothModel}|${smoothBaseUrl}`;
  $: llmConnected = llmConfigSignature === llmConnectionSignature;
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
        ? meeting.smoothed?.text ?? ""
        : meeting.segments.map((s) => s.text).join("\n");
    return summaryHash(source);
  })();
  $: summaryStale = !!meeting?.summary?.source_hash && summaryCurrentHash !== meeting.summary.source_hash;
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
  const multilingualModel="nemotron-3.5-q8";
  const authorUrls = {
    zalo: "https://zalo.me/0962239818",
    linkedin: "https://www.linkedin.com/in/alex-linh-790794237/",
    website: "https://alexlinh.io.vn/",
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
  $: multilingual=["multi","ko","zh"].includes(language);
  $: effectiveModel=multilingual?multilingualModel:model;
  $: currentModel = data.models.find((m) => m.id === effectiveModel);
  $: if (multilingual && provider!=="local") provider="local";
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
        : transcribe && !currentModel?.installed
          ? tr.readyDownload
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
  let liveUpdates: Record<string, { words: Segment[]; partial: string; partialWords?: Segment[]; end: number; unavailable?: boolean }> = {};
  $: currentLive = meeting ? liveUpdates[meeting.id] : undefined;
  $: liveWords = meeting ? Array.from(new Map([...(meeting.live ?? []), ...(currentLive?.words ?? [])].map(w => [w.id, w])).values()).filter(w => w.end > meeting!.processed + 0.00001).sort((a,b) => a.start - b.start) : [];
  $: liveText = liveWords.map(w => w.text).join(" ");
  $: partialText = meeting && currentLive && currentLive.end > meeting.processed && ["recording", "paused"].includes(meeting.status) ? currentLive.partial.split(/\s+/).slice((currentLive.partialWords ?? []).filter(w => w.end <= meeting!.processed + 0.00001).length).join(" ") : "";
  $: refinementPending = !!meeting && (refining || (["recording", "paused", "processing"].includes(meeting.status) && (meeting.segments.length > 0 || !!(liveText || partialText))));
  $: showLive = !!meeting && ["recording", "paused", "processing", "interrupted"].includes(meeting.status) && !!(liveText || partialText) && (!searchText || `${liveText} ${partialText}`.toLocaleLowerCase("vi").includes(searchText.toLocaleLowerCase("vi")));
  $: liveSegment = meeting && showLive
    ? {
        id: "live-preview",
        start: liveWords[0]?.start ?? meeting.processed,
        end: Math.max(meeting.processed, currentLive?.end ?? 0, liveWords[liveWords.length - 1]?.end ?? 0),
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
  $: if (liveText || partialText) void followLive(liveText, partialText, autoScroll);
  $: segments =
    meeting?.segments.filter((s) =>
      s.text
        .toLocaleLowerCase("vi")
        .includes(searchText.toLocaleLowerCase("vi")),
    ) ?? [];
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
  $: emptyHeading =
    searchText
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
  $: emptyDescription =
    searchText
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
    if (!desktop || updateChecking || updatePhase === "downloading" || updatePhase === "installing") return;
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
    if (!updateInfo || updatePhase === "downloading" || updatePhase === "installing") return;
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
              ? Math.min(100, Math.round((updateDownloaded / updateTotalBytes) * 100))
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
    smoothPanelOpen = false;
    smoothOpen = false;
    smoothPrompt = "";
    exportVersion = "original";
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
    provider = "local";
    smoothPanelOpen = false;
    smoothOpen = false;
    smoothPrompt = "";
    exportVersion = "original";
    meetingsOpen = false;
    renamingId = "";
    summaryOpen = false;
    transcriptTab = "transcript";
  }
  async function start() {
    await action(async () => {
      const s = await invoke<Meeting>("start", {
        options: {
          title,
          system: system || null,
          microphone: microphone || null,
          language,
          model: effectiveModel,
          transcribe,
          provider,
        },
      });
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
  async function saveSonioxKey(clear = false) {
    await action(async () => {
      try {
        await invoke("set_soniox_key", { key: clear ? "" : sonioxKey });
      } finally {
        sonioxKey = "";
      }
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
    if (!smoothKey.trim() && !data.llm_configured) {
      error = tr.aiKeyRequired;
      return;
    }
    await action(async () => {
      await invoke("test_llm_connection", {
        provider: smoothProvider,
        model: smoothModel,
        baseUrl: smoothBaseUrl,
        key: smoothKey.trim() || null,
      });
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
  async function refineMeeting() {
    if (!meeting || refining || busy) return;
    const id = meeting.id;
    refining = true;
    try {
      const updated = await invoke<Meeting>("refine_transcript", { id });
      if (selected === id) {
        data = {
          ...data,
          sessions: data.sessions.map((s) => (s.id === id ? updated : s)),
        };
        notify(tr.notifyRefined);
      }
    } catch (e) {
      error = String(e);
    } finally {
      refining = false;
    }
  }
  async function smoothTranscript() {
    if (!meeting || !canSmooth) return;
    if (!llmConnected) {
      error = tr.aiConnectFirst;
      return;
    }
    const id = meeting.id;
    await action(async () => {
      await invoke("test_llm_connection", {
        provider: smoothProvider,
        model: smoothModel,
        baseUrl: smoothBaseUrl,
        key: smoothKey.trim() || null,
      });
      llmConnectionSignature = llmConfigSignature;
      const updated = await invoke<Meeting>("smooth_transcript", {
        id,
        provider: smoothProvider,
        model: smoothModel,
        baseUrl: smoothBaseUrl,
        prompt: smoothPrompt,
        key: smoothKey.trim() || null,
      });
      if (id === meeting?.id) {
        data = {
          ...data,
          sessions: data.sessions.map((s) => (s.id === id ? updated : s)),
        };
        smoothPanelOpen = true;
        smoothOpen = false;
        smoothPrompt = "";
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
    await action(async () => {
      await invoke("test_llm_connection", {
        provider: smoothProvider,
        model: smoothModel,
        baseUrl: smoothBaseUrl,
        key: smoothKey.trim() || null,
      });
      llmConnectionSignature = llmConfigSignature;
      const updated = await invoke<Meeting>("summarize_transcript", {
        id,
        provider: smoothProvider,
        model: smoothModel,
        baseUrl: smoothBaseUrl,
        prompt: summaryPrompt,
        style: summaryStyle,
        source: summarySource,
        key: smoothKey.trim() || null,
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
    const text = meeting.segments
      .map((s) => s.text.trim())
      .filter(Boolean)
      .join("\n");
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
        version: exportVersion,
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
  async function seek(seconds: number) {
    if (audio) {
      audio.currentTime = seconds;
      await audio.play().catch((e) => {
        error = String(e);
      });
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
    tick().then(() =>
      document.getElementById(`rename-${s.id}`)?.focus(),
    );
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
          (result.peaks.system >= 0.001 ? heard : quiet).push(
            tr.audioMeeting,
          );
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
      if (!testUrl)
        testLabel = tr.testNoRecording;
    } finally {
      clearInterval(countdown);
      testing = false;
    }
  }
  async function loadAudio(id: string) {
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
        listen<{id: string; audit_epoch_unix_ms?: number; audit_emit_ms?: number; unavailable?: boolean; update?: {text:string; start:number; end:number; speech_end?: number | null; final_text:boolean; words:Segment[]}}>("live-transcript", (e) => {
          const receivedAt=Date.now();
          const {id, update, unavailable} = e.payload;
          const old = liveUpdates[id] ?? {words: [], partial: "", end: 0};
          liveUpdates = {...liveUpdates, [id]: unavailable ? {...old, partial:"", unavailable:true} : update ? {
            words: [...old.words.filter(w => w.end > (data.sessions.find(s => s.id === id)?.processed ?? 0)), ...(update.final_text ? update.words : [])],
            partial: update.final_text ? "" : update.text,
            partialWords: update.final_text ? [] : update.words,
            end: update.end,
          } : old};
          const epoch=e.payload.audit_epoch_unix_ms;
          if (typeof epoch === "number" && Number.isFinite(epoch)) {
            void (async () => {
              await tick();
              // Two frame callbacks observe the previous frame after it could paint.
              await new Promise<void>(resolve=>requestAnimationFrame(()=>requestAnimationFrame(()=>resolve())));
              if (meeting?.id !== id || page !== "meeting" || document.visibilityState !== "visible") return;
              const text=document.querySelector("[data-live-caption]")?.textContent?.trim();
              if (text) await invoke("report_live_paint",{id,text,elapsedMs:Date.now()-epoch,receivedMs:receivedAt-epoch,emittedMs:e.payload.audit_emit_ms??receivedAt-epoch,audioMs:(update?.end??0)*1000,speechEndMs:typeof update?.speech_end==="number"?update.speech_end*1000:null,finalText:!!update?.final_text});
            })().catch(()=>{/* Optional local QA must never interrupt recording. */});
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
  <aside class="sidebar" aria-label={tr.navAria}>
    <button
      class="brand"
      style="background:transparent;padding:0;text-align:left"
      onclick={newMeeting}
      ><span class="brand-mark"><Logo size={24} /></span
      ><span>Innote<span class="brand-sub">{tr.appTagline}</span></span
      ></button
    >
    <button class="new-button" onclick={newMeeting}
      ><Plus size={18} /> {tr.newMeeting}</button
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
        }}><Settings size={18} /> {tr.settings}</button
      ><span><span class="local-dot"></span> {tr.dataOnDevice}</span>
    </div>
  </aside>

  <main>
    <header class="topbar">
      <div class="breadcrumb">
        {page === "settings"
          ? tr.settings
          : meeting
            ? tr.yourMeeting
            : tr.meetingSpace}
      </div>
      <div class="local-label">
        <CheckCheck size={16} />
        {meeting?.provider === "soniox"
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
          <button class="text-button" onclick={dismissUpdate}>{tr.updateLater}</button>
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
                <option value={value}
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
            disabled={updateChecking || updatePhase === "downloading" || updatePhase === "installing"}
            onclick={() => void checkForUpdate(true)}
          >
            <RefreshCw size={17} />
            {updateChecking ? tr.updateChecking : tr.checkUpdates}
          </button>
        </div>
        <div class="settings-section">
          <h2>{tr.sonioxTitle}</h2>
          <p>{tr.sonioxDesc}</p>
          <label
            >{tr.sonioxKey}<input
              type="password"
              aria-label={tr.sonioxKey}
              autocomplete="off"
              spellcheck="false"
              bind:value={sonioxKey}
              placeholder={tr.optional}
            /></label
          >
          <div class="soniox-actions">
            <button
              class="button secondary"
              disabled={busy || !sonioxKey.trim()}
              onclick={() => saveSonioxKey()}>{tr.keepKey}</button
            >
            <button
              class="text-button"
              disabled={busy || !data.soniox_configured}
              onclick={() => saveSonioxKey(true)}>{tr.removeKey}</button
            >
          </div>
          <p>
            {data.soniox_configured
              ? tr.sonioxConfigured
              : tr.sonioxMissing} {tr.sonioxKeyNote}
          </p>
        </div>
        <div class="settings-section ai-settings">
          <div class="ai-heading">
            <div>
              <h2><Sparkles size={18} /> {tr.aiTitle}</h2>
              <p>{tr.aiDesc}</p>
            </div>
            <span class:connected={llmConnected} class="ai-status"
              >{llmConnected ? tr.aiConnected : tr.aiNotConnected}</span
            >
          </div>
          <div class="ai-grid">
            <label class="ai-field"
              >{tr.aiProvider}<select
                aria-label={tr.aiProvider}
                value={smoothProvider}
                disabled={busy}
                onchange={(event) => {
                  const value = event.currentTarget.value as SmoothProvider;
                  smoothProvider = value;
                  smoothModel = smoothDefaults[value];
                  smoothModels = [];
                  modelPickerOpen = false;
                  modelPickerSearch = "";
                  llmConnectionSignature = "";
                }}
              >
                <option value="openai">{tr.aiProviderOpenai}</option>
                <option value="deepseek">{tr.aiProviderDeepseek}</option>
                <option value="gemini">{tr.aiProviderGemini}</option>
                <option value="openai-compatible">{tr.aiProviderCompatible}</option>
              </select></label
            >
            <label class="ai-field"
              >{tr.aiModel}<div class="ai-model-row">
                <input
                  bind:value={smoothModel}
                  aria-label={tr.aiModel}
                  placeholder={smoothDefaults[smoothProvider]}
                  autocomplete="off"
                  spellcheck="false"
                  oninput={() => {
                    llmConnectionSignature = "";
                  }}
                />
                {#if smoothModels.length > 0}
                  <button
                    type="button"
                    class="button secondary small"
                    title={tr.modelPickerOpen}
                    onclick={() => {
                      modelPickerSearch = "";
                      modelPickerOpen = true;
                    }}
                  >
                    <List size={14} /> {tr.modelPickerOpen}
                  </button>
                {/if}
                <button
                  type="button"
                  class="button secondary small"
                  disabled={busy || (!smoothKey.trim() && !data.llm_configured)}
                  onclick={() => fetchModels()}
                  >{busy ? tr.aiFetchingModels : tr.aiFetchModels}</button
                >
              </div><span class="ai-hint">{tr.aiModelHint}</span></label
            >
            {#if smoothProvider === "openai-compatible"}<label class="ai-field ai-field-full"
                >{tr.aiBaseUrl}<input
                  type="text"
                  bind:value={smoothBaseUrl}
                  aria-label={tr.aiBaseUrl}
                  placeholder={tr.aiBaseUrlHint}
                  autocomplete="off"
                  spellcheck="false"
                  oninput={() => {
                    llmConnectionSignature = "";
                  }}
                /></label
              >{/if}
          </div>
          <div class="ai-key-row">
            <label class="ai-field"
              >{tr.aiKey}<div class="ai-key-input-wrap">
                <input
                  type="password"
                  aria-label={tr.aiKey}
                  autocomplete="off"
                  spellcheck="false"
                  bind:value={smoothKey}
                  placeholder={tr.aiKeyPlaceholder}
                  oninput={() => {
                    llmConnectionSignature = "";
                  }}
                /><button
                  class="button secondary"
                  disabled={busy || (!smoothKey.trim() && !data.llm_configured)}
                  onclick={() => testLlmConnection()}>{tr.aiTestConnection}</button
                >
              </div></label
            >
          </div>
          <p class="ai-hint">{tr.aiTestBeforeSave}</p>
          <div class="ai-actions">
            <button
              class="button primary"
              disabled={busy || !smoothKey.trim() || !llmConnected}
              onclick={() => saveLlmKey()}>{tr.aiKeepKey}</button
            >
            <button
              class="text-button"
              disabled={busy || !data.llm_configured}
              onclick={() => saveLlmKey(true)}>{tr.aiClearKey}</button
            >
            <span class="ai-key-state"
              >{data.llm_configured ? tr.aiConfigured : tr.aiMissing}</span
            >
          </div>
          <p class="ai-key-note">{tr.aiKeyNote}</p>
        </div>
        <div class="settings-section">
          <h2>{tr.modelTitle}</h2>
          <p>{tr.modelDesc}</p>
          {#each data.models.filter(m=>m.id==="phowhisper-small"||m.id==="large-v3-turbo-q5_0"||m.id===multilingualModel) as m}<div class="model-row">
              <div class="model-symbol"><FileText size={21} /></div>
              <div>
                <strong>{m.name}</strong>
                <p>
                  {Math.round(m.bytes / 1000000)} MB · {m.id===multilingualModel?tr.modelLangsMulti:m.id==="phowhisper-small"?tr.modelLangsPhoWhisper:tr.modelLangsViEn} · {tr.onDevice}
                </p>
              </div>
              {#if m.installed}<span class="installed"
                  ><Check size={16} /> {tr.installed}</span
                >{:else}<button
                  class="button secondary"
                  disabled={!!progress || busy}
                  onclick={() => {
                    if (m.id===multilingualModel) language="multi";
                    void download(m.id);
                  }}><Download size={16} /> {tr.downloadModel}</button
                >{/if}
            </div>{/each}
          {#if progress}<div class="download-progress">
              <div>
                <span>{tr.downloadingModel}</span><span
                  >{Math.round((progress.bytes / progress.total) * 100)}%</span
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
                  ><span></span>{states[meeting.status] ?? meeting.status}</span
                ><span class="time">{clock(meeting.duration)}</span>{:else}<span
                  >{readyMessage}</span
                >{/if}
            </div>
          </div>
          <div class="record-controls">
            {#if active && selected === active.id && recording}<button
                class="button secondary"
                disabled={busy}
                onclick={togglePause}
                >{#if active.status === "paused"}<Play size={17} /> {tr.continue}{:else}<Pause
                    size={17}
                  /> {tr.pause}{/if}</button
              ><button class="button stop" disabled={busy} onclick={finish}
                ><Square size={15} fill="currentColor" /> {tr.stop}</button
              >
            {:else if active && selected === active.id && active.status === "processing"}<span class="processing-label"
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
                  (transcribe && !currentModel?.installed)}
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
                  ><option value="">{tr.noCapture}</option>{#if systemMissing}<option
                      value={system}
                      disabled>{tr.deviceMissing}</option
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
                <summary>{fill(tr.testWarnings, { count: testWarnings.length })}</summary
                >{#each groupedTestWarnings as warning}<p>
                    {warning.message}{warning.count > 1
                      ? fill(tr.times, { count: warning.count })
                      : ""}
                  </p>{/each}
              </details>{/if}
            {#if systemMissing || microphoneMissing}<p
                class="source-warning"
                role="status"
              >{tr.sourceUnavailable}</p>{/if}
            <div class="preferences">
              <label
                >{tr.transcriptionLanguage}<select aria-label={tr.transcriptionLanguage} bind:value={language} disabled={busy}
                  ><option value="vi">{tr.langVi}</option><option value="en"
                    >{tr.langEn}</option><option value="ko">{tr.langKo}</option><option value="zh">{tr.langZh}</option><option value="multi">{tr.langMulti}</option
                  ></select
                ></label
              >{#if multilingual}<span class="fixed-model">{tr.fixedModel}</span>{/if}
              <label class="checkbox"
                ><input type="checkbox" bind:checked={transcribe} disabled={busy} /> {tr.transcribe}</label
              >
            </div>
            {#if transcribe}
              <div class="provider-choice">
                <label for="transcription-provider">{tr.provider}</label>
                <select
                  id="transcription-provider"
                  aria-label={tr.provider}
                  bind:value={provider}
                  disabled={multilingual || busy}
                >
                  <option value="local">{tr.providerLocal}</option
                  >
                  <option value="soniox">{tr.providerSoniox}</option>
                </select>
                {#if provider === "soniox"}<p class="quality-hint">
                    {tr.providerSonioxDesc} {data.soniox_configured
                      ? tr.providerOnFailure
                      : tr.providerNoKey}
                  </p>{/if}
              </div>
            {/if}
            {#if !transcribe}<p class="quality-hint">{tr.transcribeOnly}</p>{/if}
            {#if multilingual}<p class="quality-hint">{language==="multi"?tr.multiHint:language==="ko"?tr.koHint:tr.zhHint} {language==="multi"?"":tr.multiSwitchHint}</p>{/if}
            {#if !multilingual && transcribe}<p class="quality-hint">{tr.turboHint}</p>{/if}
            {#if transcribe && !currentModel?.installed}<div
                class="model-download"
              >
                <div>
                  <Download size={18} /><span
                    >{#if progress}{tr.downloading} {Math.round(
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
                    : fill(tr.downloadSize, { size: Math.round((currentModel?.bytes ?? (multilingual?741548352:574041195)) / 1000000) })}</button
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
              ><Headphones size={15} /> {tr.systemAudio}
              <span class="level"
                ><span style:width={`${systemLevel}%`}></span></span
              ></span
            ><span
              ><Mic size={15} /> {tr.mic}
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
            <summary>{fill(tr.sessionWarnings, { count: meeting.warnings.length })}</summary
            >{#each meeting.warnings as warning}<p>{warning}</p>{/each}
          </details>{/if}

        <div class="transcript-heading">
          <div>
            {#if meeting?.summary}
              <div class="transcript-tabs" role="tablist">
                <button
                  type="button"
                  role="tab"
                  class="transcript-tab"
                  class:active={transcriptTab === "transcript"}
                  aria-selected={transcriptTab === "transcript"}
                  onclick={() => {
                    transcriptTab = "transcript";
                  }}>{tr.summaryTabTranscript}</button
                >
                <button
                  type="button"
                  role="tab"
                  class="transcript-tab"
                  class:active={transcriptTab === "summary"}
                  aria-selected={transcriptTab === "summary"}
                  onclick={() => {
                    transcriptTab = "summary";
                  }}
                  ><ClipboardList size={14} /> {tr.summaryTabSummary}{#if summaryStale}<span
                      class="tab-dot"
                      title={tr.summaryStale}
                    ></span>{/if}</button
                >
              </div>
            {:else}
              <h2>{tr.transcript}</h2>
            {/if}
            {#if refinementPending}<span class="refining-note" role="status"
                >{tr.exportWaitRefinement}</span
              >{/if}
            {#if meeting && ["recording", "paused", "processing"].includes(meeting.status)}<span
                >{fill(tr.processedProgress, { status: meeting?.model===multilingualModel?tr.processedCommitted:tr.processedCorrected, processed: clock(meeting.processed), duration: clock(meeting.duration) })}</span
              >{/if}
          </div>
          <div class="transcript-tools">
            {#if meeting && !["recording", "paused", "processing"].includes(meeting.status) && meeting.segments.length > 0}
              <button
                class="button secondary small"
                title={tr.refineButtonTitle}
                disabled={busy || refining}
                onclick={() => refineMeeting()}
              >
                {#if refining}
                  <RefreshCw size={14} class="spin" /> {tr.refining}
                {:else}
                  <RefreshCw size={14} /> {tr.refineButton}
                {/if}
              </button>
            {/if}
            {#if canSmooth}<button
              class="button secondary small"
              title={meeting?.smoothed ? tr.smoothAgain : tr.smoothButton}
              disabled={busy || refining}
              onclick={() => {
                smoothOpen = true;
              }}><Sparkles size={14} /> {meeting?.smoothed ? tr.smoothAgain : tr.smoothButton}</button
            >{/if}
            {#if canSmooth}<button
              class="button secondary small"
              title={meeting?.summary ? tr.summaryRegenerate : tr.summaryButton}
              disabled={busy || refining}
              onclick={() => {
                summaryOpen = true;
              }}><ClipboardList size={14} /> {meeting?.summary ? tr.summaryRegenerate : tr.summaryButton}</button
            >{/if}
            {#if meeting && (meeting.segments.length || showLive || audioUrl)}<div class="export-controls compact">
                {#if meeting?.segments.length > 0}<button
                  class="button secondary small"
                  title={tr.copyTranscriptTitle}
                  disabled={busy}
                  onclick={() => copyTranscript()}><Copy size={14} /> {tr.copyTranscript}</button
                >{/if}
                {#if meeting?.smoothed || meeting?.summary}<div
                  class="export-version"
                  role="group"
                  aria-label={tr.exportVersion}
                >
                  <button
                    class:active={exportVersion === "original"}
                    onclick={() => {
                      exportVersion = "original";
                    }}>{tr.exportOriginal}</button
                  >
                  {#if meeting?.smoothed}<button
                    class:active={exportVersion === "smoothed"}
                    onclick={() => {
                      exportVersion = "smoothed";
                    }}>{tr.exportSmoothed}</button
                  >{/if}
                  {#if meeting?.summary}<button
                    class:active={exportVersion === "summary"}
                    onclick={() => {
                      exportVersion = "summary";
                    }}>{tr.exportSummary}</button
                  >{/if}
                </div>{/if}
                {#if meeting.segments.length || showLive}<button
                  class="button secondary small"
                  title={refinementPending ? tr.exportWaitRefinement : tr.exportMdTitle}
                  disabled={busy || refinementPending}
                  onclick={() => exportText("md")}><Download size={14} /> {tr.exportMd}</button
                ><button
                  class="button secondary small"
                  title={refinementPending ? tr.exportWaitRefinement : tr.exportTxtTitle}
                  disabled={busy || refinementPending}
                  onclick={() => exportText("txt")}><Download size={14} /> {tr.exportTxt}</button
                ><button
                  class="button secondary small"
                  title={refinementPending ? tr.exportWaitRefinement : tr.exportSrtTitle}
                  disabled={busy || refinementPending}
                  onclick={() => exportText("srt")}><Download size={14} /> {tr.exportSrt}</button
                >{/if}{#if audioUrl}<button
                  class="button secondary small"
                  title={tr.exportMp3Title}
                  disabled={busy}
                  onclick={() => exportAudio()}><Download size={14} /> {tr.exportMp3}</button
                >{/if}
              </div>{/if}
            {#if meeting}<label class="inline-search"
                ><Search size={15} /><input
                  placeholder={tr.searchContent}
                  aria-label={tr.searchTranscript}
                  bind:value={searchText}
                /></label
              ><button
                class:active={showNotes}
                class="icon-button"
                title={tr.notesTitle}
                aria-label={tr.openNotes}
                onclick={() => {
                  showNotes = !showNotes;
                }}><NotebookPen size={18} /></button
              >{/if}
          </div>
        </div>
        {#if smoothPanelOpen && meeting && canSmooth}
          <section class="smooth-panel" aria-label={tr.smoothHeading}>
            <div class="smooth-panel-head">
              <h3><Sparkles size={16} /> {tr.smoothHeading}</h3>
              <button
                class="icon-button"
                aria-label={tr.closeNotice}
                title={tr.closeNotice}
                onclick={() => {
                  smoothPanelOpen = false;
                }}><X size={15} /></button
              >
            </div>
            {#if meeting.smoothed}
              <p class="smooth-meta">
                {fill(tr.smoothResultBy, {
                  provider: meeting.smoothed.provider,
                  model: meeting.smoothed.model,
                })}
              </p>
              <div class="smooth-result">
                <p>{meeting.smoothed.text}</p>
              </div>
              <div class="smooth-actions">
                <button
                  class="text-button"
                  onclick={() => copySmoothed()}><Check size={14} /> {tr.smoothCopy}</button
                >
                <button
                  class="button secondary small"
                  disabled={busy}
                  onclick={() => {
                    smoothOpen = true;
                  }}><Sparkles size={14} /> {tr.smoothAgain}</button
                >
              </div>
            {:else}
              <p class="smooth-intro">{tr.smoothIntro}</p>
            {/if}
            <p class="smooth-note">
              {tr.smoothOriginalKept}
              {data.llm_configured ? "" : ` ${tr.aiMissing}`}
            </p>
          </section>
        {/if}
        <div class="workspace" class:workspace-with-banner={refining}>
          {#if refining}
            <div class="refine-progress-banner" role="status">
              <RefreshCw size={15} class="spin" />
              <span>{tr.refiningBanner}</span>
            </div>
          {/if}
          {#if transcriptTab === "summary" && meeting?.summary}
            <div class="summary-view" role="tabpanel" aria-label={tr.summaryTitle}>
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
            {#each segments as s (s.id)}
              <article class="segment">
                <button
                  class="timestamp"
                  title={tr.playFromHere}
                  disabled={!audioUrl}
                  onclick={() => seek(s.start)}>{clock(s.start)}</button
                >
                <div class="segment-body">
                  {#if editing === s.id}<textarea
                      aria-label={tr.editTranscriptAria}
                      bind:value={draft}></textarea>
                    <div class="edit-actions">
                      <button
                        class="text-button"
                        onclick={() => {
                          editing = "";
                        }}> {tr.editCancel}</button
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
                <button
                  class="icon-button edit-button"
                  title={tr.editSegment}
                  aria-label={tr.editSegmentAria.replace("{time}", clock(s.start))}
                  onclick={() => {
                    editing = s.id;
                    draft = s.text;
                  }}><Pencil size={14} /></button
                >
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
            {#if liveSegment}
              <article class="segment pending" aria-label={tr.liveAria}>
                <button
                  class="timestamp"
                  title={tr.playFromHere}
                  disabled={!audioUrl}
                  onclick={() => seek(liveSegment.start)}>{clock(liveSegment.start)}</button
                >
                <div class="segment-body">
                  <p data-live-caption>{liveSegment.text}</p>
                </div>
              </article>
            {/if}
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
              ></span
              >{/if}
          </button>
          <div class="meeting-card-actions">
            <button
              class="card-action"
              title={tr.renameMeeting}
              aria-label={`${tr.renameMeeting}: ${s.title}`}
              onclick={() => startRename(s)}><Pencil size={14} /><span>{tr.renameShort}</span></button
            >
            <button
              class="card-action danger"
              title={tr.deleteMeeting}
              aria-label={`${tr.deleteMeeting}: ${s.title}`}
              onclick={() => askDelete(s)}><Trash2 size={14} /><span>{tr.deleteShort}</span></button
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
    <p class="smooth-config">
      {fill(tr.smoothUsing, {
        provider: smoothProvider,
        model: smoothModel || smoothDefaults[smoothProvider],
      })}
    </p>
    {#if llmConnected}
      <p class="smooth-connected" role="status">{tr.aiConnected}</p>
    {:else}
      <p class="smooth-connect-hint">{tr.aiConnectRequired}</p>
      {#if !data.llm_configured}<p
            class="smooth-connect-hint">{tr.aiNeedKey}</p
          >{/if}
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
          aria-label={tr.smoothPromptLabel}
        ></textarea>
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
    <h2 id="summary-dialog-title"><ClipboardList size={18} /> {tr.summaryTitle}</h2>
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
          rows="3"
        ></textarea></label
      >
      <p class="smooth-config">
        {fill(tr.smoothUsing, {
          provider: smoothProvider,
          model: smoothModel || smoothDefaults[smoothProvider],
        })}
      </p>
      {#if llmConnected}
        <p class="smooth-connected" role="status">{tr.aiConnected}</p>
      {:else}
        <p class="smooth-connect-hint">{tr.aiConnectRequired}</p>
        {#if !data.llm_configured}<p
            class="smooth-connect-hint"
          >{tr.aiNeedKey}</p>{/if}
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
        {busy ? tr.summaryCreating : meeting?.summary ? tr.summaryRegenerate : tr.summaryCreate}
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
        <h2 id="model-picker-title"><List size={18} /> {tr.modelPickerTitle}</h2>
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
            <span class="model-picker-badge"><Check size={14} /> {tr.modelSelected}</span>
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
      {fill(tr.deleteBody, { title: deleteTarget?.title ?? meeting?.title ?? "" })}
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
