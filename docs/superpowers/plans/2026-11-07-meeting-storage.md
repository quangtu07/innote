# Meeting Storage Separation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Cho người dùng chọn một thư mục chung cho cuộc họp mới, giữ cuộc họp cũ ở nguyên vị trí và công bố dữ liệu cuộc họp dễ đọc cho agent mà không mang theo API key.

**Architecture:** Giữ private root tương thích, thêm registry các thư viện và định tuyến bằng UUID/location bất biến. Mỗi thư viện mới có SQLite chuẩn, thư mục mang tên cuộc họp và bundle JSON/Markdown/SRT được công bố từ DB; không nhập ngược từ file của agent. Run giữ location cố định từ đầu đến cuối.

**Tech Stack:** Rust, rusqlite, tokio, Windows filesystem, Tauri v2, Svelte 5/TypeScript; dùng pattern và dependency sẵn có trước khi thêm thư viện.

**Spec:** [Đặc tả lưu trữ](../../specs/2026-11-07-meeting-storage.md).

## Global Constraints

- Một thư mục chung, chỉ đổi trong Cài đặt; chỉ áp dụng cho cuộc họp mới. Không chuyển hoặc xóa dữ liệu cũ.
- API profiles/key-format, mẫu cấu hình, model local và private root giữ tương thích. Không copy mixed AppData sang thư viện.
- PCM/WAV/DB/bundle và temp audio do app quản lý phải ở đúng location; không fallback âm thầm về C khi nơi lưu mới lỗi.
- Không làm MCP/cloud sync; không tự nhập file agent, không tự dọn PCM, không tự tạo MP3.
- Giữ UUID nội bộ; tên thư mục `YYYY-MM-DD_HH-mm-ss - <Tên cuộc họp>`, Unicode, hậu tố trùng ` (2)`, ngân sách đường dẫn 240 UTF-16 code units.
- Công bố tối đa một lần/giây khi đang thu; force flush khi kết thúc/thao tác hoàn tất. JSON tự đủ, có revision/manifest; không hứa atomic cho nhiều file.
- Local/USB v1; từ chối UNC/network drive cho DB WAL, root chồng private/runtime, reparse point trong đường dẫn quản lý.
- Mọi test ghi dữ liệu dùng fixture riêng. Không dùng dữ liệu khách làm fixture; không sửa/xóa key thật. Không tự nâng version, release hoặc push khi thực hiện plan nếu chưa được yêu cầu riêng.

## Review Focus

1. Thay root và bắt đầu ghi cùng lúc: backend phải chốt đúng một location, không chia WAV và DB ở hai ổ (Task 3).
2. Root A mất kết nối sau khi chọn B: các cuộc họp B và legacy vẫn hiện; không ghi DB rỗng đè dấu vết A (Tasks 1–2).
3. Agent giữ file JSON đang mở trên Windows: publisher retry, JSON cũ vẫn hợp lệ và UI không báo cập nhật giả (Task 4).
4. Người dùng đổi tên thư mục ngoài app hoặc copy trùng thư viện: relink theo ID có kiểm chứng, không chọn bản ngẫu nhiên (Task 2).
5. Cuộc họp cũ thiếu WAV/đang gián đoạn: phân biệt thiếu audio với mất bản chép; recovery cũ vẫn dùng đúng nơi cũ (Tasks 2–3).

---

## Cấu trúc file và hợp đồng dùng chung

**Tiến độ 07/11/2026:** đã triển khai Tasks 1–6, kiểm thử tích hợp Windows/API thật và NSIS build tại máy. Full Rust 138/138, full UI 140/140 pass; xem [báo cáo kiểm thử](../../reviews/2026-11-07-meeting-storage-qa.md). Không di chuyển dữ liệu thật, chưa release/push.

Triển khai dùng `meeting_store::Location` và facade storage hiện có thay vì thay mọi call-site thành `MeetingStore` object; `May.root` vẫn riêng cho config/model và được router giải quyết khi đọc/ghi meeting. Đường dẫn được chốt theo UUID/index, không theo default hoặc title mới. Chi tiết điều chỉnh DTO/identity/SRT và giới hạn thử USB/full disk nằm trong QA.

