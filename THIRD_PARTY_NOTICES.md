## Silero VAD sidecar — 0.5.1

NeMo uses a GGUF conversion of the existing Silero VAD 5.1.2 MIT weights for speech-based endpointing. The stdlib converter in scripts/nemo-vad.py follows tensor names/layout and metadata documented by NVIDIA NeMo-Speech.cpp v0.2.0 conversion/vad.py (Apache-2.0); no additional proprietary weights are included. The sidecar is reproducible from the existing GGML asset and pinned by SHA-256 in scripts/nemo-resources.json. Both license texts are included in this bundle.

# Third-party notices

Innote source code is licensed under PolyForm Noncommercial 1.0.0; see
`LICENSE`. That project license does not replace the licenses of the
third-party components, models and binaries listed below.

Multilingual mode (0.5.0): NeMo-Speech.cpp 0.2.0 native ASR runtime, Apache-2.0,
https://github.com/NVIDIA/NeMo-Speech.cpp/tree/v0.2.0 . Bundled native dependencies
include ggml (MIT); all collected upstream notices/licenses are in licenses/nemo/.
Nemotron 3.5 ASR Q8 weights use OpenMDW-1.1 (not MIT), copyright NVIDIA; weights
are downloaded separately from the pinned official Hugging Face revision. The
full OpenMDW text and SDK/model URL, size, SHA-256 inventory are included there.
The CPU runtime needs the bundled MSVC OpenMP vcomp140 redistributable; Microsoft
redistribution terms apply. No NVIDIA cloud service or API key is required.


Streaming preview (0.4.0): Moonshine Voice 0.1.5 native runtime and Tiny Streaming
Vietnamese/English models, MIT: https://github.com/moonshine-ai/moonshine .
The Vietnamese streaming config changes total_lookahead from 16 to 8; weights are
unmodified. No Python interpreter is shipped. ONNX Runtime 1.23.2 uses MIT terms.
Pinned downloads, upstream/installed hashes, changes and collected upstream notices
are under `licenses/moonshine/`. Only streaming models are bundled; legacy
non-English non-streaming models have different terms and are not included.

Historical preview (0.3.0, no longer bundled in 0.4.0): Vosk API 0.3.45 / Kaldi / OpenFST, Apache 2.0;
Vietnamese `vosk-model-small-vn-0.4` and English `vosk-model-small-en-us-0.15`,
Apache 2.0 according to https://alphacephei.com/vosk/models . These models are
bundled unmodified. The Windows native runtime comes from the pinned official
PyPI wheel, not a Python interpreter. Its MinGW GCC runtime is distributed under
GPLv3 with the GCC Runtime Library Exception; winpthreads uses the bundled MIT/BSD
terms and OpenBLAS uses BSD 3-Clause. Corresponding license texts and pinned source
archive URLs/checksums are in `licenses/streaming/`. Native source/build instructions:
https://github.com/alphacep/vosk-api/tree/v0.3.45 and https://alphacephei.com/vosk/install .
No Scribble code, assets, or restricted Zipformer weights are included.

The app uses open-source dependencies; their individual copyright and license terms apply. Dependency versions are pinned in Cargo.lock and package-lock.json.

- Tauri: MIT / Apache-2.0 — https://github.com/tauri-apps/tauri
- rfd: MIT — https://github.com/PolyMeilex/rfd
- Svelte: MIT — https://github.com/sveltejs/svelte
- Lucide: ISC — https://github.com/lucide-icons/lucide
- wasapi-rs: MIT — https://github.com/HEnquist/wasapi-rs
- whisper-rs: Unlicense — https://docs.rs/crate/whisper-rs/0.16.0
- whisper.cpp and bundled ggml: MIT — https://github.com/ggml-org/whisper.cpp
- Whisper models: MIT — https://github.com/openai/whisper
- Silero VAD model v5.1.2: MIT — https://github.com/snakers4/silero-vad
  Distributed ggml conversion: https://huggingface.co/ggml-org/whisper-vad
  File SHA-256: `29940d98d42b91fbd05ce489f3ecf7c72f0a42f027e4875919a28fb4c04ea2cf`
- SQLite: public domain — https://sqlite.org/copyright.html

Model downloads use the public ggerganov/whisper.cpp repository on Hugging Face. SHA-256 digests were checked against the repository's LFS metadata on 24 October 2026 and are pinned in the Rust model catalog.

Developer test data is not bundled with the app. The Vietnamese smoke fixture is Google FLEURS `vi_vn`, validation row 0, ID 1594 (CC BY 4.0): https://huggingface.co/datasets/google/fleurs . English test speech was synthesized locally with Windows SAPI from developer-written text. Mixed test audio concatenates these fixtures; it is not a real meeting.

Microsoft Visual C++ 2022 x64 runtime files are included from the installed Visual Studio Build Tools 2022 redistributable folder (`VC/Redist/MSVC/14.44.35112/x64/Microsoft.VC143.CRT`, DLL file version 14.44.35211.0). These replace the VS2019 runtime when moving the build to MSVC v143. Microsoft copyrights and redistribution terms apply to these binaries; they are not covered by the application's open-source dependency licenses. WebView2 is provided separately by Microsoft/Windows.

Collected dependency license texts and their inventory are bundled under `licenses/dependencies/`.
