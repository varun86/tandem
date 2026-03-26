import interactionPlugin, { type DateClickArg } from "@fullcalendar/interaction";
import FullCalendar from "@fullcalendar/react";
import timeGridPlugin from "@fullcalendar/timegrid";
import type {
  DatesSetArg,
  EventClickArg,
  EventContentArg,
  EventDropArg,
  MoreLinkArg,
} from "@fullcalendar/core";
import { useMemo, useRef, useState } from "react";

type CalendarRange = {
  startMs: number;
  endMs: number;
};

type AutomationCalendarProps = {
  events: any[];
  onRangeChange: (range: CalendarRange) => void;
  onOpenAutomation: (automation: any) => void;
  onEventDrop: (info: EventDropArg) => void | Promise<void>;
  statusColor: (status: string) => string;
};

const DEFAULT_DAY_SCROLL_TIME = "09:00:00";

function pad2(value: number) {
  return String(value).padStart(2, "0");
}

function startOfUtcMinute(date: Date) {
  return new Date(
    Date.UTC(
      date.getUTCFullYear(),
      date.getUTCMonth(),
      date.getUTCDate(),
      date.getUTCHours(),
      date.getUTCMinutes(),
      0,
      0
    )
  );
}

function toScrollTime(date: Date) {
  return `${pad2(date.getUTCHours())}:${pad2(date.getUTCMinutes())}:00`;
}

function timeParts(scrollTime: string) {
  const [hourText, minuteText] = String(scrollTime || DEFAULT_DAY_SCROLL_TIME).split(":");
  return {
    hour: Number.parseInt(hourText || "9", 10) || 9,
    minute: Number.parseInt(minuteText || "0", 10) || 0,
  };
}

function formatFocusedSlot(date: Date | null) {
  if (!date) return "";
  return `${new Intl.DateTimeFormat("en-US", {
    weekday: "short",
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
    timeZone: "UTC",
  }).format(date)} UTC`;
}

