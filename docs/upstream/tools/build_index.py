"""Build a frozen upstream audit from public gh snapshots and local Git objects.

No network or application execution. --check verifies the reproducible output.
Every added/deleted text line is checked against the appropriate Git blob.
"""
from __future__ import annotations

import argparse
import gzip
import hashlib
import json
from pathlib import Path
import re
import subprocess
from urllib.parse import quote

ROOT = Path(__file__).resolve().parents[3]
DOCS = ROOT / "docs" / "upstream"
BASE = "https://github.com/xuzhougeng/WispPaper"


def git(*args: str) -> bytes:
    return subprocess.check_output(["git", *args], cwd=ROOT)


def ancestor(older: str, newer: str) -> bool:
    result = subprocess.run(["git", "merge-base", "--is-ancestor", older, newer], cwd=ROOT)
    if result.returncode not in (0, 1):
        raise RuntimeError(f"Missing commit or invalid ancestry: {older} {newer}")
    return result.returncode == 0


def names(raw: bytes) -> list[tuple[str, str, str]]:
    fields = iter(raw.decode("utf-8").rstrip("\0").split("\0"))
    out = []
    for status in fields:
        if not status:
            continue
        old = next(fields)
        new = next(fields) if status[0] in "RC" else old
        out.append((status, old, new))
    return out


def hunks(diff: str) -> list[dict]:
    out = []
    current = None
    old = new = 0
    for line in diff.split("\n"):
        match = re.match(r"^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@", line)
        if match:
            a, ac, b, bc = match.groups()
            old, new = int(a), int(b)
            current = {"old_range": [old, int(ac) if ac is not None else 1],
                       "new_range": [new, int(bc) if bc is not None else 1],
                       "deleted": [], "added": []}
            out.append(current)
        elif current is not None:
            if line.startswith("-"):
                current["deleted"].append({"line": old, "text": line[1:]})
                old += 1
            elif line.startswith("+"):
                current["added"].append({"line": new, "text": line[1:]})
                new += 1
            elif line.startswith(" "):
                old += 1
                new += 1
    for h in out:
        if len(h["deleted"]) != h["old_range"][1] or len(h["added"]) != h["new_range"][1]:
            raise ValueError("Expected a zero-context diff with complete hunk line counts")
    return out


def source_lines(blob: bytes) -> list[str]:
    # Git counts LF, not Unicode line separators. Retain CR in CRLF files.
    lines = blob.decode("utf-8").split("\n")
    return lines[:-1] if lines[-1] == "" else lines


def verify_lines(rows: list[dict], blob: bytes) -> None:
    lines = source_lines(blob)
    for row in rows:
        if row["line"] < 1 or row["line"] > len(lines) or lines[row["line"] - 1] != row["text"]:
            raise ValueError(f"Source mismatch at line {row['line']}")


def link(commit: str, path: str, start: int, count: int = 1) -> str:
    if count == 0:
        return "—"
    end = start + count - 1
    span = f"L{start}" if count == 1 else f"L{start}–L{end}"
    anchor = f"#L{start}" + (f"-L{end}" if count > 1 else "")
    return f"[{span}]({BASE}/blob/{commit}/{quote(path, safe='/')}{anchor})"


def blob(commit: str, path: str) -> bytes:
    return git("show", f"{commit}:{path}")


