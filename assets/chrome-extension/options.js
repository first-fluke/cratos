// Cratos AI — Options page

const serverUrlInput = document.getElementById("server-url");
const apiKeyInput = document.getElementById("api-key");
const saveBtn = document.getElementById("save-btn");
const testBtn = document.getElementById("test-btn");
const statusEl = document.getElementById("status");

// Load saved settings
chrome.storage.local.get(["serverUrl", "apiKey"], (result) => {
  serverUrlInput.value = result.serverUrl || "ws://127.0.0.1:19527/ws/gateway";
  apiKeyInput.value = result.apiKey || "";
});

// Save
saveBtn.addEventListener("click", () => {
  const serverUrl = serverUrlInput.value.trim();
  const apiKey = apiKeyInput.value.trim();

  chrome.storage.local.set({ serverUrl, apiKey }, () => {
    statusEl.className = "ok";
    statusEl.textContent = "Settings saved.";
    // Trigger reconnect
    chrome.runtime.sendMessage({ type: "reconnect" });
    setTimeout(() => { statusEl.textContent = ""; }, 2000);
  });
});

// Test connection
testBtn.addEventListener("click", () => {
  statusEl.className = "";
  statusEl.textContent = "Testing...";

  const serverUrl = serverUrlInput.value.trim();
  const apiKey = apiKeyInput.value.trim();

  if (!apiKey) {
    statusEl.className = "err";
    statusEl.textContent = "API key is required.";
    return;
  }

  // Convert ws:// to http:// for health check
  const httpUrl = serverUrl
    .replace(/^ws:\/\//, "http://")
    .replace(/^wss:\/\//, "https://")
    .replace(/\/ws\/gateway$/, "/health");

  fetch(httpUrl, { signal: AbortSignal.timeout(5000) })
    .then((resp) => {
      if (resp.ok) {
        statusEl.className = "ok";
        statusEl.textContent = "Server reachable. Save settings and badge will update.";
      } else {
        statusEl.className = "err";
        statusEl.textContent = `Server responded with ${resp.status}`;
      }
    })
    .catch((err) => {
      statusEl.className = "err";
      statusEl.textContent = `Connection failed: ${err.message}`;
    });
});
