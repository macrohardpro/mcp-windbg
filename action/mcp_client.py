#!/usr/bin/env python3
"""
MCP Client + AI Orchestrator for Windows crash dump analysis.

This script acts as a lightweight MCP client that communicates with
mcp-windbg-rs (MCP Server) via stdio JSON-RPC 2.0, and orchestrates
an AI analysis loop using any OpenAI-compatible LLM API.

Dependencies: Python standard library only (no third-party packages).
"""

import argparse
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
你是一名资深 Windows 转储分析专家，精通 Windows 内核、调试技术、内存管理和多线程并发。\
你可以通过 MCP（模型上下文协议）调用调试工具进行自动化分析。

你的任务是分析提供的转储文件或可执行文件，并生成一份**全中文**的结构化分析报告。

## ⚠️ 语言要求（最高优先级）

- **所有输出必须使用简体中文**，包括思考过程、分析推理和最终报告。
- 调试工具（WinDbg）的输出是英文的，这是正常的——你需要在报告中用中文解读这些英文输出。
- 报告中引用调试输出时，保留原始英文地址和模块名，但用中文解释其含义。

## 第一步：判断转储类型（关键！）

打开转储文件后，**首先判断转储类型**，然后走不同的分析路径：

### 类型 A：崩溃转储（有异常记录）
特征：`.exr -1` 有输出，或 `!analyze -v` 会报告异常。
→ 走「崩溃分析路径」

### 类型 B：快照转储 / 挂起转储（无异常记录）
特征：`.exr -1` 无有效异常，进程是在某个时刻被手动抓取或系统收集的快照。
常见场景：
- 进程卡死/无响应
- 内存持续增长、句柄泄漏
- CPU 占用异常偏高
- 磁盘 I/O 异常（读写繁忙、文件句柄堆积）
- 性能问题诊断
→ 走「快照分析路径」

---

## 崩溃分析路径（类型 A）

1. **获取异常信息**：执行 `.exr -1` 查看异常记录，`!analyze -v` 获取自动化分析。
2. **堆栈回溯**：`kb` 查看当前线程调用栈，`~*k` 查看所有线程。
3. **深入诊断**：根据异常类型选择命令：
   - 访问违例 → `!address -summary`、`!vprot <addr>`
   - 堆损坏 → `!heap -s`、`!heap -a`
   - 死锁 → `!locks`、`!cs -l`
   - 栈溢出 → `!teb`、`!gle`
4. **模块信息**：`lm` 查看加载的模块及版本。

## 快照分析路径（类型 B）——无异常的转储

对于非崩溃转储，重点分析**CPU、内存、磁盘 I/O、线程和资源占用**。

### 2.1 先快速摸底（必查，均轻量）
打开转储后，先用这几个轻量命令快速掌握全局概况，再根据发现决定是否深入：
- `~` — 线程列表及状态分布（轻量）
- `!address -summary` — 虚拟内存概况（轻量）
- `lm` — 加载模块列表（轻量）
- `.exr -1` — 确认有无异常

### 2.2 CPU 高占用分析（按需深入）
仅当 `~` 发现线程数量异常多、或从场景判断怀疑 CPU 占用高时执行：
- `!runaway 7` — 各线程 CPU 时间排行，快速定位消耗大户（轻量）
- 对 CPU 时间高的线程，`~<N>s; kb` 查看其调用栈（轻量）
- `!tp` — 线程池状态（轻量）
- `~*k` — 仅在上述命令不足以定位问题时才执行（线程多时较慢）

### 2.3 内存分析（先看概况，异常再深入）
- `!address -summary` — **必查**，极轻量：Committed / Reserved / Free / 最大连续区域
- `!heap -s` — 堆摘要，看各堆提交量（轻量）
- `!heap -stat` — ⚠️ 较耗时，仅在 `!heap -s` 发现异常增长时才执行
- 关注指标：空闲虚拟内存是否耗尽、堆提交量是否异常大

### 2.4 磁盘 I/O 分析（按需）
仅当场景涉及文件操作密集、日志写入频繁、或怀疑句柄泄漏时执行：
- `!handle` — 先看句柄总数（轻量），数量异常高再深入
- `!handle 0 f` — ⚠️ **非常耗时**（可能数十秒甚至分钟级），仅在句柄总数异常且需要定位具体泄漏源时才执行
- `~*k` — 仅在怀疑线程阻塞在 I/O 时执行，关注 `NtReadFile` / `NtWriteFile` / `ReadFile` / `WriteFile` 等调用

### 2.5 线程与锁分析（按需）
仅当怀疑死锁、锁竞争、或线程大面积阻塞时执行：
- `!locks` — 临界区锁（轻量）
- `!cs -l` — 所有临界区，关注 LockCount 高的（轻量）
- `~*k` — ⚠️ 线程多时较慢，仅在以上轻量命令发现锁问题后再执行以获取完整堆栈

