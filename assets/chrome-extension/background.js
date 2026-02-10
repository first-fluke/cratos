// Cratos AI Assistant — Chrome Extension Background Service Worker
// Manages WebSocket connection to Cratos server, message routing, and badge state.

const DEFAULT_SERVER_URL = "ws://127.0.0.1:8090/ws/gateway";
const RECONNECT_DELAY_MS = 5000;
const REQUEST_TIMEOUT_MS = 30000;

let ws = null;
let connected = false;
let connectRequestId = null; // Track the connect handshake request ID
let pendingRequests = new Map(); // id -> { resolve, reject, timer }
let requestIdCounter = 0;

// ── Helpers ──────────────────────────────────────────────────────────

function nextId() {
  return `ext-${++requestIdCounter}-${Date.now()}`;
}

function setBadge(text, color) {
  chrome.action.setBadgeText({ text });
  chrome.action.setBadgeBackgroundColor({ color });
}

async function getSettings() {
  const result = await chrome.storage.local.get(["serverUrl", "apiKey"]);
  return {
    serverUrl: result.serverUrl || DEFAULT_SERVER_URL,
    apiKey: result.apiKey || "",
  };
}

// ── WebSocket connection ─────────────────────────────────────────────

async function connect() {
  if (ws && (ws.readyState === WebSocket.OPEN || ws.readyState === WebSocket.CONNECTING)) {
    return;
  }

  const { serverUrl, apiKey } = await getSettings();
  if (!apiKey) {
    setBadge("!", "#F44");
    console.warn("[cratos] No API key configured");
    return;
  }

  try {
    ws = new WebSocket(serverUrl);
  } catch (e) {
    console.error("[cratos] WebSocket creation failed:", e);
    setBadge("!", "#F44");
    scheduleReconnect();
    return;
  }

  ws.onopen = () => {
    console.log("[cratos] WebSocket opened, sending connect handshake");
    connectRequestId = nextId();
    const connectFrame = {
      frame: "request",
      id: connectRequestId,
      method: "connect",
      params: {
        token: apiKey,
        client: { name: "cratos-chrome", version: "0.1.0" },
        role: "browser",
        protocol_version: 1,
      },
    };
    ws.send(JSON.stringify(connectFrame));
  };

  ws.onmessage = (event) => {
    let frame;
    try {
      frame = JSON.parse(event.data);
    } catch {
      console.warn("[cratos] Invalid frame:", event.data);
      return;
    }
    handleFrame(frame);
  };

  ws.onclose = () => {
    console.log("[cratos] WebSocket closed");
    connected = false;
    connectRequestId = null;
    setBadge("OFF", "#888");
    rejectAllPending("Connection closed");
    scheduleReconnect();
  };

  ws.onerror = (err) => {
    console.error("[cratos] WebSocket error:", err);
    setBadge("!", "#F44");
  };
}

function scheduleReconnect() {
  setTimeout(() => connect(), RECONNECT_DELAY_MS);
}

function rejectAllPending(reason) {
  for (const [id, entry] of pendingRequests) {
    clearTimeout(entry.timer);
    entry.reject(new Error(reason));
  }
  pendingRequests.clear();
}

// ── Frame handling ──────────────────────────────────────────────────

function handleFrame(frame) {
  // Application-level keep-alive from server — respond to keep WS active
  if (frame.frame === "ping") {
    if (ws && ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify({ frame: "pong" }));
    }
    return;
  }

  if (frame.frame === "response") {
    // Check if this is the connect handshake response (match by tracked ID)
    if (!connected && connectRequestId && frame.id === connectRequestId) {
      connectRequestId = null;
      if (frame.error) {
        console.error("[cratos] Connect handshake failed:", frame.error.message);
        setBadge("!", "#F44");
        return;
      }
      connected = true;
      setBadge("ON", "#4A4");
      console.log("[cratos] Connected, session:", frame.result?.session_id);
      return;
    }

    // Match to a pending request from extension side
    const pending = pendingRequests.get(frame.id);
    if (pending) {
      clearTimeout(pending.timer);
      pendingRequests.delete(frame.id);
      if (frame.error) {
        pending.reject(new Error(frame.error.message || "Server error"));
      } else {
        pending.resolve(frame.result);
      }
    }
    return;
  }

  if (frame.frame === "request") {
    // Server → Extension request (browser.* methods)
    handleServerRequest(frame);
    return;
  }

  if (frame.frame === "event") {
    // Forward events to side panel
    broadcastToSidePanel({ type: "event", event: frame.event, data: frame.data });
  }
}

