#!/usr/bin/env node
/**
 * MCP server for interactive AI agent sessions.
 *
 * Manages long-running CLI processes (Claude Code, Codex, Gemini CLI,
 * Antigravity) via stdin/stdout pipes, exposing them as MCP tools:
 *
 *   agent_start  — spawn a new session, returns session_id
 *   agent_send   — write to session stdin
 *   agent_output — read incremental stdout/stderr
 *   agent_stop   — kill session
 */

import { spawn } from "node:child_process";
import { randomUUID } from "node:crypto";
import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import {
  CallToolRequestSchema,
  ListToolsRequestSchema,
} from "@modelcontextprotocol/sdk/types.js";

// ── Agent presets ──────────────────────────────────────────────────
const AGENTS = {
  claude: { command: "claude", args: ["--chat"] },
  codex: { command: "codex", args: [] },
  gemini: { command: "gemini", args: ["--chat"] },
  antigravity: { command: "ag", args: ["chat"] },
};

// ── Session store ──────────────────────────────────────────────────
/** @type {Map<string, {proc: import("child_process").ChildProcess, buffer: string, agent: string, cwd?: string}>} */
const sessions = new Map();

const MAX_SESSIONS = 5;
const MAX_BUFFER = 128 * 1024; // 128 KB ring buffer per session
const SESSION_TIMEOUT_MS = 30 * 60 * 1000; // 30 min idle timeout

// ── Helpers ────────────────────────────────────────────────────────
function trimBuffer(buf) {
  if (buf.length > MAX_BUFFER) {
    return buf.slice(buf.length - MAX_BUFFER);
  }
  return buf;
}

/** Kill a session and clean up. */
function killSession(id) {
  const s = sessions.get(id);
  if (!s) return false;
  try {
    s.proc.kill("SIGTERM");
  } catch {
    // already dead
  }
  sessions.delete(id);
  return true;
}

// ── MCP Server ─────────────────────────────────────────────────────
const server = new Server(
  { name: "agent-sessions", version: "0.1.0" },
  { capabilities: { tools: {} } }
);

// List tools
server.setRequestHandler(ListToolsRequestSchema, async () => ({
  tools: [
    {
      name: "agent_start",
      description:
        "Start an interactive AI agent session. Returns a session_id for subsequent calls.",
      inputSchema: {
        type: "object",
        properties: {
          agent: {
            type: "string",
            enum: Object.keys(AGENTS),
            description: "Which agent to start",
          },
          workspace: {
            type: "string",
            description: "Optional working directory",
          },
        },
        required: ["agent"],
      },
    },
    {
      name: "agent_send",
      description: "Send input text to a running agent session.",
      inputSchema: {
        type: "object",
        properties: {
          session_id: { type: "string", description: "Session ID" },
          text: { type: "string", description: "Text to send via stdin" },
        },
        required: ["session_id", "text"],
      },
    },
    {
      name: "agent_output",
      description:
        "Read accumulated output from a session since the last read. Returns stdout content and whether the process is still alive.",
      inputSchema: {
        type: "object",
        properties: {
          session_id: { type: "string", description: "Session ID" },
        },
        required: ["session_id"],
      },
    },
    {
      name: "agent_stop",
      description: "Stop a running agent session.",
      inputSchema: {
        type: "object",
        properties: {
          session_id: { type: "string", description: "Session ID" },
        },
        required: ["session_id"],
      },
    },
  ],
}));

// Handle tool calls
server.setRequestHandler(CallToolRequestSchema, async (request) => {
  const { name, arguments: args } = request.params;

  switch (name) {
    // ── agent_start ──────────────────────────────────────────────
    case "agent_start": {
      if (sessions.size >= MAX_SESSIONS) {
        return {
          content: [
            {
              type: "text",
              text: `Error: maximum ${MAX_SESSIONS} concurrent sessions reached. Stop an existing session first.`,
            },
          ],
          isError: true,
        };
      }

      const preset = AGENTS[args.agent];
      if (!preset) {
        return {
          content: [
            {
              type: "text",
              text: `Unknown agent "${args.agent}". Supported: ${Object.keys(AGENTS).join(", ")}`,
            },
          ],
          isError: true,
        };
      }

      const id = randomUUID().slice(0, 8);
      const spawnOpts = {
        stdio: ["pipe", "pipe", "pipe"],
        env: { ...process.env },
      };
      if (args.workspace) {
        spawnOpts.cwd = args.workspace;
      }

      let proc;
      try {
        proc = spawn(preset.command, preset.args, spawnOpts);
      } catch (err) {
        return {
          content: [
            {
              type: "text",
              text: `Failed to start ${args.agent}: ${err.message}`,
            },
          ],
          isError: true,
        };
      }

      const session = {
        proc,
        buffer: "",
        agent: args.agent,
        cwd: args.workspace,
      };

      proc.stdout?.on("data", (chunk) => {
        session.buffer = trimBuffer(session.buffer + chunk.toString());
      });
      proc.stderr?.on("data", (chunk) => {
        session.buffer = trimBuffer(
          session.buffer + "[stderr] " + chunk.toString()
        );
      });
      proc.on("exit", (code) => {
        session.buffer += `\n[process exited with code ${code}]`;
      });

      // Idle timeout
      const timer = setTimeout(() => killSession(id), SESSION_TIMEOUT_MS);
      proc.on("exit", () => clearTimeout(timer));

      sessions.set(id, session);

      return {
        content: [
          {
            type: "text",
            text: JSON.stringify({
              session_id: id,
              agent: args.agent,
              pid: proc.pid,
            }),
          },
        ],
      };
    }

    // ── agent_send ───────────────────────────────────────────────
    case "agent_send": {
      const s = sessions.get(args.session_id);
      if (!s) {
        return {
          content: [
            {
              type: "text",
              text: `Session "${args.session_id}" not found.`,
            },
          ],
          isError: true,
        };
      }
      try {
        s.proc.stdin.write(args.text + "\n");
      } catch (err) {
        return {
          content: [
            { type: "text", text: `Failed to write to stdin: ${err.message}` },
          ],
          isError: true,
        };
      }
      return {
        content: [{ type: "text", text: "OK" }],
      };
    }

    // ── agent_output ─────────────────────────────────────────────
    case "agent_output": {
      const s = sessions.get(args.session_id);
      if (!s) {
        return {
          content: [
            {
              type: "text",
              text: `Session "${args.session_id}" not found.`,
            },
          ],
          isError: true,
        };
      }
      const output = s.buffer;
      s.buffer = ""; // drain
      const alive = !s.proc.killed && s.proc.exitCode === null;
      return {
        content: [
          {
            type: "text",
            text: JSON.stringify({ output, alive, agent: s.agent }),
          },
        ],
      };
    }

    // ── agent_stop ───────────────────────────────────────────────
    case "agent_stop": {
      const ok = killSession(args.session_id);
      return {
        content: [
          {
            type: "text",
            text: ok
              ? `Session "${args.session_id}" stopped.`
              : `Session "${args.session_id}" not found.`,
          },
        ],
        isError: !ok,
      };
    }

    default:
      return {
        content: [{ type: "text", text: `Unknown tool: ${name}` }],
        isError: true,
      };
  }
});

// ── Start ──────────────────────────────────────────────────────────
const transport = new StdioServerTransport();
await server.connect(transport);