### 2.6 综合交叉分析
将 CPU/内存/磁盘/线程的发现交叉印证，常见模式：
- 少数线程 CPU 时间高 + 调用栈无 Wait → **CPU 密集型计算或死循环**
- 大量线程在同一个锁等待 + 持有者 CPU 低 → **锁竞争导致并发性能差**
- 虚拟内存碎片化 + 空闲连续区域小 → **OOM 前兆，即使提交量不大**
- 大量文件句柄 + 线程卡在 WriteFile → **磁盘 I/O 瓶颈或日志风暴**
- 堆提交持续增长 + 大量同大小分配 → **内存泄漏**
- 线程池工作线程全部繁忙 + 任务队列堆积 → **线程池耗尽**

---

## 报告格式

你的最终报告必须使用 Markdown 格式。根据转储类型，选择对应的报告模板：

---

## 研发分析报告

> 面向开发人员，包含完整的技术细节。**本部分必须用中文撰写。**

### 转储类型判断
说明这是崩溃转储还是快照转储，判断依据是什么。

### 核心发现（崩溃转储）
- **崩溃概要**：异常代码、故障模块、崩溃地址
- **详细分析**：异常类型及含义、故障指令、相关寄存器状态
- **堆栈分析**：关键堆栈帧，系统代码→用户代码的转换点
- **根因分析**：最可能的崩溃原因，附证据。多可能性时按置信度排序。

### 核心发现（快照转储）
- **CPU 状况**：CPU 时间最高的线程、是否存在计算密集型或死循环线程、线程池是否耗尽
- **内存状况**：虚拟内存使用、堆使用量、最大连续空闲区域、是否存在内存泄漏迹象或 OOM 风险
- **磁盘 I/O 状况**：文件句柄数量是否异常、是否有句柄泄漏、线程是否大量阻塞在 I/O 操作上
- **线程状况**：线程总数、各线程状态分布、可疑线程（长时间等待/持有锁/高 CPU）、是否存在死锁
- **资源状况**：句柄总数与类型分布、临界区持有情况

### 诊断结论
综合所有发现，给出最可能的问题诊断。如果是快照转储，明确指出是"无明显异常，进程处于正常运行状态"还是"发现异常指标"。

### 修复/处理建议
具体可操作的方案，或建议进一步收集的诊断信息。

---

## 售后支持报告

> 面向售后/技术支持人员，**用通俗易懂的中文**，避免技术术语堆砌。

### 问题描述
用非技术语言解释发生了什么，用户会看到什么现象。

### 受影响场景
哪些使用场景或操作可能触发此问题。

### 影响范围
单用户还是多用户受影响？是否涉及数据安全或业务中断？

### 建议措施
可以给客户的操作建议（升级驱动、安装补丁、修改配置、重启服务等）。

### 升级建议
如需转交研发团队，应附带的关键信息。

---

## 分析规范
- **全程使用简体中文**——硬性要求。
- WinDbg 输出是英文的，保留原文地址和模块名（如 `0x7ffe1234`、`ntdll.dll`），用中文解释含义。
- 每个结论都要有调试输出作为证据支撑，**严禁虚构**不存在的内容。
- 如果转储文件信息不足，如实说明，提出补充信息的建议。
- 两个报告部分都必须完整，不能省略。

## ⚠️ 命令执行原则（避免耗时过长）

并非所有命令都需要执行。遵循"快速摸底 → 按需深入"的原则，避免不必要的耗时操作：

### 优先使用的轻量命令（执行快）
- `~` — 线程列表（比 `~*k` 快很多，先看分布）
- `kb` — 当前线程堆栈（比所有线程堆栈快）
- `!address -summary` — 内存概况（几乎瞬间）
- `lm` — 模块列表
- `.exr -1` — 异常记录
- `!locks` — 临界区检查
- `!teb` — 当前线程环境块

### 按需执行的重量命令（耗时长，仅在必要时使用）
- `!handle 0 f` — **非常耗时**，仅在怀疑句柄泄漏时执行。大部分情况下用 `!handle`（无参，仅统计总数）即可
- `~*k` — 线程多时较慢。先用 `~` 了解线程分布，仅当发现可疑线程多或排查死锁时才执行
- `!heap -stat` — 堆分配统计耗时。先用 `!heap -s` 看摘要，发现有异常增长的堆再深入
- `!analyze -v` — 崩溃转储优先执行（价值高），快照转储可跳过或最后作为补充

