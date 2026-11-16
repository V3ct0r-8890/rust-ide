#!/usr/bin/env python3
"""Where did a Claude Code session's tokens go?  (read-only, standard library only)

Usage:
    python -I tools/usage_report.py                 # newest session of any project
    python -I tools/usage_report.py <session.jsonl> # a specific transcript
    python -I tools/usage_report.py --project Rust-IDE

Weights (cost relative to one normal input token): cache read 0.1, cache write 2,
output 5. Group shares are estimates: each request's cost is split over the pieces
of the conversation it had to read or write, sized by text length / 4 (images by
pixels / 750). The totals are exact; the split between groups is approximate.
"""
import base64, collections, glob, json, os, struct, sys

W_READ, W_WRITE, W_OUT = 0.1, 2.0, 5.0
G_INSTR = "Instructions (system prompt + tool list)"
G_THINK = "Claude's thinking and replies"
G_IMG = "Screenshots (images)"


def find_transcript(args):
    paths = [a for a in args if a.endswith(".jsonl")]
    if paths:
        return paths[0]
    project = args[args.index("--project") + 1] if "--project" in args else ""
    root = os.path.join(os.path.expanduser("~"), ".claude", "projects")
    files = [f for f in glob.glob(os.path.join(root, "*", "*.jsonl")) if project in f]
    if not files:
        sys.exit(f"No transcripts found under {root}")
    return max(files, key=os.path.getmtime)


