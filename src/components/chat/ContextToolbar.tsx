import { AgentSelector } from "./AgentSelector";
import { ToolCategoryPicker } from "./ToolCategoryPicker";
import { ModelSelector } from "./ModelSelector";
import type { ModelInfo } from "@/lib/tauri";

interface ContextToolbarProps {
  // Agent
  selectedAgent?: string;
  onAgentChange?: (agent: string | undefined) => void;
  // Tools
  enabledToolCategories: Set<string>;
  onToolCategoriesChange: (categories: Set<string>) => void;
  // Model (optional for future use)
  selectedModel?: string;
  onModelChange?: (model: string) => void;
  availableModels?: ModelInfo[];
  // State
  disabled?: boolean;
}

export function ContextToolbar({
  selectedAgent,
  onAgentChange,
  enabledToolCategories,
  onToolCategoriesChange,
  selectedModel,
  onModelChange,
  availableModels,
  disabled,
}: ContextToolbarProps) {
  return (
    <div className="flex items-center gap-2 px-3 py-2 border-t border-border/50 bg-surface/30">
      {/* Agent selector */}
      {onAgentChange && (
        <AgentSelector
          selectedAgent={selectedAgent}
          onAgentChange={onAgentChange}
          disabled={disabled}
        />
      )}

      {/* Divider */}
      {onAgentChange && <div className="h-4 w-px bg-border" />}

      {/* Tool categories */}
      <ToolCategoryPicker
        enabledCategories={enabledToolCategories}
        onCategoriesChange={onToolCategoriesChange}
        onAgentChange={onAgentChange}
        selectedAgent={selectedAgent}
        disabled={disabled}
      />

      {/* Model selector (optional) */}
      {onModelChange && availableModels && availableModels.length > 0 && (
        <>
          {/* Divider */}
          <div className="h-4 w-px bg-border" />

          <ModelSelector
            selectedModel={selectedModel}
            onModelChange={onModelChange}
            models={availableModels}
            disabled={disabled}
          />
        </>
      )}

      {/* Spacer to push hints right */}
      <div className="flex-1" />

      {/* Compact hints */}
      <span className="text-[10px] text-text-subtle hidden sm:inline">
        Enter to send • Shift+Enter for newline
      </span>
    </div>
  );
}