def build(snapshot: Path) -> dict[Path, bytes]:
    config = json.loads((snapshot / "snapshot.json").read_text(encoding="utf-8"))
    prs = json.loads((snapshot / "prs.json").read_text(encoding="utf-8"))
    issues = json.loads((snapshot / "issues.json").read_text(encoding="utf-8"))
    release, main = config["release_commit"], config["main_commit"]
    outputs = {}
    summary = []
    totals = {"prs": len(prs), "issues": len(issues), "added": 0, "deleted": 0, "hunks": 0, "files": 0}
    status_counts = {}

    def put(path: str, content: str | bytes):
        outputs[DOCS / path] = content.encode("utf-8") if isinstance(content, str) else content

    for pr in sorted(prs, key=lambda p: p["number"]):
        number = pr["number"]
        merged = pr["state"] == "MERGED"
        commit = pr["mergeCommit"]["oid"] if merged else pr["headRefOid"]
        if merged:
            parent = git("rev-parse", f"{commit}^1").decode().strip()
            if ancestor(commit, release):
                status = "v0.6.0 已包含"
            elif ancestor(commit, main):
                status = "v0.6.0 后已合并"
            else:
                status = "已合并，但不在固定 main 快照"
        else:
            parent = git("merge-base", main, commit).decode().strip()
            status = "未合并／提案" if pr["state"] == "OPEN" else "关闭且未合并"
        status_counts[status] = status_counts.get(status, 0) + 1
        files = []
        md = [f"# PR #{number}：{pr['title']}", "", f"[上游 PR]({pr['url']}) · **{status}**", "",
              f"新版本／提案 head：`{commit}`", "", f"旧版本：`{parent}`", "",
              "合并 PR 比较第一父提交与合并结果；未合并 PR 比较固定 main 与 head 的共同祖先。删除行链接旧版本，新增行链接新版本。", "",
              f"[完整逐行数据（gzip JSON）](pr-{number}.json.gz)：包含每一条新增／删除行的行号与原文。", "",
              "范围链接覆盖该 hunk 的每一行；`—` 表示该侧没有行。二进制和仅模式变化另行标出。", ""]
        added = deleted = nhunks = 0
        for status_code, old_path, new_path in names(git("diff", "--name-status", "-z", "--find-renames", parent, commit)):
            old_blob = b"" if status_code.startswith("A") else blob(parent, old_path)
            new_blob = b"" if status_code.startswith("D") else blob(commit, new_path)
            raw_diff = git("diff", "--no-ext-diff", "--no-textconv", "--no-color", "--unified=0", "--find-renames", parent, commit, "--", *dict.fromkeys([old_path, new_path]))
            binary = b"\0" in old_blob or b"\0" in new_blob
            change = {"status": status_code, "old_path": old_path, "new_path": new_path,
                      "old_sha256": hashlib.sha256(old_blob).hexdigest(), "new_sha256": hashlib.sha256(new_blob).hexdigest(),
                      "binary": binary, "hunks": []}
            md += [f"## `{new_path}`", "", f"Git 状态：`{status_code}`" + (f"；原路径：`{old_path}`" if old_path != new_path else ""), ""]
            if binary:
                md += ["二进制变更，无文本行号；JSON 记录前后内容校验值，使用 PR diff 检查文件变化。", ""]
            else:
                change["hunks"] = hunks(raw_diff.decode("utf-8"))
                md += ["| 删除／旧源码 | 新增／新源码 |", "|---|---|"]
                for h in change["hunks"]:
                    verify_lines(h["deleted"], old_blob)
                    verify_lines(h["added"], new_blob)
                    md.append(f"| {link(parent, old_path, *h['old_range'])} | {link(commit, new_path, *h['new_range'])} |")
                    added += len(h["added"])
                    deleted += len(h["deleted"])
                nhunks += len(change["hunks"])
                if not change["hunks"]:
                    md.append("| 仅重命名／文件模式等元数据变化 | 无文本行变化 |")
                md.append("")
            files.append(change)
        payload = {"schema_version": 1, "pr": number, "status": status, "before": parent, "after": commit,
                   "source_url_template": BASE + "/blob/{commit}/{path}#L{line}", "files": files}
        encoded = json.dumps(payload, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        put(f"changes/pr-{number}.json.gz", gzip.compress(encoded, mtime=0))
        put(f"changes/pr-{number}.md", "\n".join(md))
        summary.append(f"| [#{number}]({pr['url']}) | {pr['title'].replace('|', '/')} | {status} | [{commit[:12]}](pr-{number}.md) | +{added} / −{deleted} |")
        for key, value in [("added", added), ("deleted", deleted), ("hunks", nhunks), ("files", len(files))]:
            totals[key] += value

    put("changes/README.md", "\n".join(["# 上游全部 PR 与逐行源码索引", "", f"快照：{config['captured_at']}。共 {len(prs)} 个 PR；状态由 Git 提交祖先关系计算。", "",
        "这是全部 PR 的历史差异索引，含打包、文档和测试；具体功能以[能力核对](../README.md)和[固定版本源码入口](../sources.md)为准。历史 PR 的实现可能被后续 PR 替换。", "",
        "| PR | 变更 | 固定快照状态 | 源码 hunk 索引 | 文本行 |", "|---|---|---|---|---|", *summary, "",
        f"合计：{totals['files']} 个文件变更记录、{totals['hunks']} 个 hunk、+{totals['added']} / −{totals['deleted']} 条文本行（各 PR 历史累计，不代表当前代码规模）。", "",
        "逐行 JSON 中，每个 file 的 hunks[].added / deleted 都保存 {line, text}；分别用 after/new_path、before/old_path 代入 source_url_template。路径需 URL 编码。生成时已将每条原文与对应 Git blob 核对。", "",
        "gzip 使用固定时间戳；输入快照不更新时，索引可以重复生成并逐字节验证。二进制没有文本行号；纯重命名和文件模式变化没有新增／删除源码行。", ""]))

    anchors = json.loads((snapshot / "anchors.json").read_text(encoding="utf-8"))
    sections = ["# 固定版本的源码入口", "", "以下定位通过固定提交中的精确字符串查找生成，并校验每个范围。`release` 是 v0.6.0；`main` 是本次发布后快照；`fork` 是我们自己的版本。局部入口用于解释行为，全部变更见 [PR 索引](changes/README.md)。", ""]
    for a in anchors:
        commit = config[{"release": "release_commit", "main": "main_commit", "fork": "fork_commit"}[a["version"]]]
        data = source_lines(blob(commit, a["path"]))
        matches = [i for i, line in enumerate(data) if a["needle"] in line]
        if len(matches) != 1:
            raise ValueError(f"Anchor must be unique: {a}")
        start = matches[0] + 1
        count = a.get("lines", 12)
        if start + count - 1 > len(data):
            raise ValueError(f"Anchor outside file: {a}")
        url = link(commit, a["path"], start, count)
        if a["version"] == "fork":
            url = url.replace("xuzhougeng/WispPaper", "Yu-Qiao-sjtu/WispPaper")
        sections += [f"## {a['id']}", "", f"**{a['title']}**（{a['version']}，`{commit[:12]}`）", "", f"`{a['path']}` {url}", "", a["finding"], ""]
    put("sources.md", "\n".join(sections))
    put("validation.json", json.dumps({"schema_version": 1, "snapshot": str(snapshot.relative_to(DOCS)).replace('\\', '/'), "totals": totals, "statuses": status_counts, "anchors": len(anchors), "all_text_lines_verified_against_git_blobs": True}, ensure_ascii=False, indent=2) + "\n")
    return outputs


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--snapshot", default="snapshots/2026-11-13")
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    outputs = build(DOCS / args.snapshot)
    for path, data in outputs.items():
        if args.check:
            if not path.exists() or path.read_bytes() != data:
                raise SystemExit(f"Out of date: {path}")
        else:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
    print(f"{'Verified' if args.check else 'Generated'} {len(outputs)} files; all changed text lines match pinned Git blobs.")


if __name__ == "__main__":
    main()
