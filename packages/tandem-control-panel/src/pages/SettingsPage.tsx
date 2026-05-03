import { useSettingsPageController } from "./SettingsPageController";
import { AnimatedPage, SplitView, StaggerGroup } from "../ui/index.tsx";
import { SettingsPageNavigationProvidersSections } from "./SettingsPageNavigationProvidersSections";
import { SettingsPageSearchIdentityThemeSections } from "./SettingsPageSearchIdentityThemeSections";
import { SettingsPageChannelsMcpSections } from "./SettingsPageChannelsMcpSections";
import { SettingsPageBugMonitorSections } from "./SettingsPageBugMonitorSections";
import { SettingsPageMaintenanceBrowserSections } from "./SettingsPageMaintenanceBrowserSections";
import { SettingsPageOverlays } from "./SettingsPageOverlays";
import type { AppPageProps } from "./pageTypes";

export function SettingsPage(props: AppPageProps) {
  const controller = useSettingsPageController(props);
  const { rootRef, sectionTabs, activeSection, setActiveSection } = controller;

  return (
    <AnimatedPage className="grid gap-4">
      <div ref={rootRef} className="grid gap-4">
        <div className="tcp-settings-tabs">
          {sectionTabs.map((section) => (
            <button
              key={section.id}
              type="button"
              className={`tcp-settings-tab tcp-settings-tab-underline ${
                activeSection === section.id ? "active" : ""
              }`}
              onClick={() => setActiveSection(section.id)}
            >
              <i data-lucide={section.icon}></i>
              {section.label}
            </button>
          ))}
        </div>

        <SplitView
          main={
            <StaggerGroup className="grid gap-4">
              <SettingsPageNavigationProvidersSections controller={controller} />
              <SettingsPageSearchIdentityThemeSections controller={controller} />
              <SettingsPageChannelsMcpSections controller={controller} />
              <SettingsPageBugMonitorSections controller={controller} />
              <SettingsPageMaintenanceBrowserSections controller={controller} />
            </StaggerGroup>
          }
        />

        <SettingsPageOverlays controller={controller} />
      </div>
    </AnimatedPage>
  );
}
