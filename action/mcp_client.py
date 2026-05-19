#!/usr/bin/env python3
"""
MCP Client + AI Orchestrator for Windows crash dump analysis.

This script acts as a lightweight MCP client that communicates with
mcp-windbg-rs (MCP Server) via stdio JSON-RPC 2.0, and orchestrates
an AI analysis loop using any OpenAI-compatible LLM API.

Dependencies: Python standard library only (no third-party packages).
"""

import json
import os
import re
import subprocess
import sys
import time
import urllib.request
import urllib.error

# ============================================================================
# Constants
# ============================================================================

DEFAULT_SYSTEM_PROMPT = """\
You are a Windows crash dump analysis expert with deep knowledge of Windows \
internals, debugging techniques, and common crash patterns. You have access to \
debugging tools via MCP (Model Context Protocol).

Your task is to analyze the provided crash dump or executable files using the \
available debugging tools and produce a comprehensive analysis report.

## Workflow

1. **Open the dump file** using `open_windbg_dump` with `include_stack_trace=true` \
to get an initial overview of the crash.
2. **Review the initial output** carefully — note the exception code, faulting \
module, and preliminary stack trace.
3. **Run additional debugging commands** using `run_windbg_cmd` as needed:
   - `!analyze -v` — Detailed automated crash analysis
   - `kb` — Stack backtrace with parameters
   - `~*k` — Stack traces for all threads
   - `lm` — List loaded modules
   - `!heap -s` — Heap summary (for heap corruption)
   - `.exr -1` — Exception record
   - `!locks` — Deadlock detection
   - `!peb` — Process environment block
   - Any other WinDbg commands relevant to the crash type
4. **For exe+pdb files**, use `launch_debug` to start the program under the \
debugger and observe the crash behavior.
5. **Synthesize your findings** into a structured report.

## Report Format

Your final report MUST be in Markdown format with these sections:

### Crash Summary
Brief one-paragraph description of what happened.

### Detailed Analysis
In-depth explanation of the crash mechanism, including exception type, \
faulting instruction, and relevant memory state.

### Stack Trace Analysis
Key stack frames and their significance. Identify the transition from \
system code to application code.

### Root Cause
Most likely root cause of the crash with supporting evidence from the \
debugging output.

### Recommendations
Actionable suggestions for fixing the issue or gathering more information.

## Guidelines
- Be thorough but concise.
- Always cite specific addresses, module names, and offsets from the debug output.
- If the dump is inconclusive, say so and suggest what additional information \
would help.
- Do NOT fabricate debugging output — only report what the tools actually return.
"""

# ============================================================================
# Exceptions
# ============================================================================


class McpError(Exception):
    """Error in MCP protocol communication."""
    pass


class McpToolError(Exception):
    """Error returned by an MCP tool execution."""
    pass


class LlmApiError(Exception):
    """Error calling the LLM API."""
    pass


# ============================================================================
# Utility Functions
# ============================================================================

def log(msg):
    """Print a timestamped log message to stderr."""
    ts = time.strftime("%H:%M:%S", time.localtime())
    print(f"[{ts}] {msg}", file=sys.stderr, flush=True)


def mcp_tools_to_openai_functions(tools):
    """Convert MCP tools/list result to OpenAI function calling format.

    MCP format:
        {"name": "...", "description": "...", "inputSchema": {...}}
    OpenAI format:
        {"type": "function", "function": {"name": "...", "description": "...", "parameters": {...}}}
    """
    result = []
    for tool in tools:
        fn = {
            "type": "function",
            "function": {
                "name": tool["name"],
                "description": tool.get("description", ""),
                "parameters": tool.get("inputSchema", {"type": "object", "properties": {}}),
            },
        }
        result.append(fn)
    return result


def build_user_message(file_paths):
    """Construct the user message listing files to analyze."""
    if not file_paths:
        return "No dump or executable files were found. Please check the download step."

    lines = ["Please analyze the following file(s):\n"]
    for fp in file_paths:
        ext = os.path.splitext(fp)[1].lower()
        label = {".dmp": "Crash dump", ".exe": "Executable", ".pdb": "Debug symbols"}.get(ext, "File")
        lines.append(f"- {label}: `{fp}`")

    lines.append("\nStart by opening the dump file (if present) or launching the executable under the debugger.")
    return "\n".join(lines)


