#!/usr/bin/env python3
"""Bounded independent Python MCP SDK interoperability probe for SEMAPRAX."""
import argparse
from datetime import timedelta
import hashlib
from importlib.metadata import version
import json
from pathlib import Path
import re
import sys
from typing import Any, Literal

import anyio
from mcp import ClientSession, StdioServerParameters, types
from mcp.client.stdio import stdio_client

DIGEST = re.compile(r"^sha256:[0-9a-f]{64}$")
MAX_TEXT = 1024 * 1024
REQUIRED = ("workspace__open", "candidate__open", "candidate__query", "candidate__discard")
FORBIDDEN = ("candidate__build", "candidate__test", "candidate__commit", "candidate__commit-report")


def canonical(value: Any) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()


def sha(body: bytes) -> str:
    return "sha256:" + hashlib.sha256(body).hexdigest()


def closed(value: Any, keys: set[str], label: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != keys:
        raise AssertionError(f"unexpected {label} fields")
    return value


def decode_json(text: str) -> Any:
    if len(text.encode()) > MAX_TEXT:
        raise AssertionError("MCP inner text exceeds bound")
    def pairs(rows):
        result = {}
        for key, value in rows:
            if key in result:
                raise ValueError("duplicate JSON key")
            result[key] = value
        return result
    return json.loads(text, object_pairs_hook=pairs)


def inner(result: types.CallToolResult, error: bool = False) -> dict[str, Any]:
    if result.isError is not error or result.structuredContent is not None or len(result.content) != 1:
        raise AssertionError("unexpected MCP tool result")
    item = result.content[0]
    if not isinstance(item, types.TextContent):
        raise AssertionError("tool result is not text")
    value = closed(decode_json(item.text), {"jsonrpc", "id", "error"} if error else {"jsonrpc", "id", "result"}, "inner response")
    if value["jsonrpc"] != "2.0" or value["id"] != 0:
        raise AssertionError("inner identity mismatch")
    if error:
        closed(value["error"], {"code", "message"}, "inner error")
        return value["error"]
    envelope = closed(value["result"], {"schema", "protocol", "image_revision", "project_revision", "payload"}, "inner result")
    if envelope["schema"] != "semaprax.image-agent-result.v5" or envelope["protocol"] != "semaprax.image-agent-protocol.v5":
        raise AssertionError("inner protocol mismatch")
    if not DIGEST.fullmatch(envelope["image_revision"]) or not DIGEST.fullmatch(envelope["project_revision"]):
        raise AssertionError("inner revision mismatch")
    if not isinstance(envelope["payload"], dict):
        raise AssertionError("inner payload is not an object")
    return envelope


def fixture_rows(manifest: Path) -> tuple[list[str], str]:
    root = manifest.parent
    paths = ["semaprax.toml", "src/app.spx", "src/core.spx", "src/tests.spx"]
    rows = []
    for name in paths:
        body = (root / name).read_bytes()
        rows.append({"path": name, "bytes": len(body), "sha256": sha(body)})
    return paths, sha(b"semaprax.mcp-sdk.fixture.v1\0" + canonical(rows))


async def execute(args) -> dict[str, Any]:
    manifest = Path(args.manifest).resolve(strict=True)
    paths, before = fixture_rows(manifest)
    stderr_path = Path(args.stderr).resolve()
    stderr_path.parent.mkdir(parents=True, exist_ok=True)
    parameters = StdioServerParameters(
        command=str(Path(args.compiler).resolve(strict=True)),
        args=["serve-workspace-mcp", str(manifest), str(Path(args.policy).resolve(strict=True))],
        cwd=manifest.parent,
        env={}, encoding="utf-8", encoding_error_handler="strict",
    )
    with stderr_path.open("w", encoding="utf-8", newline="\n") as errlog:
        async with stdio_client(parameters, errlog=errlog) as (read, write):
            async with ClientSession(
                read, write, read_timeout_seconds=timedelta(seconds=30),
                client_info=types.Implementation(name="semaprax-independent-mcp-sdk", version="1"),
            ) as session:
                initialized = await session.initialize()
                if initialized.protocolVersion != "2025-11-25" or initialized.serverInfo.name != "semaprax" or initialized.serverInfo.version != "0.2.0":
                    raise AssertionError("unexpected MCP negotiation")
                names, cursors, pages, total_bytes, cursor = [], set(), 0, 0, None
                while True:
                    page = await session.list_tools(cursor)
                    pages += 1
                    if pages > 64 or not 1 <= len(page.tools) <= 8:
                        raise AssertionError("catalogue page bound")
                    page_bytes = len(page.model_dump_json(by_alias=True, exclude_none=True).encode())
                    total_bytes += page_bytes
                    if page_bytes > 900 * 1024 or total_bytes > 16 * 1024 * 1024:
                        raise AssertionError("catalogue byte bound")
                    for tool in page.tools:
                        if not re.fullmatch(r"[A-Za-z0-9_.-]{1,128}", tool.name) or tool.name in names or len(names) >= 512:
                            raise AssertionError("invalid catalogue tool")
                        names.append(tool.name)
                    cursor = page.nextCursor
                    if cursor is None:
                        break
                    if not 1 <= len(cursor) <= 128 or cursor in cursors:
                        raise AssertionError("invalid catalogue cursor")
                    cursors.add(cursor)
                if names != sorted(names):
                    raise AssertionError("catalogue order is not canonical")
                if any(name not in names for name in REQUIRED) or any(name in names for name in FORBIDDEN):
                    raise AssertionError("least-authority catalogue mismatch")

                opened = inner(await session.call_tool("workspace__open", {}))
                workspace = closed(opened["payload"], {"schema", "state", "image_revision", "project_revision", "workspace_revision"}, "workspace payload")
                if workspace["schema"] != "semaprax.image-agent-workspace.v1" or workspace["state"] != "open":
                    raise AssertionError("workspace did not open")
                image = workspace["image_revision"]
                candidate_result = inner(await session.call_tool("candidate__open", {"image_revision": image}))
                candidate = closed(candidate_result["payload"], {"schema", "candidate_revision", "project_revision", "base_revision", "report_bytes", "source_authority", "tests"}, "candidate payload")
                if candidate["schema"] != "semaprax.image-candidate-handle.v1" or candidate["source_authority"] is not False or candidate["tests"] != "not_run":
                    raise AssertionError("candidate handle mismatch")
                candidate_revision = candidate["candidate_revision"]

                ToolCallNotification = types.Notification[dict[str, Any], Literal["tools/call"]]
                notice = ToolCallNotification(method="tools/call", params={"name":"candidate__discard","arguments":{"image_revision":image,"candidate_revision":candidate_revision}})
                dumped = notice.model_dump(mode="json", by_alias=True, exclude_none=True)
                if dumped != {"method":"tools/call","params":{"name":"candidate__discard","arguments":{"image_revision":image,"candidate_revision":candidate_revision}}}:
                    raise AssertionError("notification parameters were not preserved")
                await session.send_notification(notice)
                query = inner(await session.call_tool("candidate__query", {"image_revision":image,"candidate_revision":candidate_revision,"offset":0,"chunk_bytes":1024}))
                query_payload = query["payload"]
                if query_payload.get("candidate_revision") != candidate_revision or query_payload.get("schema") != "semaprax.image-candidate-report-chunk.v1":
                    raise AssertionError("notification executed or query binding changed")
                discarded = inner(await session.call_tool("candidate__discard", {"image_revision":image,"candidate_revision":candidate_revision}))
                if discarded["payload"].get("discarded") is not True:
                    raise AssertionError("ordinary discard failed")
                rejection = inner(await session.call_tool("candidate__query", {"image_revision":image,"candidate_revision":candidate_revision,"offset":0,"chunk_bytes":1024}), error=True)
                if rejection["code"] != -32000:
                    raise AssertionError("discarded candidate query did not reject semantically")

    _, after = fixture_rows(manifest)
    if after != before:
        raise AssertionError("saved fixture bytes changed")
    return {
        "schema":"semaprax.python-mcp-sdk-interoperability-observation.v1",
        "sdk":{"distribution":"mcp","version":version("mcp")},
        "protocol":{"requested":"2025-11-25","negotiated":initialized.protocolVersion,"server_name":initialized.serverInfo.name,"server_version":initialized.serverInfo.version,"tools_capability":initialized.capabilities.tools is not None},
        "catalogue":{"pages":pages,"tools":len(names),"ordered_names_sha256":sha(canonical(names)),"required_present":list(REQUIRED),"forbidden_absent":list(FORBIDDEN),"terminal_cursor":True},
        "workspace":workspace,
        "candidate":candidate,
        "notification_probe":{"method":"tools/call","tool":"candidate__discard","subsequent_query":"passed_same_candidate"},
        "ordinary_discard":{"discarded":True,"post_discard_query_error_code":rejection["code"]},
        "source":{"paths":paths,"before_sha256":before,"after_sha256":after,"unchanged":True},
    }


def main() -> None:
    parser=argparse.ArgumentParser()
    parser.add_argument("--compiler",required=True); parser.add_argument("--manifest",required=True); parser.add_argument("--policy",required=True); parser.add_argument("--stderr",required=True)
    args=parser.parse_args()
    if version("mcp") != "1.27.0":
        raise SystemExit("requires provisioned mcp==1.27.0")
    observation=anyio.run(execute,args)
    sys.stdout.buffer.write(canonical(observation)+b"\n")

if __name__=="__main__": main()
