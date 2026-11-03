import type { UiLocale } from "../i18n/shared";
export interface ReleaseContent {
  title:string; subtitle:string; close:string; done:string; once:string;
  items:{title:string;detail:string}[];
}
// Draft for the next release. Version files stay unchanged until approval.
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
export const previewReleaseVersion = "0.8.6";