Tên dưới đây là API dự kiến, cần giữ nhất quán giữa các task. `Result<T>` dùng `anyhow::Result<T>`; ID là UUID string đã kiểm tra. Các type mới khai báo cùng module sở hữu, không dùng chuỗi đường dẫn frontend làm định danh cuộc họp.

| File | Trách nhiệm |
|---|---|
| `src-tauri/src/storage_paths.rs` (mới) | Registry riêng tư, nhận diện thư viện, kiểm tra root, tính đường dẫn có giới hạn |
| `src-tauri/src/meeting_store.rs` (mới) | Router legacy/thư viện, DB session_paths, tạo/đổi tên/khôi phục/xóa an toàn |
| `src-tauri/src/meeting_bundle.rs` (mới) | DTO chia sẻ, render bundle, publish atomic từng file, revision/retry |
| `src-tauri/src/main.rs`, `types.rs` | State/commands, task coordinator, Snapshot và Run location |
| `src-tauri/src/storage.rs` | Giữ adapter legacy và formatter export; không còn quyết định root cho phiên mới |
| `src-tauri/src/audio.rs`, `engine.rs`, `caption.rs`, `meeting_soniox.rs`, `meeting_groq.rs` | Tách capture dir khỏi WAV cuối, dùng location đã chốt |
| `src-tauri/tauri.conf.json`, `src-tauri/src/instance.rs` | Media scope và single-writer tương thích |
| `src/MeetingStorageSettings.svelte` (mới), `App.svelte`, `types.ts`, `style.css`, `src/i18n/{index,vi,en,ko,zh}.ts` | Settings lưu trữ, trạng thái root/bundle, mở đúng thư mục |
| `scripts/meeting-storage-check.mjs`, `scripts/meeting-storage-native-check.cjs` (mới) | Kiểm thử UI và Windows thật với dữ liệu cách ly |

### Task 1: Registry và đường dẫn thư viện

**Files:** tạo `src-tauri/src/storage_paths.rs`; khai báo module trong `src-tauri/src/main.rs`; chỉ sửa `src-tauri/Cargo.toml` nếu API filesystem/locking hiện có chưa đủ. Tests trong `storage_paths.rs::tests`.

**Interfaces:**

- `StorageRegistry { schema_version: u32, default_library_id: Option<String>, libraries: Vec<LibraryRef> }`; `LibraryRef { id: String, root: PathBuf }`.
- `MeetingLocation { library_id: Option<String>, session_id: String, layout: MeetingLayout, paths: SessionPaths }`; `MeetingLayout::{Legacy, NamedV1}`.
- `SessionPaths { meeting_dir: PathBuf, capture_dir: PathBuf, recording_path: PathBuf }`.
- `load_registry(private_root: &Path) -> Result<StorageRegistry>`; `save_registry(private_root: &Path, registry: &StorageRegistry) -> Result<()>`.
- `validate_library_root(candidate: &Path, private_root: &Path, runtime_root: &Path) -> Result<ValidatedRoot>`; `ValidatedRoot` giữ canonical path và kết quả probe. Đọc trạng thái root không gọi hàm tạo DB.
- `open_or_create_library(root: &ValidatedRoot) -> Result<LibraryRef>` phân biệt root mới với library marker hiện hữu; trả về writer lease riêng khi mở ghi.

- [ ] Viết tests `registry_restart_keeps_default_and_previous_roots`, `reject_private_overlap_and_reparse`, `missing_root_does_not_create_database`, `duplicate_library_identity_is_not_merged`, `probe_failure_keeps_previous_default`. Assertions: A→B vẫn giữ A, cancel không đổi bytes registry, thiếu ổ không tạo DB, root canonical trùng không nhân đôi.
- [ ] Chạy `cargo test --manifest-path src-tauri/Cargo.toml storage_paths::tests`; xác nhận các test thất bại vì chưa có API mới.
- [ ] Implement registry ghi temp+replace trên cùng ổ, marker `library.json`, probe tạo/đọc/rename/xóa đúng file probe, phát hiện loại ổ mạng. Không sửa key/profile hoặc tạo registry mới đè registry hỏng.
- [ ] Chạy lại lệnh test trên; PASS. Test writer lease bằng hai handle/tiến trình cách ly, không chỉ mutex trong một object.
- [ ] Commit riêng file của task: `feat: add private meeting library registry` (không stage toàn bộ docs/test-results sẵn có).