### 原则
- **先轻后重**：用轻量命令摸底，发现异常指标后再执行对应重量命令深入
- **够用即止**：一旦获得足够信息支撑结论，就不必继续执行更多命令
- **针对性执行**：根据已发现的线索选择命令，而不是盲目遍历所有命令
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
        return "未找到转储文件或可执行文件，请检查上传步骤。"

    lines = ["请分析以下文件：\n"]
    for fp in file_paths:
        ext = os.path.splitext(fp)[1].lower()
        label = {".dmp": "崩溃转储", ".exe": "可执行文件", ".pdb": "调试符号"}.get(ext, "文件")
        lines.append(f"- {label}：`{fp}`")

    lines.append("\n请先打开转储文件（如有），或在调试器中启动可执行文件，然后按流程完成分析。")
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

    def __init__(self, mcp, llm, max_turns=30, timeout=600, system_prompt=None,
                 cdb_path=None, symbols_path=None):
        self._mcp = mcp
        self._llm = llm
        self._max_turns = max_turns
        self._timeout = timeout
        self._system_prompt = system_prompt or DEFAULT_SYSTEM_PROMPT
        self._cdb_path = cdb_path or "cdb.exe"
        self._symbols_path = symbols_path or ""
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

    def _try_direct_cdb(self, tool_name, tool_args):
        """Attempt direct cdb.exe invocation as fallback for timed-out MCP calls.
        Returns (result_text, success_bool).
        """
        cdb = self._cdb_path

        # Check cdb exists
        if not os.path.isfile(cdb):
            log(f"CDB not found at {cdb}, skipping fallback")
            return None, False

        dump_path = tool_args.get("dump_path", "")
        connection_string = tool_args.get("connection_string", "")
        command = tool_args.get("command", "")

        try:
            if tool_name == "open_windbg_dump":
                if not dump_path:
                    return None, False
                cmd = [cdb, "-z", dump_path, "-c", ".symopt+0x100;.lastevent;!analyze -v;q"]

            elif tool_name == "run_windbg_cmd":
                if dump_path:
                    cmd = [cdb, "-z", dump_path, "-c", f".symopt+0x100;{command};q"]
                elif connection_string:
                    cmd = [cdb, "-remote", connection_string, "-c", f".symopt+0x100;{command};q"]
                else:
                    return None, False

            elif tool_name == "open_windbg_remote":
                if not connection_string:
                    return None, False
                cmd = [cdb, "-remote", connection_string, "-c", ".symopt+0x100;!peb;r;q"]

            else:
                # launch_debug, close_*, list_windbg_dumps — no fallback
                return None, False

            log(f"MCP call timed out, falling back to direct CDB invocation: {' '.join(cmd)}")

            env = os.environ.copy()
            if self._symbols_path:
                env["_NT_SYMBOL_PATH"] = self._symbols_path

            proc = subprocess.run(
                cmd,
                capture_output=True,
                timeout=120,
                env=env,
            )

            output = proc.stdout.decode("utf-8", errors="replace")
            if proc.stderr:
                stderr_text = proc.stderr.decode("utf-8", errors="replace")
                if stderr_text.strip():
                    output += "\n[STDERR]\n" + stderr_text

            if not output.strip():
                output = "(CDB produced no output)"

            log(f"Direct CDB fallback succeeded ({len(output)} chars of output)")
            return output, True

        except subprocess.TimeoutExpired:
            log(f"Direct CDB fallback also timed out")
            return None, False
        except Exception as exc:
            log(f"Direct CDB fallback failed: {exc}")
            return None, False

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
                        log(f"  MCP error: {exc}, attempting direct CDB fallback...")
                        fallback_text, ok = self._try_direct_cdb(tool_name, tool_args)
                        if ok:
                            result_text = fallback_text
                            log(f"  Fallback succeeded using direct CDB")
                        else:
                            result_text = f"[MCP Error] {exc}"

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
    """Main entry point — read config from env vars and CLI args, run analysis, write output."""
    # -- Parse command line arguments ----------------------------------------
    parser = argparse.ArgumentParser(
        description="MCP Client for Windows crash dump analysis",
        formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument(
        "--symbols-path",
        type=str,
        default=None,
        help="Path to symbol files directory (optional). Will be set as _NT_SYMBOL_PATH."
    )
    parser.add_argument(
        "--source-path",
        type=str,
        default=None,
        help="Path to source code directory (optional). Will be set as _NT_SOURCE_PATH."
    )
    
    args = parser.parse_args()
    
    print("::group::Step 4 - AI Crash Dump Analysis", flush=True)
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")

    # -- Read configuration from environment variables -----------------------
    api_key = os.environ.get("API_KEY", "")
    api_base = os.environ.get("API_BASE", "")
    model = os.environ.get("MODEL", "")
    max_turns = int(os.environ.get("MAX_TURNS", "30"))
    timeout = int(os.environ.get("TIMEOUT", "600"))
    system_prompt = os.environ.get("SYSTEM_PROMPT", "") or None
    symbols_path = (
        os.environ.get("SYMBOLS_PATH")
        or os.environ.get("_NT_SYMBOL_PATH")
        or r"SRV*C:\Symbols*https://msdl.microsoft.com/download/symbols"
    )
    cdb_path = os.environ.get("CDB_PATH", "cdb.exe")
    mcp_server_path = os.environ.get("MCP_SERVER_PATH", "mcp-windbg-rs.exe")
    download_dir = os.environ.get("DOWNLOAD_DIR", "dump_files")
    
    # -- Override symbols_path from command line if provided -----------------
    if args.symbols_path:
        symbols_path = args.symbols_path
        log(f"Using symbols path from command line: {symbols_path}")

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
    
    # -- Set source path environment variable if provided --------------------
    if args.source_path:
        server_env["_NT_SOURCE_PATH"] = args.source_path
        log(f"Using source path: {args.source_path}")

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
        cdb_path=cdb_path,
        symbols_path=symbols_path,
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
