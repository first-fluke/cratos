// Cratos AI — Side Panel Chat UI

const messagesEl = document.getElementById("messages");
const inputEl = document.getElementById("input");
const sendBtn = document.getElementById("send-btn");
const statusDot = document.getElementById("status-dot");
const statusText = document.getElementById("status-text");
const contextBar = document.getElementById("context-bar");

// ── Status polling ──────────────────────────────────────────────────

function updateStatus() {
  chrome.runtime.sendMessage({ type: "get_status" }, (resp) => {
    if (chrome.runtime.lastError || !resp) {
      statusDot.className = "disconnected";
      statusText.textContent = "Extension error";
      return;
    }
    if (resp.connected) {
      statusDot.className = "connected";
      statusText.textContent = resp.serverUrl || "Connected";
    } else {
      statusDot.className = "disconnected";
      statusText.textContent = "Disconnected";
    }
  });
}

updateStatus();
setInterval(updateStatus, 3000);

// ── Tab context ─────────────────────────────────────────────────────

async function getCurrentTabContext() {
  try {
    const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
    if (tab) {
      contextBar.textContent = tab.title || tab.url || "";
      return { url: tab.url, title: tab.title };
    }
  } catch {
    // Side panel may not have tabs access in some contexts
  }
  return null;
}

getCurrentTabContext();

// ── Messages ────────────────────────────────────────────────────────

function addMessage(text, role) {
  const div = document.createElement("div");
  div.className = `msg ${role}`;
  div.textContent = text;
  messagesEl.appendChild(div);
  messagesEl.scrollTop = messagesEl.scrollHeight;
}

// ── Send ─────────────────────────────────────────────────────────────

async function send() {
  const text = inputEl.value.trim();
  if (!text) return;

  inputEl.value = "";
  inputEl.style.height = "auto";
  sendBtn.disabled = true;

  addMessage(text, "user");

  const context = await getCurrentTabContext();

  chrome.runtime.sendMessage(
    { type: "chat_send", text, context },
    (resp) => {
      sendBtn.disabled = false;
      if (chrome.runtime.lastError) {
        addMessage("Failed to send message", "system");
        return;
      }
      if (resp && resp.ok && resp.result) {
        const reply = resp.result.text || resp.result.message || JSON.stringify(resp.result);
        addMessage(reply, "assistant");
      } else if (resp && resp.error) {
        addMessage(`Error: ${resp.error}`, "system");
      } else {
        addMessage("No response from server", "system");
      }
    }
  );
}

sendBtn.addEventListener("click", send);

inputEl.addEventListener("keydown", (e) => {
  if (e.key === "Enter" && !e.shiftKey) {
    e.preventDefault();
    send();
  }
});

// Auto-resize textarea
inputEl.addEventListener("input", () => {
  inputEl.style.height = "auto";
  inputEl.style.height = Math.min(inputEl.scrollHeight, 120) + "px";
});

// ── Listen for events from background ────────────────────────────────

chrome.runtime.onMessage.addListener((message) => {
  if (message.type === "event") {
    // Show relevant events
    if (message.event === "assistant_message" || message.event === "chat_response") {
      const text = message.data?.text || message.data?.message || JSON.stringify(message.data);
      addMessage(text, "assistant");
    }
  }
  if (message.type === "chat_response") {
    const text = message.result?.text || message.result?.message || JSON.stringify(message.result);
    addMessage(text, "assistant");
  }
});

// ── Welcome ──────────────────────────────────────────────────────────

addMessage("Cratos AI ready. Type a message to start.", "system");