### Task 2: Router dữ liệu, thư mục có tên và journal

**Files:** tạo `src-tauri/src/meeting_store.rs`; sửa `src-tauri/src/storage.rs`; tests trong hai module.

**Interfaces:**

- `MeetingStore::open(private_root: PathBuf, registry: StorageRegistry) -> Result<Self>`.
- `list(&self) -> Result<LibraryListing>` với `LibraryListing { sessions: Vec<Session>, issues: Vec<LibraryIssue> }`; offline/corrupt library là issue riêng, không che tất cả nguồn còn tốt.
- `load(&self, id: &str) -> Result<Session>`; `locate(&self, id: &str) -> Result<MeetingLocation>`; `create(&self, session: &Session) -> Result<MeetingLocation>`.
- `save(&self, session: &Session) -> Result<u64>` trả revision đã commit; `rename(&self, id: &str, title: &str, defer: bool) -> Result<RenameOutcome>`; `delete(&self, id: &str) -> Result<()>`; `recover(&self) -> Result<Vec<LibraryIssue>>`; `rescan_library(&self, library_id: &str) -> Result<Vec<LibraryIssue>>`.
- `RenameOutcome::{Renamed, Deferred, PendingRetry}`. `LibraryIssue` giữ ID, mã lỗi và thông báo an toàn; không đẩy Session lỗi hoặc private config ra bundle.

- [ ] Viết tests `new_meeting_uses_named_library`, `legacy_finished_rows_unchanged_on_open`, `switch_default_keeps_old_routes`, `unicode_reserved_long_and_duplicate_titles`, `rename_journal_recovers_each_commit_boundary`, `rescan_rejects_duplicate_session_id`, `offline_library_keeps_other_sessions_visible`. Assertions: cùng UUID sau rename; audio vẫn đúng; không ghi vào C khi A lỗi; chỉ quét trong root đã đăng ký, không theo junction.
- [ ] Chạy `cargo test --manifest-path src-tauri/Cargo.toml meeting_store::tests`; xác nhận FAIL trước triển khai.
- [ ] Implement adapter legacy không migrate DB cũ; DB thư viện mới tái dùng Session body với `session_paths` và revision/bundle state. Tạo thư mục bằng create-new, journal durable cho create/rename/delete; recovery lặp an toàn. Hạn chế thao tác filesystem vào location đã xác minh.
- [ ] Implement deferred rename sau khi hết job/audio handle; rescan đọc allowlisted ID/location, không nhập bản chép từ file. Phát hiện copy trùng marker, root offline và DB corrupt thành trạng thái rõ; không nuốt lỗi thành danh sách rỗng.
- [ ] Chạy test của task và test `storage::tests`; PASS. So sánh bytes fixture key, body DB và hash WAV legacy trước/sau mở app/chọn root/restart; không so raw SQLite file sau checkpoint như bằng chứng nội dung.
- [ ] Commit: `feat: route meetings across legacy and named libraries`.

### Task 3: Toàn bộ vòng đời ghi, nghe lại, hiệu chỉnh và media scope

**Files:** sửa `src-tauri/src/main.rs`, `types.rs`, `audio.rs`, `engine.rs`, `caption.rs`, `meeting_soniox.rs`, `meeting_groq.rs`, `instance.rs`, `src-tauri/tauri.conf.json`.

**Interfaces:**

- `May.private_root: PathBuf`, `May.meetings: MeetingStore`; `Run.location: MeetingLocation`. Không giữ `May.root` mơ hồ cho mọi chức năng.
- `audio::finalize_to(capture_dir: &Path, recording_path: &Path, duration: f64) -> Result<()>`; giữ `finalize(dir, duration)` làm legacy wrapper nếu cần.
- Bộ điều phối tác vụ của `May` đồng bộ create/start/change-default/rename/delete và lease đọc audio; không giữ khóa qua await mạng hoặc audio callback. Đổi root phải kiểm tra backend idle ngay trước commit registry.
- `configure_media_scope(app: &tauri::AppHandle, registry: &StorageRegistry, private_root: &Path) -> Result<()>` là helper duy nhất cấp scope, dùng API Tauri v2 thực tế đã kiểm chứng khi triển khai; không cấp toàn private root.

