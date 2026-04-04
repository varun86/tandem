import { useId } from "react";
import { COMMON_TIMEZONES, detectBrowserTimezone } from "./timezone";

type TimezoneFieldProps = {
  value: string;
  onChange: (value: string) => void;
  error?: string;
  label?: string;
  hint?: string;
  allowBrowserDefault?: boolean;
  className?: string;
};

export function TimezoneField({
  value,
  onChange,
  error,
  label = "Timezone",
  hint = "Use an IANA timezone like Europe/Berlin.",
  allowBrowserDefault = true,
  className = "",
}: TimezoneFieldProps) {
  const datalistId = useId();
  const browserTimezone = detectBrowserTimezone();
  const currentValue = String(value || "").trim();
  const currentIsCommon = COMMON_TIMEZONES.includes(
    currentValue as (typeof COMMON_TIMEZONES)[number]
  );

  return (
    <div className={`grid gap-1 ${className}`}>
      <label className="text-xs text-slate-400">{label}</label>
      <div className="grid gap-2 md:grid-cols-[1fr_auto]">
        <select
          className="tcp-select font-mono text-sm"
          value={currentIsCommon ? currentValue : "__custom__"}
          onInput={(event) => {
            const next = (event.target as HTMLSelectElement).value;
            if (next === "__custom__") return;
            onChange(next);
          }}
        >
          {COMMON_TIMEZONES.map((timezone) => (
            <option key={timezone} value={timezone}>
              {timezone}
            </option>
          ))}
          <option value="__custom__" disabled>
            Custom / type below
          </option>
        </select>
        {allowBrowserDefault ? (
          <button
            type="button"
            className="tcp-btn h-10 px-3"
            onClick={() => onChange(browserTimezone)}
          >
            Use browser
          </button>
        ) : null}
      </div>
      <input
        className={`tcp-input font-mono text-sm ${error ? "border-red-500/60 text-red-100" : ""}`}
        list={datalistId}
        placeholder={browserTimezone}
        value={currentValue}
        onInput={(event) => onChange((event.target as HTMLInputElement).value)}
      />
      <div className="text-xs text-slate-500">{hint}</div>
      {error ? <div className="text-xs text-red-300">{error}</div> : null}
      <datalist id={datalistId}>
        {COMMON_TIMEZONES.map((timezone) => (
          <option key={timezone} value={timezone} />
        ))}
      </datalist>
    </div>
  );
}
