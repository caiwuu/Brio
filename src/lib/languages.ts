/**
 * Browser 创建时可选的语言列表。
 * value 与 Chromium `--lang=` / Accept-Language 头保持一致。
 */
export interface LanguageOption {
  value: string;
  label: string;
  defaultTimezone: string;
}

export const LANGUAGE_OPTIONS: LanguageOption[] = [
  { value: "en-US", label: "English (US)", defaultTimezone: "America/Chicago" },
  { value: "en-GB", label: "English (UK)", defaultTimezone: "Europe/London" },
  { value: "ja-JP", label: "Japanese", defaultTimezone: "Asia/Tokyo" },
  { value: "zh-CN", label: "Chinese (Simplified)", defaultTimezone: "Asia/Shanghai" },
  { value: "zh-TW", label: "Chinese (Traditional)", defaultTimezone: "Asia/Taipei" },
  { value: "de-DE", label: "German", defaultTimezone: "Europe/Berlin" },
  { value: "fr-FR", label: "French", defaultTimezone: "Europe/Paris" },
  { value: "ko-KR", label: "Korean", defaultTimezone: "Asia/Seoul" },
];

export const DEFAULT_LANGUAGE = LANGUAGE_OPTIONS[0];

/** 时区 → 推荐语言。用于代理探测出时区后联动填充语言。 */
const TZ_LANGUAGE_RULES: [RegExp, string][] = [
  [/^Asia\/Tokyo$/, "ja-JP"],
  [/^Asia\/(Shanghai|Chongqing|Urumqi|Harbin)$/, "zh-CN"],
  [/^Asia\/(Taipei|Macau|Hong_Kong)$/, "zh-TW"],
  [/^Asia\/Seoul$/, "ko-KR"],
  [/^Europe\/(London|Belfast|Dublin)$/, "en-GB"],
  [/^Europe\/(Berlin|Vienna|Zurich|Busingen)$/, "de-DE"],
  [/^Europe\/(Paris|Brussels|Luxembourg|Monaco)$/, "fr-FR"],
];

/**
 * 根据探测到的时区推断语言。匹配不到已知规则时回退 en-US
 * （语言列表有限，英语是最通用的兜底）。
 */
export function languageForTimezone(tz: string): string {
  const trimmed = tz.trim();
  for (const [re, lang] of TZ_LANGUAGE_RULES) {
    if (re.test(trimmed)) return lang;
  }
  return DEFAULT_LANGUAGE.value;
}
