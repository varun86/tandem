import { useState, useRef, useEffect } from "react";
import { ChevronDown, Bot } from "lucide-react";
import { motion, AnimatePresence } from "framer-motion";
import { cn } from "@/lib/utils";
import type { ModelInfo } from "@/lib/tauri";

interface ModelSelectorProps {
  selectedModel?: string;
  onModelChange: (model: string) => void;
  models: ModelInfo[];
  disabled?: boolean;
}

export function ModelSelector({
  selectedModel,
  onModelChange,
  models,
  disabled,
}: ModelSelectorProps) {
  const [isOpen, setIsOpen] = useState(false);
  const dropdownRef = useRef<HTMLDivElement>(null);

  const currentModel = models.find((m) => m.id === selectedModel);
  const displayName = currentModel?.name || "Select Model";

  // Close dropdown when clicking outside
  useEffect(() => {
    const handleClickOutside = (event: globalThis.MouseEvent) => {
      const target = event.target;
      if (!dropdownRef.current || !(target instanceof globalThis.Node)) return;
      if (!dropdownRef.current.contains(target)) {
        setIsOpen(false);
      }
    };

    if (isOpen) {
      document.addEventListener("mousedown", handleClickOutside);
      return () => document.removeEventListener("mousedown", handleClickOutside);
    }
  }, [isOpen]);

  const handleSelect = (modelId: string) => {
    onModelChange(modelId);
    setIsOpen(false);
  };

  // Group models by provider
  const modelsByProvider = models.reduce(
    (acc, model) => {
      const provider = model.provider || "Other";
      if (!acc[provider]) {
        acc[provider] = [];
      }
      acc[provider].push(model);
      return acc;
    },
    {} as Record<string, ModelInfo[]>
  );

  return (
    <div className="relative" ref={dropdownRef}>
      <button
        type="button"
        onClick={() => setIsOpen(!isOpen)}
        disabled={disabled}
        className={cn(
          "flex h-8 items-center gap-1.5 rounded-md px-2 text-xs font-medium transition-colors",
          disabled
            ? "cursor-not-allowed opacity-50"
            : "hover:bg-surface text-text-muted hover:text-text",
          isOpen && "bg-surface text-text"
        )}
        title={currentModel?.name || "Select model"}
      >
        <Bot className="h-3.5 w-3.5" />
        <span className="max-w-[120px] truncate">{displayName}</span>
        <ChevronDown className={cn("h-3 w-3 transition-transform", isOpen && "rotate-180")} />
      </button>

      <AnimatePresence>
        {isOpen && (
          <motion.div
            initial={{ opacity: 0, y: 10 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: 10 }}
            transition={{ duration: 0.15 }}
            className="absolute left-0 bottom-full z-50 mb-2 w-72 max-h-96 overflow-y-auto rounded-lg border border-border bg-surface-elevated shadow-lg"
          >
            <div className="p-2 border-b border-border">
              <p className="text-xs font-medium text-text">Select Model</p>
              <p className="text-[10px] text-text-muted">Choose AI model for this session</p>
            </div>
            <div className="p-1">
              {Object.entries(modelsByProvider).map(([provider, providerModels]) => (
                <div key={provider}>
                  <div className="px-2 py-1.5 text-[10px] font-semibold text-text-muted uppercase tracking-wider">
                    {provider}
                  </div>
                  {providerModels.map((model) => {
                    const isSelected = model.id === selectedModel;

                    return (
                      <button
                        key={model.id}
                        type="button"
                        onClick={() => handleSelect(model.id)}
                        className={cn(
                          "flex w-full items-center justify-between gap-2 rounded-md px-2 py-1.5 text-left transition-colors",
                          isSelected ? "bg-primary/10 text-primary" : "text-text hover:bg-surface"
                        )}
                      >
                        <div className="flex-1 min-w-0">
                          <div className="text-xs font-medium truncate">{model.name}</div>
                          {model.context_length && (
                            <div className="text-[10px] text-text-muted">
                              {(model.context_length / 1000).toFixed(0)}K context
                            </div>
                          )}
                        </div>
                        {isSelected && <span className="text-primary text-[10px]">✓</span>}
                      </button>
                    );
                  })}
                </div>
              ))}
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