- [ ] Viết tests `start_and_change_root_pin_one_location`, `old_correction_reads_old_wav`, `legacy_interrupted_session_recovers_in_place`, `missing_wav_keeps_transcript_readable`, `capture_full_disk_never_falls_back_to_private_root`, `active_jobs_defer_rename`. Inject I/O failure tại capture/finalize; kiểm tra trạng thái lỗi có thể phục hồi, không báo complete giả.
- [ ] Chạy Rust tests chọn lọc theo từng tên; xác nhận FAIL trước khi sửa route tương ứng.
- [ ] Chuyển `May.edit`, snapshot/list/load/save, start/stop/pause/resume/recovery, Soniox/Groq shared stream, local ASR sang router. Tính title trước create; truyền capture dir đã pin cho mọi worker; WAV cuối đặt tại meeting folder. Tách caption archive path nếu hiện đang ngầm dùng chung capture dir.
- [ ] Chuyển audio_path, correction (kể cả pending WAV/temp), summary persist, notes, exports/MP3 và delete sang resolver. Export mặc định `meeting_dir/exports/`; đích người dùng tự chọn vẫn được tôn trọng. Audio tests nằm `.innote/audio-tests/` của thư viện mới, không tự tải model. Audit/log chẩn đoán nếu gắn với phiên chỉ nằm trong `.innote/work/`, không lẫn các file công bố cho agent.
- [ ] Giữ key/model/log/instance root trên private root; thêm library writer lease trước recovery. Thay asset scope `**` bằng grant các audio root hợp lệ và legacy `sessions/`; kiểm tra bằng ID, không cho command trả WAV của đường dẫn tùy ý.
- [ ] Chạy `cargo test --manifest-path src-tauri/Cargo.toml`; PASS. Native media scope kiểm tra lại ở Task 6 vì unit test không chứng minh được WebView phát WAV khác ổ.
- [ ] Commit: `feat: pin meeting storage across recording and playback`.

### Task 4: Bundle JSON/Markdown cho agent

**Files:** tạo `src-tauri/src/meeting_bundle.rs`; sửa hook lưu trong `meeting_store.rs`, formatter `storage.rs`, trạng thái trong `types.rs`/`main.rs`.

**Interfaces:**

- `MeetingBundleV1`: DTO allowlist theo spec §5, với `schema_version`, `revision`, metadata, bản gốc/dịch/hiệu chỉnh/tóm tắt/notes và manifest đường dẫn tương đối/checksum; không embed Session thô.
- `BundlePublisher::mark_dirty(&self, id: &str, revision: u64)`; `flush(&self, id: &str) -> Result<BundleStatus>`; `BundleStatus { revision: u64, published_revision: u64, state: BundleState }`; `BundleState::{Current, Pending, Failed}`.
- `MeetingStore` lưu revision chuẩn và published revision; publisher không được ghi revision thấp hơn bản đã công bố. `flush` chỉ báo Current khi đã công bố tới revision cần flush.