async function handleServerRequest(frame) {
  const { id, method, params } = frame;
  try {
    let result;
    switch (method) {
      case "browser.exec_action":
        result = await execAction(params);
        break;
      case "browser.get_tabs":
        result = await getTabs();
        break;
      case "browser.screenshot":
        result = await takeScreenshot(params);
        break;
      case "browser.navigate":
        result = await navigateTab(params);
        break;
      default:
        sendResponse(id, null, { code: "UNKNOWN_METHOD", message: `Unknown: ${method}` });
        return;
    }
    sendResponse(id, result, null);
  } catch (e) {
    sendResponse(id, null, { code: "INTERNAL_ERROR", message: e.message });
  }
}

function sendResponse(id, result, error) {
  if (!ws || ws.readyState !== WebSocket.OPEN) return;
  const frame = { frame: "response", id };
  if (error) {
    frame.error = error;
  } else {
    frame.result = result;
  }
  ws.send(JSON.stringify(frame));
}

// ── Send request from extension to server ────────────────────────────

function sendRequest(method, params) {
  return new Promise((resolve, reject) => {
    if (!ws || ws.readyState !== WebSocket.OPEN || !connected) {
      reject(new Error("Not connected to server"));
      return;
    }
    const id = nextId();
    const timer = setTimeout(() => {
      pendingRequests.delete(id);
      reject(new Error("Request timeout"));
    }, REQUEST_TIMEOUT_MS);

    pendingRequests.set(id, { resolve, reject, timer });
    ws.send(JSON.stringify({ frame: "request", id, method, params }));
  });
}

// ── Browser actions ──────────────────────────────────────────────────

async function execAction(params) {
  const tabs = await chrome.tabs.query({ active: true, lastFocusedWindow: true });
  if (!tabs.length) throw new Error("No active tab");
  const tabId = tabs[0].id;
  const tabUrl = tabs[0].url || "";

  // Can't execute on restricted pages
  if (tabUrl.startsWith("chrome://") || tabUrl.startsWith("chrome-extension://")) {
    throw new Error(`Cannot execute action on restricted page: ${tabUrl}`);
  }

  // Send message to content script (runs in extension's isolated world with DOM access)
  return new Promise((resolve, reject) => {
    chrome.tabs.sendMessage(tabId, { type: "exec_action", params }, (response) => {
      if (chrome.runtime.lastError) {
        // Content script not injected yet — inject it and retry once
        chrome.scripting.executeScript(
          { target: { tabId }, files: ["content.js"] },
          () => {
            if (chrome.runtime.lastError) {
              reject(new Error(chrome.runtime.lastError.message));
              return;
            }
            // Retry after injection
            setTimeout(() => {
              chrome.tabs.sendMessage(tabId, { type: "exec_action", params }, (resp2) => {
                if (chrome.runtime.lastError) {
                  reject(new Error(chrome.runtime.lastError.message));
                } else if (resp2 && !resp2.ok) {
                  reject(new Error(resp2.error || "Action failed"));
                } else {
                  resolve(resp2?.result ?? null);
                }
              });
            }, 100);
          }
        );
        return;
      }
      if (response && !response.ok) {
        reject(new Error(response.error || "Action failed"));
      } else {
        resolve(response?.result ?? null);
      }
    });
  });
}

async function getTabs() {
  const tabs = await chrome.tabs.query({});
  return {
    tabs: tabs.map((t) => ({
      id: t.id,
      url: t.url,
      title: t.title,
      active: t.active,
      windowId: t.windowId,
    })),
  };
}

async function takeScreenshot(params) {
  // Check if active tab is a restricted URL (can't capture)
  const [tab] = await chrome.tabs.query({ active: true, lastFocusedWindow: true });
  if (tab && tab.url && (tab.url.startsWith("chrome://") || tab.url.startsWith("chrome-extension://"))) {
    throw new Error(`Cannot capture screenshot of restricted page: ${tab.url}`);
  }
  const opts = { format: "png" };
  const dataUrl = await chrome.tabs.captureVisibleTab(null, opts);
  return { screenshot: dataUrl.replace(/^data:image\/png;base64,/, "") };
}

