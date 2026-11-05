import type { UiLocale } from "../i18n/shared";
export interface ReleaseContent {
  title:string; subtitle:string; close:string; done:string; once:string;
  items:{title:string;detail:string}[];
}
// Per-version notes are preserved so upgrade notices describe the installed build.
export const releaseNotes: Record<string,Record<UiLocale,ReleaseContent>> = {
  "0.8.5": {
    vi:{title:"Có gì mới trong Innote?",subtitle:"Phụ đề linh hoạt hơn, ghi chép thuận tiện hơn.",close:"Đóng thông báo cập nhật",done:"Đã hiểu",once:"Chỉ tự động hiện một lần cho phiên bản này.",items:[
      {title:"Phụ đề theo cách bạn cần",detail:"Xem lời gốc hoặc dịch sang ngôn ngữ khác. Bật song ngữ, kéo hộp phụ đề, đổi kích thước và tùy chỉnh màu nền trong Cài đặt."},
      {title:"Chọn AI riêng cho từng tác vụ",detail:"Lưu nhiều cấu hình API, chọn model và mức Nhanh, Cân bằng hoặc Chính xác. Dự phòng chỉ dùng cấu hình bạn đã chọn."},
      {title:"Theo dõi cuộc họp liền mạch",detail:"Khi đang ghi, bản chép hiện thành một đoạn nối tiếp. Kết thúc phiên mới hiện từng đoạn có mốc thời gian để nghe lại và sửa."},
      {title:"Bộ cài nhẹ hơn",detail:"Moonshine có sẵn để chép lời trên máy. Đã bỏ PhoWhisper khỏi bộ cài; các cuộc họp và cấu hình API đã lưu vẫn được giữ lại."},
    ]},
    en:{title:"What’s new in Innote?",subtitle:"More flexible captions and smoother meeting notes.",close:"Close update notes",done:"Got it",once:"Shown automatically once for this version.",items:[
      {title:"Captions your way",detail:"Read the original speech or translate it. Enable bilingual captions, drag and resize the window, and customize its background in Settings."},
      {title:"Choose AI for each task",detail:"Save multiple API profiles and choose models with Fast, Balanced or Accurate processing. Fallback uses only the profile you selected."},
      {title:"Follow meetings continuously",detail:"While recording, the transcript flows as one paragraph. After ending, timestamped segments are available for listening and editing."},
      {title:"A smaller installer",detail:"Moonshine is included for on-device transcription. PhoWhisper is no longer bundled; saved meetings and API profiles are preserved."},
    ]},
    ko:{title:"Innote의 새로운 기능",subtitle:"더 유연한 자막과 편리한 회의 기록을 만나보세요.",close:"업데이트 안내 닫기",done:"확인",once:"이 버전에서 한 번만 자동으로 표시됩니다.",items:[
      {title:"원하는 방식으로 자막 보기",detail:"원문 또는 번역을 보고 이중 언어 자막을 켤 수 있습니다. 자막 창을 이동하거나 크기를 조절하고 설정에서 배경을 바꾸세요."},
      {title:"작업별 AI 선택",detail:"여러 API 설정과 모델을 저장하고 빠르게, 균형, 정확도 우선 처리를 선택하세요. 미리 선택한 설정만 대체 경로로 사용합니다."},
      {title:"끊김 없이 회의 내용 읽기",detail:"녹음 중에는 기록이 한 문단으로 이어집니다. 종료 후에는 시간별 구간을 듣고 수정할 수 있습니다."},
      {title:"더 가벼운 설치 파일",detail:"기기 내 음성 인식용 Moonshine이 포함됩니다. PhoWhisper는 제외되며 저장된 회의와 API 설정은 유지됩니다."},
    ]},
    zh:{title:"Innote 更新了什么？",subtitle:"更灵活的字幕，更方便的会议记录。",close:"关闭更新说明",done:"知道了",once:"此版本仅自动显示一次。",items:[
      {title:"按需显示字幕",detail:"查看原文或翻译，开启双语字幕，拖动和调整字幕窗口大小，并在设置中自定义背景。"},
      {title:"为每项任务选择 AI",detail:"保存多个 API 配置，选择模型及快速、均衡或准确优先模式。仅使用您选定的备用配置。"},
      {title:"连贯地阅读会议内容",detail:"录音时，转录文本连续显示为一个段落。结束后，可按时间段回听和编辑。"},
      {title:"更轻量的安装包",detail:"内置 Moonshine，支持本机转录。不再捆绑 PhoWhisper；已保存的会议和 API 配置仍会保留。"},
    ]},
  },
};
releaseNotes["0.8.6"]=releaseNotes["0.8.5"];
releaseNotes["0.8.7"]={
  "vi": {
    "title": "Có gì mới trong Innote?",
    "subtitle": "Mẫu cấu hình rõ ràng hơn, phụ đề dễ dùng hơn.",
    "close": "Đóng thông báo cập nhật",
    "done": "Đã hiểu",
    "once": "Chỉ tự động hiện một lần cho phiên bản này.",
    "items": [
      {
        "title": "Mẫu riêng cho cuộc họp và chỉ phụ đề",
        "detail": "Lưu model, ngôn ngữ, mức xử lý và kiểu hộp phụ đề thành mẫu. Bấm Áp mẫu để sử dụng và nhận xác nhận; tùy chỉnh phiên không sửa mẫu gốc."
      },
      {
        "title": "Chỉnh chữ ngay tại màn hình sử dụng",
        "detail": "Cỡ chữ, căn chữ, màu chữ/nền, song ngữ và trong suốt luôn hiện sẵn trong Cuộc họp và Chỉ phụ đề. Áp mẫu sẽ cập nhật các ô để dễ đối chiếu."
      },
      {
        "title": "Sửa bản dịch trong cuộc họp",
        "detail": "Bật hoặc tắt hộp phụ đề trong lúc ghi. Bản chép và bản dịch đã chọn vẫn được lưu khi ẩn hộp; sửa trường hợp cột bản dịch hiện lại lời gốc."
      },
      {
        "title": "Hộp nổi và thanh bên gọn hơn",
        "detail": "Câu dài hiện lần lượt trong kích thước hộp đã chọn. Phụ đề song ngữ chuyển phần gốc/dịch cùng nhau. Thanh bên có thể thu gọn và vẫn giữ liên kết tác giả."
      }
    ]
  },
  "en": {
    "title": "What’s new in Innote?",
    "subtitle": "Clearer presets and easier-to-use captions.",
    "close": "Close update notes",
    "done": "Got it",
    "once": "Shown automatically once for this version.",
    "items": [
      {
        "title": "Separate meeting and caption-only presets",
        "detail": "Save models, languages, processing levels and caption appearance. Apply a preset with confirmation; session edits keep the saved preset unchanged."
      },
      {
        "title": "Appearance controls where you use captions",
        "detail": "Font size, alignment, text/background colors, bilingual mode and transparency stay expanded on the meeting and caption-only screens. Applying a preset updates these controls."
      },
      {
        "title": "More reliable meeting translations",
        "detail": "Toggle the caption window while recording. Selected transcripts and translations continue to be saved when the window is hidden. Fixed source text appearing in the translation column."
      },
      {
        "title": "A tidier overlay and sidebar",
        "detail": "Long captions advance within your chosen window size. Original and translated pages advance together in bilingual mode. Collapse the sidebar while keeping author contact links available."
      }
    ]
  },
  "ko": {
    "title": "Innote의 새로운 기능",
    "subtitle": "더 명확한 프리셋과 편리한 자막을 만나보세요.",
    "close": "업데이트 안내 닫기",
    "done": "확인",
    "once": "이 버전에서 한 번만 자동으로 표시됩니다.",
    "items": [
      {
        "title": "회의와 자막 전용 프리셋",
        "detail": "모델, 언어, 처리 수준과 자막 모양을 프리셋으로 저장하세요. 적용 결과를 확인할 수 있으며 세션에서 수정해도 저장된 프리셋은 바뀌지 않습니다."
      },
      {
        "title": "사용 화면에서 바로 자막 모양 변경",
        "detail": "회의와 자막 전용 화면에서 글자 크기, 정렬, 글자·배경색, 이중 언어와 투명도를 항상 펼쳐 표시합니다. 프리셋을 적용하면 각 값이 함께 바뀝니다."
      },
      {
        "title": "회의 번역 오류 수정",
        "detail": "녹음 중 자막 창을 켜거나 끌 수 있습니다. 창을 숨겨도 선택한 전사와 번역은 계속 저장됩니다. 번역 열에 원문이 다시 표시되는 문제를 수정했습니다."
      },
      {
        "title": "간결한 자막 창과 사이드바",
        "detail": "긴 자막을 선택한 창 크기 안에서 나누어 표시하고 이중 언어의 원문과 번역을 함께 넘깁니다. 사이드바를 접을 수 있으며 작성자 연락 링크도 유지됩니다."
      }
    ]
  },
  "zh": {
    "title": "Innote 更新了什么？",
    "subtitle": "更清晰的预设，更方便的字幕。",
    "close": "关闭更新说明",
    "done": "知道了",
    "once": "此版本仅自动显示一次。",
    "items": [
      {
        "title": "会议与仅字幕模式分别保存预设",
        "detail": "保存模型、语言、处理级别及字幕外观。应用预设后显示确认；修改当前会话不会更改已保存的预设。"
      },
      {
        "title": "在使用字幕的页面直接调整外观",
        "detail": "会议和仅字幕页面始终展开字号、对齐、文字与背景颜色、双语和透明选项。应用预设后可直接核对各项数值。"
      },
      {
        "title": "修复会议翻译显示",
        "detail": "录音时可开关字幕窗口。隐藏窗口后，选定的转录与翻译仍会继续保存；修复翻译栏重复显示原文的问题。"
      },
      {
        "title": "更简洁的悬浮字幕与侧边栏",
        "detail": "长字幕在选定的窗口大小内分段显示，双语模式下原文与译文同步翻页。侧边栏可以折叠，并保留作者联系链接。"
      }
    ]
  }
};


