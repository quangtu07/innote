export type SpeakerColor = { fg: string; bg: string; border: string };

/** Distinct, low-saturation colors for transcript badges and panel dots. */
export const SPEAKER_COLORS: SpeakerColor[] = [
  { fg: "#146b56", bg: "#e6f0ea", border: "#b9d6c4" },
  { fg: "#2f5d8a", bg: "#e8f0f8", border: "#b9cfe3" },
  { fg: "#7a3f8a", bg: "#f2e8f4", border: "#d8bfe0" },
  { fg: "#9a4b22", bg: "#fbf0e5", border: "#e8c8ad" },
  { fg: "#a13a52", bg: "#f8e9ed", border: "#e5bcc7" },
  { fg: "#356d73", bg: "#e5f2f2", border: "#b8d9da" },
  { fg: "#6d5a1f", bg: "#f7f1dc", border: "#dfd0a4" },
  { fg: "#3d5f9a", bg: "#e8edf8", border: "#c2cce6" },
  { fg: "#1f6b3a", bg: "#e6f4e9", border: "#b8d9c2" },
  { fg: "#8a3b3b", bg: "#f8e8e8", border: "#e1bcbc" },
  { fg: "#5b4b8a", bg: "#eeebf8", border: "#cec7e5" },
  { fg: "#2d6b66", bg: "#e4f3f0", border: "#b6dcd5" },
  { fg: "#8a4b6b", bg: "#f5e9ef", border: "#e0c5d2" },
  { fg: "#4b628a", bg: "#e9eef7", border: "#c4cfe3" },
  { fg: "#6b6428", bg: "#f5f2dd", border: "#ddd6ac" },
  { fg: "#4f6b3a", bg: "#edf4e7", border: "#cadbbd" },
];

/** Stable per participant: sort IDs, not display names, so renaming keeps color. */
export function speakerColorFor(
  participants: { id: string }[],
  participantId?: string | null,
): SpeakerColor {
  if (!participantId) return SPEAKER_COLORS[0];
  const ids = participants.map((p) => p.id).sort();
  const index = ids.indexOf(participantId);
  return SPEAKER_COLORS[(index >= 0 ? index : 0) % SPEAKER_COLORS.length];
}