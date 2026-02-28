import { TandemClient } from "@frumu/tandem-client";

// DOM Elements
const statusIndicator = document.getElementById("status-indicator");
const statusText = document.getElementById("status-text");
const engineVersion = document.getElementById("engine-version");

const viewLoading = document.getElementById("view-loading");
const viewProviders = document.getElementById("view-providers");

const providerForm = document.getElementById("provider-form");
const saveStatus = document.getElementById("save-status");

// Tandem SDK Client
// We connect to the localhost port defined in setup.js where the engine is spawned
const client = new TandemClient({
    baseUrl: "http://127.0.0.1:39731"
});

async function checkHealth() {
    try {
        const response = await fetch("http://127.0.0.1:39731/global/health");
        if (response.ok) {
            const data = await response.json();
            return data;
        }
        return false;
    } catch (e) {
        return false;
    }
}

async function init() {
    // 1. Poll until the Engine is alive
    let isHealthy = false;
    let attempts = 0;

    while (!isHealthy && attempts < 20) {
        let health = await checkHealth();
        if (health && health.status === "ok") {
            isHealthy = true;
            statusIndicator.className = "w-2.5 h-2.5 rounded-full bg-emerald-500 shadow-[0_0_10px_rgba(16,185,129,0.8)]";
            statusText.textContent = "Engine Connected";
            statusText.className = "text-sm font-bold tracking-wide text-emerald-400";
            engineVersion.textContent = `v${health.version || '0.3.x'}`;
        } else {
            attempts++;
            await new Promise(r => setTimeout(r, 1000));
        }
    }

    if (!isHealthy) {
        statusIndicator.className = "w-2.5 h-2.5 rounded-full bg-red-500 shadow-[0_0_10px_rgba(239,68,68,0.8)]";
        statusText.textContent = "Connection Failed";
        statusText.className = "text-sm font-bold tracking-wide text-red-400";
        viewLoading.innerHTML = `<i data-feather="alert-circle" class="w-8 h-8 mb-4 text-red-500"></i><p class="text-sm">Could not find Tandem Engine at localhost:39731.</p>`;
        feather.replace();
        return;
    }

    // 2. Load Configuration via SDK
    try {
        const config = await client.config.get();
        // Pre-fill UI (Assuming the config endpoint returns provider keys or shapes)
        // Since we are setting Environment vars for keys, the config endpoint may obscure them.

        viewLoading.classList.add("hidden");
        viewProviders.classList.remove("hidden");
    } catch (e) {
        console.error("Failed to load generic config", e);
    }
}

providerForm.addEventListener("submit", async (e) => {
    e.preventDefault();

    const openrouter = document.getElementById("input-openrouter").value;
    const anthropic = document.getElementById("input-anthropic").value;
    const openai = document.getElementById("input-openai").value;

    const payload = {};
    if (openrouter) payload.OPENROUTER_API_KEY = openrouter;
    if (anthropic) payload.ANTHROPIC_API_KEY = anthropic;
    if (openai) payload.OPENAI_API_KEY = openai;

    // Send the keys back to the engine via SDK (hypothetical update endpoint)
    try {
        await client.config.update({ providers: payload });

        // Show success flash
        saveStatus.classList.remove("opacity-0");
        saveStatus.textContent = "Saved successfully!";
        setTimeout(() => saveStatus.classList.add("opacity-0"), 3000);
    } catch (err) {
        console.error("Failed to save keys", err);
        saveStatus.classList.remove("opacity-0", "text-emerald-400");
        saveStatus.classList.add("text-red-400");
        saveStatus.textContent = "Error saving config";
        setTimeout(() => {
            saveStatus.classList.add("opacity-0");
            saveStatus.classList.remove("text-red-400");
            saveStatus.classList.add("text-emerald-400");
        }, 3000);
    }
});

// Boot the app
init();