# ============================================================================
# MCP Protocol Layer
# ============================================================================

class McpClient:
    """MCP stdio client — manages JSON-RPC 2.0 communication with an MCP Server."""

    def __init__(self, server_cmd, env=None):
        """
        Args:
            server_cmd: Command list to start the MCP server (e.g. ["./mcp-windbg-rs.exe"]).
            env: Environment variables dict for the subprocess.
        """
        self._cmd = server_cmd
        self._env = env
        self._proc = None
        self._next_id = 1

    # -- lifecycle -----------------------------------------------------------

    def start(self):
        """Start the MCP Server subprocess with stdio pipes."""
        log(f"Starting MCP server: {' '.join(self._cmd)}")
        try:
            self._proc = subprocess.Popen(
                self._cmd,
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                env=self._env,
            )
        except OSError as exc:
            raise McpError(f"Failed to start MCP server: {exc}") from exc
        log(f"MCP server started (pid={self._proc.pid})")

        # Start background thread to read and log stderr
        import threading
        def _read_stderr():
            try:
                for line in self._proc.stderr:
                    text = line.decode("utf-8", errors="replace").rstrip()
                    if text:
                        log(f"[MCP-SERVER] {text}")
            except Exception:
                pass
        self._stderr_thread = threading.Thread(target=_read_stderr, daemon=True)
        self._stderr_thread.start()

    def shutdown(self):
        """Terminate the MCP Server subprocess gracefully."""
        if self._proc is None:
            return
        log("Shutting down MCP server...")
        try:
            self._proc.stdin.close()
        except Exception:
            pass
        try:
            self._proc.wait(timeout=5)
            log(f"MCP server exited (code={self._proc.returncode}).")
        except subprocess.TimeoutExpired:
            log("MCP server did not exit in 5s, killing...")
            self._proc.kill()
            try:
                self._proc.wait(timeout=3)
            except Exception:
                pass
            log("MCP server killed.")

    # -- low-level transport -------------------------------------------------

    def _send_request(self, method, params=None):
        """Send a JSON-RPC 2.0 request over stdin using Content-Length framing.

        Returns the request id.
        """
        if self._proc is None or self._proc.poll() is not None:
            raise McpError("MCP server process is not running")

        req_id = self._next_id
        self._next_id += 1

        msg = {
            "jsonrpc": "2.0",
            "id": req_id,
            "method": method,
        }
        if params is not None:
            msg["params"] = params

        payload = json.dumps(msg).encode("utf-8")

        try:
            self._proc.stdin.write(payload + b"\n")
            self._proc.stdin.flush()
        except (BrokenPipeError, OSError) as exc:
            raise McpError(f"Failed to send request to MCP server: {exc}") from exc

        return req_id

    def _read_response(self, expected_id, timeout=60):
        """Read a JSON-RPC response with the expected id from stdout.

        Uses newline-delimited JSON. Skips notifications (messages without id).
        """
        deadline = time.time() + timeout

        while True:
            if time.time() > deadline:
                raise McpError(f"Timeout waiting for response id={expected_id}")

            if self._proc.poll() is not None:
                stderr_out = ""
                try:
                    stderr_out = self._proc.stderr.read().decode("utf-8", errors="replace")
                except Exception:
                    pass
                raise McpError(f"MCP server exited unexpectedly (code={self._proc.returncode}). stderr: {stderr_out[:1000]}")

            line = self._proc.stdout.readline()
            if not line:
                raise McpError("MCP server closed stdout unexpectedly")

            line_str = line.decode("utf-8", errors="replace").strip()
            if not line_str:
                continue

            try:
                msg = json.loads(line_str)
            except json.JSONDecodeError as exc:
                # Could be a partial line or non-JSON output, skip
                continue

            # Skip notifications (no id field)
            if "id" not in msg:
                continue

            if msg["id"] == expected_id:
                if "error" in msg:
                    err = msg["error"]
                    raise McpError(f"MCP error {err.get('code', '?')}: {err.get('message', 'unknown')}")
                return msg.get("result", {})

            # Not our id — skip
            continue

    # -- high-level MCP methods ----------------------------------------------

    def initialize(self):
        """Perform MCP initialize handshake."""
        # Give server a moment to start up
        time.sleep(0.5)
        if self._proc.poll() is not None:
            raise McpError(f"MCP server exited immediately (code={self._proc.returncode})")
        log("Sending MCP initialize...")
        req_id = self._send_request("initialize", {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "mcp-windbg-action",
                "version": "1.0.0",
            },
        })
        result = self._read_response(req_id, timeout=30)
        log(f"MCP initialized. Server: {result.get('serverInfo', {}).get('name', 'unknown')}")

        # Send initialized notification (no id, no response expected)
        notif = json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized", "params": {}}).encode("utf-8")
        try:
            self._proc.stdin.write(notif + b"\n")
            self._proc.stdin.flush()
        except Exception:
            pass

        return result

    def list_tools(self):
        """Request the list of available tools from the MCP server."""
        log("Requesting tools/list...")
        req_id = self._send_request("tools/list", {})
        result = self._read_response(req_id, timeout=30)
        tools = result.get("tools", [])
        log(f"Received {len(tools)} tool(s): {', '.join(t['name'] for t in tools)}")
        return tools

    def call_tool(self, name, arguments):
        """Call an MCP tool and return the result text.

        Raises McpToolError if the tool reports an error.
        """
        log(f"Calling tool: {name}({json.dumps(arguments)[:200]})")
        req_id = self._send_request("tools/call", {
            "name": name,
            "arguments": arguments,
        })
        result = self._read_response(req_id, timeout=120)

        # Check for isError flag
        if result.get("isError"):
            content = result.get("content", [])
            err_text = "\n".join(c.get("text", "") for c in content if c.get("type") == "text")
            raise McpToolError(err_text or "Tool returned an error with no message")

        # Extract text from content array
        content = result.get("content", [])
        texts = [c.get("text", "") for c in content if c.get("type") == "text"]
        return "\n".join(texts)