async function navigateTab(params) {
  const { url, tab_id } = params || {};
  if (!url) throw new Error("Missing url parameter");

  let tabId = tab_id;
  if (!tabId) {
    // Use lastFocusedWindow for service worker (no currentWindow in MV3 SW)
    const tabs = await chrome.tabs.query({ active: true, lastFocusedWindow: true });
    if (tabs.length) {
      const activeUrl = tabs[0].url || "";
      // Don't navigate restricted pages, create new tab instead
      if (activeUrl.startsWith("chrome://") || activeUrl.startsWith("chrome-extension://")) {
        const newTab = await chrome.tabs.create({ url, active: true });
        return { ok: true, tab_id: newTab.id };
      }
      tabId = tabs[0].id;
    }
  }

  if (tabId) {
    await chrome.tabs.update(tabId, { url, active: true });
    // Also focus the window containing the tab
    const tab = await chrome.tabs.get(tabId);
    if (tab.windowId) {
      await chrome.windows.update(tab.windowId, { focused: true });
    }
  } else {
    const newTab = await chrome.tabs.create({ url, active: true });
    return { ok: true, tab_id: newTab.id };
  }
  return { ok: true, tab_id: tabId };
}

// ── Side panel communication ─────────────────────────────────────────

function broadcastToSidePanel(message) {
  chrome.runtime.sendMessage(message).catch(() => {
    // Side panel not open, ignore
  });
}

chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
  if (message.type === "chat_send") {
    sendRequest("chat.send", { text: message.text, context: message.context })
      .then((result) => sendResponse({ ok: true, result }))
      .catch((err) => sendResponse({ ok: false, error: err.message }));
    return true; // keep channel open for async response
  }

  if (message.type === "get_status") {
    sendResponse({ connected, serverUrl: ws ? ws.url : null });
    return false;
  }

  if (message.type === "reconnect") {
    connect();
    sendResponse({ ok: true });
    return false;
  }

  if (message.type === "keepalive") {
    // Offscreen ping — just touching the SW keeps it alive
    sendResponse({ ok: true });
    return false;
  }
});

// ── Context menu ─────────────────────────────────────────────────────

chrome.runtime.onInstalled.addListener(() => {
  chrome.contextMenus.create({
    id: "cratos-send",
    title: "Cratos\uc5d0 \ubcf4\ub0b4\uae30",
    contexts: ["selection", "page"],
  });
});

chrome.contextMenus.onClicked.addListener(async (info, tab) => {
  if (info.menuItemId !== "cratos-send") return;
  const text = info.selectionText || "";
  const context = { url: tab?.url, title: tab?.title, selectedText: text };

  try {
    const result = await sendRequest("chat.send", {
      text: text || `Analyze this page: ${tab?.url}`,
      context,
    });
    broadcastToSidePanel({ type: "chat_response", result });
  } catch (e) {
    console.error("[cratos] Context menu send failed:", e);
  }
});

// ── Action icon → open side panel ────────────────────────────────────

chrome.action.onClicked.addListener((tab) => {
  chrome.sidePanel.open({ windowId: tab.windowId });
});

// ── Storage change → reconnect ───────────────────────────────────────

chrome.storage.onChanged.addListener((changes) => {
  if (changes.serverUrl || changes.apiKey) {
    if (ws) ws.close();
    connect();
  }
});

// ── MV3 Service Worker Keep-Alive ─────────────────────────────────────

// 1. Offscreen document sends "keepalive" messages every 20s to prevent
//    Chrome from terminating this service worker (30s idle timeout).
async function ensureOffscreen() {
  const contexts = await chrome.runtime.getContexts({
    contextTypes: ["OFFSCREEN_DOCUMENT"],
  });
  if (contexts.length === 0) {
    try {
      await chrome.offscreen.createDocument({
        url: "offscreen.html",
        reasons: ["WORKERS"],
        justification: "Keep service worker alive for WebSocket connection",
      });
    } catch (e) {
      // Already exists or not supported — ignore
      if (!e.message?.includes("Only a single offscreen")) {
        console.warn("[cratos] Offscreen creation failed:", e.message);
      }
    }
  }
}

// 2. Alarm-based fallback reconnect (fires every 25s)
//    In case offscreen is killed or not supported.
chrome.alarms.create("cratos-reconnect", { periodInMinutes: 25 / 60 });
chrome.alarms.onAlarm.addListener((alarm) => {
  if (alarm.name === "cratos-reconnect") {
    if (!ws || ws.readyState === WebSocket.CLOSED || ws.readyState === WebSocket.CLOSING) {
      console.log("[cratos] Alarm: WebSocket not open, reconnecting");
      connect();
    }
  }
});

// ── Init ─────────────────────────────────────────────────────────────

setBadge("OFF", "#888");
ensureOffscreen();
connect();