- [ ] Viết tests `bundle_excludes_secret_and_profile_sentinels`, `bundle_keeps_original_translation_and_correction_distinct`, `bundle_json_is_complete_after_each_publish_fault`, `locked_json_retains_previous_valid_snapshot`, `stale_worker_cannot_overwrite_new_revision`, `srt_remains_parseable`, `agent_extra_files_are_untouched`. Sentinel phải bao gồm key/profile_id/Base URL/prompt trong các trường kỹ thuật, không đưa vào lời nói rồi xóa lời người dùng.
- [ ] Chạy `cargo test --manifest-path src-tauri/Cargo.toml meeting_bundle::tests`; xác nhận FAIL.
- [ ] Implement render từ snapshot DB, chỉ tạo output có thật; cập nhật MD/SRT/WAV availability, JSON cuối cùng chứa manifest. Dùng atomic replace Windows trên cùng volume, xử lý file lock và stale revision; không xóa file lạ của agent. Output do app sở hữu đã hết dữ liệu phải được cập nhật/loại bỏ có chủ đích, tránh giữ bản dịch cũ giả là hiện hành.
- [ ] Gộp publish nền tối đa 1 Hz cho committed segments; force flush cuối phiên/notes/correction/summary/rename. Lỗi render/publish giữ DB chuẩn, hiển thị Failed và cho retry; shutdown drain có giới hạn, giữ dirty state để retry lần sau. Không chạy publisher trên audio callback hoặc giữ khóa DB qua disk flush dài.
- [ ] Chạy lại tests task và full Rust; PASS, fault injection không còn JSON dở hoặc trạng thái Current giả.
- [ ] Commit: `feat: publish credential-free meeting bundles`.

### Task 5: Settings gọn, rõ nơi lưu và trạng thái file

**Files:** tạo `src/MeetingStorageSettings.svelte`; sửa `src/App.svelte`, `src/types.ts`, `src/style.css`, `src/i18n/index.ts`, `src/i18n/vi.ts`, `src/i18n/en.ts`, `src/i18n/ko.ts`, `src/i18n/zh.ts`, `src-tauri/src/main.rs`, `src-tauri/src/types.rs`; tạo `scripts/meeting-storage-check.mjs` theo UI harness hiện có.

**Interfaces:**

- Commands: `meeting_storage_status() -> StorageSummary`; `choose_meetings_directory() -> Option<String>` chỉ mở native picker, chưa commit; `set_meetings_directory(path: String) -> StorageSummary` revalidate backend rồi commit.
- Commands theo ID: `open_meetings_directory()`, `open_meeting_directory(id: String)`, `retry_meeting_bundle(id: String) -> BundleStatus`, `rescan_meeting_library(library_id: String) -> StorageSummary`.
- `StorageSummary` gồm default path/ID có thể null, readiness/error, available bytes, legacy path và issues từng library; `Snapshot` thêm storage/bundle status, giữ `data_dir` tương thích trong giai đoạn chuyển UI. Dùng private-root readiness riêng cho WhatsNew thay vì lấy meeting root làm dấu đã khởi tạo app.

- [ ] Viết UI tests: chọn folder/cancel/confirm/restart; A→B báo đúng “chỉ mới”; backend busy từ chối dù UI stale; root offline vẫn vào Cài đặt; lần đầu chưa chọn root không bắt đầu ghi; thông báo publish Failed/retry; mở đúng folder theo meeting ID. Assert không xuất hiện picker riêng trên meeting/preset/captions-only, không thêm storage path vào preset.
- [ ] Chạy `node scripts/meeting-storage-check.mjs`; xác nhận FAIL trên UI cũ.
- [ ] Implement Settings → Lưu trữ một vùng đường dẫn+nút chọn/mở, preview tên thư mục và dung lượng. Copy chính xác: “Chỉ áp dụng cho cuộc họp tạo từ bây giờ. Các cuộc họp đã có không bị chuyển.” Sau commit toast “Đã đổi nơi lưu cho cuộc họp mới”. Native picker dùng `rfd::FileDialog::pick_folder` từ dependency `rfd` 0.15 đang có, qua blocking worker theo pattern export hiện tại; không gọi browser file input để lấy đường dẫn giả.
- [ ] Thêm dòng gọn “Lưu tại: …” ở setup meeting và mở folder sau kết thúc; gợi ý Documents Known Folder khi chưa có registry nhưng phải xác nhận trước thu. Giữ Cài đặt AI, update, ngôn ngữ, model và sidebar đang hoạt động. Đường dẫn dài wrap/ellipsis có title/copy; phím Tab/Enter/Escape, focus sau picker, trạng thái loading và thông báo lỗi không lộ key.
- [ ] Chạy `npm run check`, `node scripts/meeting-storage-check.mjs`, `node scripts/ui-check.mjs`, `node scripts/workflow-presets-check.mjs`; PASS. Chụp Settings và new-meeting ở 900×640 và màn hình lớn; không tăng scroll/card không cần thiết, lỗi không che nút chính.
- [ ] Commit: `feat: add meeting storage settings and status`.