# ============================================================================
# LLM API Layer
# ============================================================================

class LlmClient:
    """OpenAI-compatible Chat Completions API client using urllib."""

    def __init__(self, api_base, api_key, model):
        self._api_base = api_base.rstrip("/")
        self._api_key = api_key
        self._model = model

    def chat_completions(self, messages, tools=None, timeout=120):
        """Call /chat/completions and return the parsed JSON response.

        Retries once on failure.
        """
        url = f"{self._api_base}/chat/completions"

        body = {
            "model": self._model,
            "messages": messages,
        }
        if tools:
            body["tools"] = tools

        data = json.dumps(body).encode("utf-8")

        headers = {
            "Authorization": f"Bearer {self._api_key}",
            "Content-Type": "application/json",
        }

        last_err = None
        for attempt in range(2):
            try:
                req = urllib.request.Request(url, data=data, headers=headers, method="POST")
                with urllib.request.urlopen(req, timeout=timeout) as resp:
                    resp_data = resp.read().decode("utf-8")
                    return json.loads(resp_data)
            except (urllib.error.URLError, urllib.error.HTTPError, OSError, json.JSONDecodeError) as exc:
                last_err = exc
                if attempt == 0:
                    log(f"LLM API call failed ({exc}), retrying in 2s...")
                    time.sleep(2)

        raise LlmApiError(f"LLM API call failed after 2 attempts: {last_err}")


# ============================================================================
# AI Orchestration Layer
# ============================================================================