releaseNotes["0.8.8"]={
  "vi": {
    "title": "Có gì mới trong Innote?",
    "subtitle": "Chọn AI dễ hơn, nghe lại bản thu để hiệu chỉnh.",
    "close": "Đóng thông báo cập nhật",
    "done": "Đã hiểu",
    "once": "Chỉ tự động hiện một lần cho phiên bản này.",
    "items": [
      {
        "title": "Soniox v6 cho cuộc họp và phụ đề",
        "detail": "Cập nhật model realtime lên v6. Khi dùng Soniox trong cuộc họp, bản chép và phụ đề dùng chung luồng âm thanh; lời gốc và bản dịch được lưu riêng."
      },
      {
        "title": "Chọn Groq và model rõ ràng hơn",
        "detail": "Cấu hình Groq đã lưu xuất hiện trong Cuộc họp, Chỉ phụ đề và Mẫu cấu hình. Chọn Whisper cho lời gốc; tên model có kèm tên cấu hình để phân biệt nhiều API key."
      },
      {
        "title": "Hiệu chỉnh bằng file ghi âm",
        "detail": "Sau khi kết thúc, bấm Hiệu chỉnh để model nghe lại bản thu và tạo Bản hiệu chỉnh riêng. Mặc định Moonshine Việt/Anh; phiên đa ngôn ngữ dùng Nemotron đã tải. Có thể đổi sang Groq hoặc Soniox async."
      },
      {
        "title": "Mẫu gọn hơn, chọn AI khi cần",
        "detail": "Mẫu chỉ cấu hình chép lời và phụ đề khi đang ghi. Bấm Hiệu chỉnh hoặc Tóm tắt để chọn model và dự phòng cho lần chạy đó. Tóm tắt có thể dùng Bản gốc hoặc Bản hiệu chỉnh; dữ liệu cũ được giữ nguyên."
      }
    ]
  },
  "en": {
    "title": "What’s new in Innote?",
    "subtitle": "Clearer AI choices and correction from your recording.",
    "close": "Close update notes",
    "done": "Got it",
    "once": "Shown automatically once for this version.",
    "items": [
      {
        "title": "Soniox v6 for meetings and captions",
        "detail": "The realtime model is updated to v6. Soniox meeting transcripts and captions share one audio stream; original text and translations are saved separately."
      },
      {
        "title": "Clearer Groq and model selection",
        "detail": "Saved Groq profiles are available in Meetings, Captions only and Presets. Choose Whisper for original speech; model labels include profile names to distinguish multiple API keys."
      },
      {
        "title": "Correct from the recorded audio",
        "detail": "After ending, choose Correction to transcribe the recording into a separate corrected transcript. Defaults use Vietnamese/English Moonshine or downloaded Nemotron for multilingual meetings. Groq and Soniox async remain selectable."
      },
      {
        "title": "Simpler presets, AI choices when needed",
        "detail": "Presets configure live transcription and captions. Choose a model and optional backup when opening Correction or Summary. Summaries can use the original or corrected transcript; existing data is preserved."
      }
    ]
  },
  "ko": {
    "title": "Innote의 새로운 기능",
    "subtitle": "더 명확한 AI 선택과 녹음 기반 교정.",
    "close": "업데이트 안내 닫기",
    "done": "확인",
    "once": "이 버전에서 한 번만 자동으로 표시됩니다.",
    "items": [
      {
        "title": "회의와 자막용 Soniox v6",
        "detail": "실시간 모델을 v6로 업데이트했습니다. Soniox 회의 전사와 자막은 하나의 오디오 스트림을 공유하며 원문과 번역을 따로 저장합니다."
      },
      {
        "title": "더 명확한 Groq와 모델 선택",
        "detail": "저장된 Groq 설정을 회의, 자막 전용, 프리셋에서 선택할 수 있습니다. 원문 전사에는 Whisper를 사용하고 모델 이름 앞에 설정 이름을 표시합니다."
      },
      {
        "title": "녹음 파일로 전사 교정",
        "detail": "회의 종료 후 교정을 눌러 녹음을 다시 인식하고 별도의 교정본을 만듭니다. 베트남어·영어는 Moonshine, 다국어 회의는 다운로드한 Nemotron이 기본입니다. Groq 또는 Soniox async로 바꿀 수 있습니다."
      },
      {
        "title": "간결한 프리셋과 작업별 AI 선택",
        "detail": "프리셋은 실시간 전사와 자막만 설정합니다. 교정 또는 요약을 열 때 모델과 선택적 대체 모델을 고르세요. 요약에는 원본이나 교정본을 사용할 수 있으며 기존 데이터는 유지됩니다."
      }
    ]
  },
  "zh": {
    "title": "Innote 更新了什么？",
    "subtitle": "更清晰的 AI 选择，直接听录音校正转录。",
    "close": "关闭更新说明",
    "done": "知道了",
    "once": "此版本仅自动显示一次。",
    "items": [
      {
        "title": "会议与字幕升级 Soniox v6",
        "detail": "实时模型升级至 v6。Soniox 会议转录和字幕共用一路音频，原文与译文分别保存。"
      },
      {
        "title": "更清晰地选择 Groq 和模型",
        "detail": "已保存的 Groq 配置可在会议、仅字幕和预设中选择。原文转录可选 Whisper；模型名称包含配置名称，便于区分多个 API key。"
      },
      {
        "title": "根据录音文件校正转录",
        "detail": "结束会议后点击校正，重新识别录音并生成独立的校正版。越南语、英语默认使用 Moonshine，多语言会议使用已下载的 Nemotron；也可改选 Groq 或 Soniox async。"
      },
      {
        "title": "精简预设，按需选择 AI",
        "detail": "预设只配置录音中的转录和字幕。打开校正或摘要时再选择模型及可选备用模型。摘要可使用原文或校正版，已有数据保持不变。"
      }
    ]
  }
};
export const previewReleaseVersion = "0.8.8";
