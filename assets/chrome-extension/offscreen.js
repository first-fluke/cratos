// Offscreen keep-alive: sends a ping to the service worker every 20 seconds
// to prevent Chrome MV3 from terminating it (30s idle timeout).
setInterval(() => {
  chrome.runtime.sendMessage({ type: "keepalive" }).catch(() => {});
}, 20000);