def text_tokens(s):
    return max(1, len(s) // 4)


def image_tokens(block):
    try:
        data = block["source"]["data"]
        head = base64.b64decode(data[:64] + "=" * (-len(data[:64]) % 4))
        if head[:8] == b"\x89PNG\r\n\x1a\n":
            w, h = struct.unpack(">II", head[16:24])
            k = min(1.0, 1568 / max(w, h))
            return int(w * k * h * k / 750)
    except Exception:
        pass
    return 1200


def tool_group(name):
    if name in ("Write", "Edit", "NotebookEdit"):
        return "Writing and editing files"
    if name in ("Bash", "PowerShell"):
        return "Running commands"
    if name in ("Read", "Grep", "Glob"):
        return "Reading and searching files"
    if name in ("WebSearch", "WebFetch"):
        return "Web research"
    if name == "Agent":
        return "Subagents"
    if name.startswith("mcp__claude-in-chrome__"):
        return "Claude in Chrome"
    if name.startswith("mcp__ccd_turn__"):
        return "Task list and wrap-up tools"
    if name.startswith("mcp__"):
        return "Connectors: " + name.split("__")[1][:24]
    return "Everything else"


def main():
    path = find_transcript(sys.argv[1:])
    entries = []
    with open(path, encoding="utf-8") as f:
        for line in f:
            try:
                entries.append(json.loads(line))
            except Exception:
                pass

    calls = collections.OrderedDict()
    for d in entries:
        if d.get("type") != "assistant":
            continue
        rid = d.get("requestId") or d["message"].get("id")
        u = d["message"].get("usage") or {}
        c = calls.setdefault(rid, dict(inp=0, cr=0, cw=0, out=0))
        for k, key in (("inp", "input_tokens"), ("cr", "cache_read_input_tokens"),
                       ("cw", "cache_creation_input_tokens"), ("out", "output_tokens")):
            c[k] = max(c[k], u.get(key, 0))
    if not calls:
        sys.exit("No assistant requests in this transcript.")
    order = list(calls)
    raw = {k: sum(c[k] for c in calls.values()) for k in ("inp", "cr", "cw", "out")}
    total = raw["inp"] + W_READ * raw["cr"] + W_WRITE * raw["cw"] + W_OUT * raw["out"]
    first = calls[order[0]]
    instr = first["cr"] + first["cw"] + first["inp"]

    cost = collections.defaultdict(float)
    items = []
    state = dict(last=0, cur=None, gen=[], images=0, image_tok=0)
    tool_name, seen = {}, set()

    def charge_input(c):
        new, old = items[state["last"]:], items[:state["last"]]
        est_new, est_old = sum(t for _, t in new), sum(t for _, t in old)
        write = W_WRITE * c["cw"] + c["inp"]
        if est_new:
            for g, t in new:
                cost[g] += write * t / est_new
        else:
            cost["Everything else"] += write
        scale = (c["cr"] - instr) / est_old if est_old and c["cr"] > instr else 1.0
        parts = [(G_INSTR, instr)] + [(g, t * max(scale, 0.05)) for g, t in old]
        s = sum(w for _, w in parts)
        for g, w in parts:
            cost[g] += W_READ * c["cr"] * w / s
        state["last"] = len(items)

    def charge_output():
        c, gen = calls[state["cur"]], state["gen"]
        est = sum(t for _, t in gen)
        if not gen:
            cost[G_THINK] += W_OUT * c["out"]
        for g, t in gen:
            cost[g] += W_OUT * c["out"] * t / est
        items.extend([g, t] for g, t in gen)

    def add_image(block):
        t = image_tokens(block)
        state["images"] += 1
        state["image_tok"] += t
        items.append([G_IMG, t])

    for d in entries:
        kind = d.get("type")
        if kind == "assistant":
            rid = d.get("requestId") or d["message"].get("id")
            if rid not in seen:
                if state["cur"] is not None:
                    charge_output()
                seen.add(rid)
                state["cur"], state["gen"] = rid, []
                charge_input(calls[rid])
            for b in d["message"].get("content", []):
                bt = b.get("type")
                if bt == "thinking":
                    state["gen"].append((G_THINK, text_tokens(b.get("thinking", ""))))
                elif bt == "text":
                    state["gen"].append((G_THINK, text_tokens(b.get("text", ""))))
                elif bt == "tool_use":
                    tool_name[b["id"]] = b["name"]
                    state["gen"].append((tool_group(b["name"]), text_tokens(json.dumps(b.get("input", {})))))
        elif kind == "user":
            content = d.get("message", {}).get("content")
            if isinstance(content, str):
                items.append(["Your messages", text_tokens(content)])
                continue
            for b in content or []:
                bt = b.get("type")
                if bt == "text":
                    items.append(["Your messages", text_tokens(b.get("text", ""))])
                elif bt == "image":
                    add_image(b)
                elif bt == "tool_result":
                    group = tool_group(tool_name.get(b.get("tool_use_id"), ""))
                    inner = b.get("content")
                    parts = [{"type": "text", "text": inner}] if isinstance(inner, str) else (inner or [])
                    for p in parts:
                        if p.get("type") == "image":
                            add_image(p)
                        elif p.get("type") == "text":
                            items.append([group, text_tokens(p.get("text", ""))])
        elif kind == "attachment":
            items.append(["System notes and reminders", text_tokens(json.dumps(d.get("attachment")))])
    if state["cur"] is not None:
        charge_output()

    ctx = [c["cr"] + c["cw"] + c["inp"] for c in calls.values()]
    print(f"Transcript : {path}")
    print(f"Requests   : {len(calls)}   average context {sum(ctx) // len(ctx):,} tokens, last {ctx[-1]:,}")
    print(f"Raw tokens : input {raw['inp']:,}  cache read {raw['cr']:,}  cache write {raw['cw']:,}  output {raw['out']:,}")
    print(f"Weighted   : {total:,.0f}   (instructions alone: ~{instr:,} tokens per request)")
    print(f"Screenshots: {state['images']} images, ~{state['image_tok']:,} tokens counted once\n")
    top = max(cost.values())
    for g, v in sorted(cost.items(), key=lambda kv: -kv[1]):
        if v / total < 0.001:
            continue
        bar = "#" * int(v / top * 30)
        print(f"{g[:42]:<42} {bar:<30} {v / total * 100:5.1f}%")
    print(f"\nBy kind: output {W_OUT * raw['out'] / total * 100:.1f}%  "
          f"re-reading history {W_READ * raw['cr'] / total * 100:.1f}%  "
          f"new context written {(W_WRITE * raw['cw'] + raw['inp']) / total * 100:.1f}%")


if __name__ == "__main__":
    main()