class AnalysisOrchestrator:
    """Coordinates the MCP client and LLM client in an AI tool-calling loop."""

    def __init__(self, mcp, llm, max_turns=30, timeout=300, system_prompt=None):
        self._mcp = mcp
        self._llm = llm
        self._max_turns = max_turns
        self._timeout = timeout
        self._system_prompt = system_prompt or DEFAULT_SYSTEM_PROMPT
        self.turns_used = 0
        self.estimated_tokens = 0
        self.partial_report = None

    def _build_partial_report(self, messages):
        """Extract partial analysis content from incomplete conversation."""
        parts = []

        # Find the last assistant text content (not tool_calls only)
        for m in reversed(messages):
            if m.get("role") == "assistant" and m.get("content"):
                parts.append(m["content"])
                break

        # Summarize what tools were invoked
        tool_names = []
        for m in messages:
            if m.get("role") == "assistant" and m.get("tool_calls"):
                for tc in m.get("tool_calls", []):
                    name = tc.get("function", {}).get("name", "")
                    if name and name not in tool_names:
                        tool_names.append(name)

        if tool_names:
            parts.append("\n\n---\n*Analysis was in progress. Tools used: {}*".format(", ".join(tool_names)))

        return "\n".join(parts) if parts else ""

    def run(self, user_message):
        """Execute the AI analysis loop.

        Returns the final Analysis_Report string.
        """
        messages = [
            {"role": "system", "content": self._system_prompt},
            {"role": "user", "content": user_message},
        ]

        # Discover tools
        mcp_tools = self._mcp.list_tools()
        if not mcp_tools:
            raise McpError("MCP server returned an empty tool list")
        openai_tools = mcp_tools_to_openai_functions(mcp_tools)

        start_time = time.time()

        for turn in range(1, self._max_turns + 1):
            elapsed = time.time() - start_time
            if elapsed > self._timeout:
                log(f"Timeout reached ({elapsed:.0f}s > {self._timeout}s)")
                self.partial_report = self._build_partial_report(messages)
                raise TimeoutError(f"Analysis timed out after {elapsed:.0f}s")

            self.turns_used = turn
            # Rough token estimate: ~4 chars per token
            self.estimated_tokens = sum(len(json.dumps(m)) for m in messages) // 4

            log(f"--- Turn {turn}/{self._max_turns} (elapsed {elapsed:.0f}s, ~{self.estimated_tokens} tokens) ---")

            # Call LLM
            try:
                resp = self._llm.chat_completions(messages, tools=openai_tools, timeout=120)
            except LlmApiError:
                raise

            choice = resp.get("choices", [{}])[0]
            assistant_msg = choice.get("message", {})
            finish_reason = choice.get("finish_reason", "")

            tool_calls = assistant_msg.get("tool_calls")

            if tool_calls:
                # Append assistant message with tool_calls
                messages.append(assistant_msg)

                for tc in tool_calls:
                    fn = tc.get("function", {})
                    tool_name = fn.get("name", "")
                    tool_call_id = tc.get("id", "")

                    try:
                        tool_args = json.loads(fn.get("arguments", "{}"))
                    except json.JSONDecodeError:
                        tool_args = {}

                    args_summary = json.dumps(tool_args)[:150]
                    log(f"  Tool call: {tool_name}({args_summary})")

                    # Execute via MCP
                    try:
                        result_text = self._mcp.call_tool(tool_name, tool_args)
                    except McpToolError as exc:
                        result_text = f"[Tool Error] {exc}"
                        log(f"  Tool error: {exc}")
                    except McpError as exc:
                        result_text = f"[MCP Error] {exc}"
                        log(f"  MCP error: {exc}")

                    # Append tool result
                    messages.append({
                        "role": "tool",
                        "tool_call_id": tool_call_id,
                        "content": result_text,
                    })

                continue  # next turn

            # Pure text response — this is the final report
            content = assistant_msg.get("content", "")
            if content:
                log(f"AI returned text response (finish_reason={finish_reason}). Analysis complete.")
                return content

            # Edge case: empty response
            log(f"AI returned empty response (finish_reason={finish_reason}). Ending loop.")
            return content or "(No analysis produced)"

        # max_turns exhausted
        log(f"Max turns ({self._max_turns}) reached — returning partial analysis.")
        partial = self._build_partial_report(messages)
        return partial or "(Analysis incomplete — max turns reached)"


# ============================================================================
# Entry Point
# ============================================================================

