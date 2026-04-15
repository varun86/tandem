import { useState } from "react";
import { AnimatedPage } from "../ui/index.tsx";
import { MyAutomations } from "./AutomationsPage";
import { PageCard } from "./ui";
import type { AppPageProps } from "./pageTypes";

export function RunsPage({ client, toast, navigate }: AppPageProps) {
  const [selectedRunId, setSelectedRunId] = useState("");

  return (
    <AnimatedPage className="grid h-full min-h-0 gap-4">
      <PageCard
        title="Run Overview"
        subtitle="Watch active and blocked runs together, then open the full debugger for any individual automation run."
        actions={
          <div className="flex flex-wrap items-center gap-2">
            <button className="tcp-btn h-8 px-3 text-xs" onClick={() => navigate("automations")}>
              <i data-lucide="bot"></i>
              Open automations
            </button>
          </div>
        }
        fullHeight
      >
        <MyAutomations
          client={client}
          toast={toast}
          navigate={navigate}
          viewMode="running"
          selectedRunId={selectedRunId}
          onSelectRunId={setSelectedRunId}
          onOpenRunningView={() => undefined}
          onOpenAdvancedEdit={() => undefined}
          defaultRunningSectionsOpen={{
            active: true,
            issues: true,
            history: true,
          }}
        />
      </PageCard>
    </AnimatedPage>
  );
}
