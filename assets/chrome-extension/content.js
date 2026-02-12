// Cratos AI Assistant — Content Script
// Provides DOM manipulation actions callable from the background service worker.
//
// SECURITY MODEL:
// - Server connection is API-key authenticated (gateway handshake).
// - host_permissions restricted to 127.0.0.1/localhost only.
// - The "evaluate" action mirrors Playwright page.evaluate() — it runs
//   server-authored JS in page context, which is the core value proposition
//   of browser automation tools. This is safe because only the authenticated
//   local Cratos server can invoke it.

(function () {
  "use strict";

  function querySelector(selector) {
    const el = document.querySelector(selector);
    if (!el) throw new Error(`Element not found: ${selector}`);
    return el;
  }

  /** Scroll element into view and return its viewport-center coordinates. */
  function elementCenter(el) {
    el.scrollIntoView({ block: "center", inline: "center" });
    const rect = el.getBoundingClientRect();
    return {
      x: Math.round(rect.x + rect.width / 2),
      y: Math.round(rect.y + rect.height / 2),
    };
  }

  const actions = {
    // click/type/fill/hover resolve element coordinates and return a CDP signal.
    // background.js dispatches real input via chrome.debugger (Input domain),
    // which is indistinguishable from user input and works with React/Vue/Angular.

    click(params) {
      const el = querySelector(params.selector);
      const { x, y } = elementCenter(el);
      return { use_cdp: "click", x, y };
    },

    type(params) {
      const el = querySelector(params.selector);
      const { x, y } = elementCenter(el);
      return { use_cdp: "type", x, y, text: params.text || "" };
    },

    fill(params) {
      const el = querySelector(params.selector);
      const { x, y } = elementCenter(el);
      return { use_cdp: "fill", x, y, value: params.value || "" };
    },

    get_text(params) {
      const el = querySelector(params.selector);
      return { text: el.innerText };
    },

    get_html(params) {
      const sel = params.selector || "html";
      const el = querySelector(sel);
      const outer = params.outer !== false;
      let html = outer ? el.outerHTML : el.innerHTML;
      if (html.length > 15000) {
        html = html.substring(0, 15000) + "\n... (truncated)";
      }
      return { html };
    },

    get_attribute(params) {
      const el = querySelector(params.selector);
      return { value: el.getAttribute(params.attribute) };
    },

    scroll(params) {
      if (params.selector) {
        const el = querySelector(params.selector);
        el.scrollBy(params.x || 0, params.y || 0);
      } else {
        window.scrollBy(params.x || 0, params.y || 0);
      }
      return { ok: true };
    },

    hover(params) {
      const el = querySelector(params.selector);
      const { x, y } = elementCenter(el);
      return { use_cdp: "hover", x, y };
    },

    select(params) {
      const el = querySelector(params.selector);
      el.value = params.value;
      el.dispatchEvent(new Event("change", { bubbles: true }));
      return { ok: true };
    },

    check(params) {
      const el = querySelector(params.selector);
      el.checked = params.checked !== false;
      el.dispatchEvent(new Event("change", { bubbles: true }));
      return { ok: true };
    },

    get_url() {
      return { url: window.location.href };
    },

    get_title() {
      return { title: document.title };
    },

    wait_for_selector(params) {
      const timeout = params.timeout || 10000;
      return new Promise((resolve, reject) => {
        const start = Date.now();
        const check = () => {
          if (document.querySelector(params.selector)) {
            resolve({ found: true });
          } else if (Date.now() - start > timeout) {
            reject(new Error(`Timeout waiting for: ${params.selector}`));
          } else {
            setTimeout(check, 100);
          }
        };
        check();
      });
    },

    // Equivalent to Playwright page.evaluate(). Runs server-authored JS.
    // Protected by API-key auth + localhost-only host_permissions.
    evaluate(params) {
      try {
        // Intentional: browser automation requires JS execution in page context
        const fn = Function("return (" + params.script + ")()"); // NOSONAR
        return { result: fn() };
      } catch (e) {
        if (
          e.message &&
          (e.message.includes("unsafe-eval") ||
            e.message.includes("Content Security Policy"))
        ) {
          // Signal background.js to retry via CDP (chrome.debugger)
          return { ok: false, csp_blocked: true, error: "CSP blocks eval" };
        }
        throw e;
      }
    },

    get_page_context() {
      const sel = window.getSelection();
      return {
        url: window.location.href,
        title: document.title,
        selectedText: sel ? sel.toString() : "",
        meta: {
          description:
            document.querySelector('meta[name="description"]')?.content || "",
          keywords:
            document.querySelector('meta[name="keywords"]')?.content || "",
        },
      };
    },

    get_element_rect(params) {
      const el = querySelector(params.selector);
      const rect = el.getBoundingClientRect();
      return {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
        top: rect.top,
        left: rect.left,
      };
    },
  };

  // Expose global function for background.js executeScript calls
  window.__cratos_exec_action = function (params) {
    const actionName = params.action;
    const handler = actions[actionName];
    if (!handler) {
      throw new Error(`Unknown action: ${actionName}`);
    }
    return handler(params);
  };

  // Also listen for chrome.runtime messages (alternative path)
  chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
    if (message.type !== "exec_action") return false;
    try {
      const result = actions[message.params.action](message.params);
      if (result instanceof Promise) {
        result
          .then((r) => sendResponse({ ok: true, result: r }))
          .catch((e) => sendResponse({ ok: false, error: e.message }));
        return true;
      }
      sendResponse({ ok: true, result });
    } catch (e) {
      sendResponse({ ok: false, error: e.message });
    }
    return false;
  });
})();