def main():
    """Main entry point — read config from env vars, run analysis, write output."""
    print("::group::Step 4 - AI Crash Dump Analysis", flush=True)

    # -- Read configuration from environment variables -----------------------
    api_key = os.environ.get("API_KEY", "")
    api_base = os.environ.get("API_BASE", "")
    model = os.environ.get("MODEL", "")
    max_turns = int(os.environ.get("MAX_TURNS", "30"))
    timeout = int(os.environ.get("TIMEOUT", "300"))
    system_prompt = os.environ.get("SYSTEM_PROMPT", "") or None
    symbols_path = os.environ.get("SYMBOLS_PATH", r"SRV*C:\Symbols*https://msdl.microsoft.com/download/symbols")
    cdb_path = os.environ.get("CDB_PATH", "cdb.exe")
    mcp_server_path = os.environ.get("MCP_SERVER_PATH", "mcp-windbg-rs.exe")
    download_dir = os.environ.get("DOWNLOAD_DIR", "dump_files")

    if not api_key or not api_base or not model:
        print("::error::Missing required environment variables: API_KEY, API_BASE, MODEL", flush=True)
        print("::endgroup::", flush=True)
        sys.exit(1)

    log(f"Config: model={model}, max_turns={max_turns}, timeout={timeout}s")
    log(f"CDB: {cdb_path}")
    log(f"MCP Server: {mcp_server_path}")
    log(f"Download dir: {download_dir}")

    # -- Validate MCP server binary exists -----------------------------------
    if not os.path.isfile(mcp_server_path):
        print(f"::error::MCP server binary not found: {mcp_server_path}", flush=True)
        print("::endgroup::", flush=True)
        sys.exit(1)
    log(f"MCP server binary OK ({os.path.getsize(mcp_server_path)} bytes)")

    # -- Validate CDB exists -------------------------------------------------
    if not os.path.isfile(cdb_path):
        log(f"Warning: CDB not found at {cdb_path}, MCP server may fail to execute commands")

    # -- Scan for dump/exe files ---------------------------------------------
    file_paths = []
    if os.path.isdir(download_dir):
        for fname in os.listdir(download_dir):
            ext = os.path.splitext(fname)[1].lower()
            if ext in (".dmp", ".exe", ".pdb"):
                file_paths.append(os.path.join(download_dir, fname))
    else:
        log(f"Download directory not found: {download_dir}")

    if not file_paths:
        print("::error::No .dmp, .exe, or .pdb files found to analyze.", flush=True)
        print("::endgroup::", flush=True)
        sys.exit(1)

    log(f"Files to analyze: {file_paths}")

    # -- Build MCP server command and env ------------------------------------
    server_env = os.environ.copy()
    server_env["CDB_PATH"] = cdb_path
    server_env["_NT_SYMBOL_PATH"] = symbols_path

    server_cmd = [mcp_server_path]

    # -- Instantiate components ----------------------------------------------
    mcp = McpClient(server_cmd, env=server_env)
    llm = LlmClient(api_base, api_key, model)
    orchestrator = AnalysisOrchestrator(
        mcp=mcp,
        llm=llm,
        max_turns=max_turns,
        timeout=timeout,
        system_prompt=system_prompt,
    )

    # -- Run analysis --------------------------------------------------------
    report = ""
    exit_code = 0
    try:
        mcp.start()
        mcp.initialize()

        user_msg = build_user_message(file_paths)
        report = orchestrator.run(user_msg)

        log("Analysis completed successfully.")
    except McpError as exc:
        print(f"::error::MCP error: {exc}", flush=True)
        report = f"# Analysis Failed\n\nMCP communication error: {exc}"
        exit_code = 1
    except LlmApiError as exc:
        print(f"::error::LLM API error: {exc}", flush=True)
        report = f"# Analysis Failed\n\nLLM API error: {exc}"
        exit_code = 1
    except TimeoutError as exc:
        print(f"::error::Analysis timed out: {exc}", flush=True)
        partial = orchestrator.partial_report
        if partial:
            report = f"# Analysis Report (Partial — Timed Out)\n\n> ⚠️ {exc}\n\n{partial}"
            exit_code = 0
        else:
            report = f"# Analysis Timed Out\n\n{exc}"
            exit_code = 1
    except Exception as exc:
        print(f"::error::Unexpected error: {exc}", flush=True)
        report = f"# Analysis Failed\n\nUnexpected error: {exc}"
        exit_code = 1
    finally:
        try:
            mcp.shutdown()
        except Exception:
            pass

    # -- Write output files --------------------------------------------------
    workspace = os.environ.get("GITHUB_WORKSPACE", ".")

    output_path = os.path.join(workspace, "analysis_output.txt")
    with open(output_path, "w", encoding="utf-8") as f:
        f.write(report)
    log(f"Report written to {output_path} ({len(report)} chars)")

    meta_path = os.path.join(workspace, "analysis_meta.txt")
    with open(meta_path, "w", encoding="utf-8") as f:
        f.write(f"turns={orchestrator.turns_used}\n")
        f.write(f"tokens={orchestrator.estimated_tokens}\n")
    log(f"Metadata written to {meta_path}")

    # Print report to stdout as well
    print(report, flush=True)

    print("::endgroup::", flush=True)
    sys.exit(exit_code)


if __name__ == "__main__":
    main()