export function AutomationCalendar({
  events,
  onRangeChange,
  onOpenAutomation,
  onEventDrop,
  statusColor,
}: AutomationCalendarProps) {
  const calendarRef = useRef<FullCalendar | null>(null);
  const pendingScrollTimeRef = useRef("");
  const [currentView, setCurrentView] = useState("timeGridWeek");
  const [focusedSlot, setFocusedSlot] = useState<Date | null>(null);

  const focusedSlotLabel = useMemo(() => formatFocusedSlot(focusedSlot), [focusedSlot]);

  const syncFocusedSlot = (date: Date | null) => {
    if (!date) {
      setFocusedSlot(null);
      pendingScrollTimeRef.current = "";
      return;
    }
    const normalized = startOfUtcMinute(date);
    setFocusedSlot(normalized);
    pendingScrollTimeRef.current = toScrollTime(normalized);
  };

  const focusDayAtTime = (date: Date) => {
    const normalized = startOfUtcMinute(date);
    syncFocusedSlot(normalized);
    const api = calendarRef.current?.getApi();
    if (!api) return;
    api.changeView("timeGridDay", normalized);
  };

  const handleDatesSet = (arg: DatesSetArg) => {
    setCurrentView(arg.view.type);
    onRangeChange({
      startMs: arg.start.getTime(),
      endMs: arg.end.getTime(),
    });
    if (arg.view.type !== "timeGridDay" || !pendingScrollTimeRef.current) return;
    window.requestAnimationFrame(() => {
      calendarRef.current?.getApi().scrollToTime(pendingScrollTimeRef.current);
      pendingScrollTimeRef.current = "";
    });
  };

  const handleEventClick = (arg: EventClickArg) => {
    arg.jsEvent?.preventDefault?.();
    onOpenAutomation(arg.event?.extendedProps?.automation);
  };

  const handleDateClick = (arg: DateClickArg) => {
    focusDayAtTime(arg.date);
  };

  const handleMoreLinkClick = (arg: MoreLinkArg) => {
    arg.jsEvent?.preventDefault?.();
    focusDayAtTime(arg.date);
  };

  const renderEventContent = (arg: EventContentArg) => {
    const status = String(arg?.event?.extendedProps?.status || "active").trim() || "active";
    const scheduleLabel = String(arg?.event?.extendedProps?.scheduleLabel || "").trim();
    return (
      <div className="flex h-full min-h-0 flex-col gap-0.5 overflow-hidden rounded-lg border border-slate-700/60 bg-slate-950/90 px-2 py-1 text-xs shadow-sm">
        <div className="flex items-center justify-between gap-2">
          <span className="truncate font-medium text-slate-100">
            {String(arg?.event?.title || "")}
          </span>
          <span className={statusColor(status)}>{status}</span>
        </div>
        <div className="truncate text-[11px] text-slate-400">{scheduleLabel}</div>
      </div>
    );
  };

  return (
    <div className="grid gap-3">
      <div className="flex flex-wrap items-start justify-between gap-2">
        <div className="grid gap-1">
          <p className="text-xs font-medium uppercase tracking-wide text-slate-500">
            Cron schedules
          </p>
          <div className="tcp-subtle text-xs">
            Drag a card to change when that automation fires. Click a day or crowded time slot to
            drill into a detailed day view. Only cron-based automations are shown here for now.
          </div>
        </div>
        <div className="flex flex-wrap items-center justify-end gap-2">
          {currentView === "timeGridDay" && focusedSlotLabel ? (
            <span className="tcp-badge-info">{focusedSlotLabel}</span>
          ) : null}
          <span className="tcp-badge-info">{events.length} scheduled items</span>
        </div>
      </div>
      <div className="overflow-hidden rounded-2xl border border-slate-800/80 bg-slate-950/35 p-2">
        <FullCalendar
          ref={calendarRef}
          plugins={[timeGridPlugin, interactionPlugin]}
          initialView="timeGridWeek"
          timeZone="UTC"
          firstDay={0}
          height="auto"
          expandRows
          nowIndicator
          editable
          eventStartEditable
          eventDurationEditable={false}
          eventOverlap
          slotEventOverlap
          allDaySlot={false}
          slotMinTime="00:00:00"
          slotMaxTime="24:00:00"
          scrollTimeReset={false}
          stickyHeaderDates
          navLinks
          headerToolbar={{
            left: "prev,next today",
            center: "title",
            right: "timeGridWeek,timeGridDay",
          }}
          buttonText={{
            timeGridWeek: "week",
            timeGridDay: "day",
            today: "today",
          }}
          views={{
            timeGridWeek: {
              eventMaxStack: 1,
            },
            timeGridDay: {
              slotDuration: "00:15:00",
              slotLabelInterval: "01:00:00",
            },
          }}
          events={events}
          datesSet={handleDatesSet}
          navLinkDayClick={(date) => {
            const fallbackTime = focusedSlot ? toScrollTime(focusedSlot) : DEFAULT_DAY_SCROLL_TIME;
            const { hour, minute } = timeParts(fallbackTime);
            syncFocusedSlot(
              new Date(
                Date.UTC(
                  date.getUTCFullYear(),
                  date.getUTCMonth(),
                  date.getUTCDate(),
                  hour,
                  minute,
                  0,
                  0
                )
              )
            );
          }}
          dateClick={handleDateClick}
          moreLinkClick={handleMoreLinkClick}
          moreLinkClassNames={() => ["tcp-calendar-more-link"]}
          moreLinkContent={(arg) => <span>+{arg.num} more</span>}
          eventClick={handleEventClick}
          eventDrop={onEventDrop}
          eventContent={renderEventContent}
          slotLaneClassNames={(arg) => {
            if (currentView !== "timeGridDay" || !focusedSlot || !arg.date) return [];
            return arg.date.getTime() === focusedSlot.getTime() ? ["tcp-calendar-slot-focus"] : [];
          }}
          eventClassNames={(arg) => [
            String(arg?.event?.extendedProps?.family || "legacy") === "v2"
              ? "tcp-calendar-event tcp-calendar-event-v2"
              : "tcp-calendar-event tcp-calendar-event-legacy",
          ]}
        />
      </div>
    </div>
  );
}