### Task 6: Kiểm thử Windows và hồi quy trước bàn giao

**Files:** tạo `scripts/meeting-storage-native-check.cjs`, báo cáo `docs/reviews/2026-11-07-meeting-storage-qa.md`; sửa fixture setup của `scripts/audio-correction-live-check.cjs`, `scripts/soniox-meeting-check.cjs`, `scripts/groq-live-usage-check.cjs` nếu cần để tách private root/library root. Không dùng trực tiếp dữ liệu thật làm test input.

- [ ] Tạo private fixture với các phiên legacy hoàn tất/gián đoạn/thiếu WAV và sentinel key, root A/B ở hai volume thực khi máy có. Ghi baseline Session body hash/audio hash/key bytes, chuẩn bị audio hệ thống xác định; harness dọn chỉ fixture đã xác minh tuyệt đối nằm trong workspace/test root.
- [ ] Kiểm thử nâng cấp/chọn A/restart/đổi B: toàn bộ legacy/A/B vẫn truy cập; mẫu cấu hình/AI key không mất; bản mới PCM/WAV/DB/bundle đúng B, không có audio/transcript mới dưới private fixture. Người dùng không phải chọn nơi lưu mỗi phiên.
- [ ] Phát audio qua loa, chạy local, pause/resume/stop, bật/tắt hộp phụ đề và nghe lại theo đoạn từ ổ mới. Có key đã lưu và được phép dùng thì chạy riêng Soniox shared translation/Groq shared transcript; không có thì ghi rõ chưa xác thực API thật, không gọi mock là live pass. Test source-only captions không tự tạo cuộc họp lưu trữ.
- [ ] Chạy hiệu chỉnh Moonshine/Nemotron theo model sẵn có, hiệu chỉnh cloud nếu có quyền, tóm tắt bằng fixture hoặc API được phép; xác nhận original giữ nguyên, corrected/translation/summary riêng và relative audio link hợp lệ. Mở agent JSON/MD từ named folder mà không cần đọc private root.
- [ ] Native fault tests: rename khi audio đang mở, file lock, root offline/đổi drive letter, denied write/full disk bằng injection hoặc volume fixture, process kill/restart tại journal boundary. Không cố rút ổ đang chứa dữ liệu người dùng. Chứng minh không fallback C và khôi phục đúng location; không cam kết bytes chưa flush được phục hồi.
- [ ] Chạy `cargo test --manifest-path src-tauri/Cargo.toml`, `npm run check`, `npm run build` và nhóm UI mới/hiện có đã nêu. `npm run package` để thử NSIS cục bộ khi triển khai; không upload/release. Mở đúng exe vừa build, kiểm tra phát WAV khác ổ và asset protocol từ chối đường dẫn key/config; không nối UI mới vào backend cũ.
- [ ] Viết QA với phiên bản/commit được test, dữ liệu fixture, check thực tế và phần chưa test, ảnh UI, hash so sánh; tự đối chiếu 10 acceptance criteria của spec. Commit chỉ test/docs của task. Bàn giao kết quả trước mọi yêu cầu phát hành riêng.

## Tự rà soát kế hoạch

- Spec §1–3 → Tasks 1–3: new-only, legacy đọc/sửa tại chỗ, root lịch sử không mất.
- Spec §4 → Tasks 1–2/6: Unicode/collision/path budget/journal/deferred rename và Explorer relink.
- Spec §5 → Task 4/6: allowlist, revision, lỗi publish, file agent không bị coi là lệnh.
- Spec §6 → Task 5: thư mục chung chỉ trong Settings, native confirm, busy/offline/first run rõ.
- Spec §7–8 → Tasks 1–3/6: single-writer, media scope, disk failures, audio thật, native và regression.
- Các type/API nối task đã dùng cùng tên. Không đưa bulk migration, per-meeting storage picker, MCP, cleanup hoặc push vào phạm vi.

Đây là kế hoạch để review, chưa phải báo cáo triển khai hoặc kiểm thử đã chạy. Sau khi người dùng chọn cách thực hiện mới triển khai từng task.
