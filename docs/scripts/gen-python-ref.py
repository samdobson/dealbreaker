#!/usr/bin/env python3
"""Auto-generates docs/reference/python.md from crates/dealbreaker-py/python/dealbreaker/."""

from __future__ import annotations

import ast
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PKG = ROOT / "crates/dealbreaker-py/python/dealbreaker"
SRC = PKG / "__init__.py"
STUB = PKG / "_dealbreaker.pyi"
OUT = ROOT / "docs/reference/python.md"


def format_arg(arg: ast.arg, default: ast.expr | None) -> str:
    s = arg.arg
    if arg.annotation is not None:
        s += f": {ast.unparse(arg.annotation)}"
    if default is not None:
        s += f" = {ast.unparse(default)}"
    return s


def format_signature(fn: ast.FunctionDef) -> str:
    args = fn.args
    parts: list[str] = []

    pos_args = [a for a in args.args if a.arg not in ("self", "cls")]
    pos_defaults = [None] * (len(pos_args) - len(args.defaults)) + list(args.defaults)
    for a, d in zip(pos_args, pos_defaults):
        parts.append(format_arg(a, d))

    if args.kwonlyargs:
        parts.append("*")
        for a, d in zip(args.kwonlyargs, args.kw_defaults):
            parts.append(format_arg(a, d))

    ret = f" -> {ast.unparse(fn.returns)}" if fn.returns is not None else ""
    decorators = [
        dec.id for dec in fn.decorator_list if isinstance(dec, ast.Name)
    ]
    if "property" in decorators:
        return f"@property {fn.name}{ret}"
    prefix = ""
    if "classmethod" in decorators:
        prefix = "@classmethod "
    elif "staticmethod" in decorators:
        prefix = "@staticmethod "
    return f"{prefix}{fn.name}({', '.join(parts)}){ret}"


def clean_doc(doc: str | None) -> str:
    if not doc:
        return ""
    s = doc.replace("``", "`")
    s = re.sub(r":func:`([^`]+)`", r"`\1()`", s)
    s = re.sub(r":(?:meth|class|attr|data|mod):`([^`]+)`", r"`\1`", s)
    return s.strip()


def is_typed_dict_class(node: ast.ClassDef, td_names: set[str]) -> bool:
    for base in node.bases:
        if isinstance(base, ast.Name) and (base.id == "TypedDict" or base.id in td_names):
            return True
    return False


def is_total_false(node: ast.ClassDef) -> bool:
    for kw in node.keywords:
        if kw.arg == "total" and isinstance(kw.value, ast.Constant) and kw.value.value is False:
            return True
    return False


def extract_class_fields(
    node: ast.ClassDef,
    all_classes: dict[str, ast.ClassDef],
) -> list[tuple[str, str, bool]]:
    """Return ``(field_name, type_str, required)`` for a TypedDict class and its bases."""
    fields: list[tuple[str, str, bool]] = []
    for base in node.bases:
        if isinstance(base, ast.Name) and base.id in all_classes:
            fields.extend(extract_class_fields(all_classes[base.id], all_classes))
    required = not is_total_false(node)
    for item in node.body:
        if isinstance(item, ast.AnnAssign) and isinstance(item.target, ast.Name):
            fields.append((item.target.id, ast.unparse(item.annotation), required))
    return fields


def extract_functional_typed_dict(node: ast.Assign) -> tuple[str, list[tuple[str, str, bool]]] | None:
    if len(node.targets) != 1 or not isinstance(node.targets[0], ast.Name):
        return None
    if not isinstance(node.value, ast.Call):
        return None
    if not (isinstance(node.value.func, ast.Name) and node.value.func.id == "TypedDict"):
        return None
    if len(node.value.args) < 2 or not isinstance(node.value.args[1], ast.Dict):
        return None
    required = True
    for kw in node.value.keywords:
        if kw.arg == "total" and isinstance(kw.value, ast.Constant) and kw.value.value is False:
            required = False
    fields: list[tuple[str, str, bool]] = []
    for k, v in zip(node.value.args[1].keys, node.value.args[1].values):
        if isinstance(k, ast.Constant) and isinstance(k.value, str):
            fields.append((k.value, ast.unparse(v), required))
    return node.targets[0].id, fields


def format_fields_table(fields: list[tuple[str, str, bool]]) -> list[str]:
    if not fields:
        return []
    rows = [
        "",
        "| Field | Type | Required |",
        "| --- | --- | --- |",
    ]
    for name, ty, req in fields:
        escaped_ty = ty.replace("|", "\\|")
        rows.append(f"| `{name}` | `{escaped_ty}` | {'yes' if req else 'no'} |")
    return rows


def render_class(node: ast.ClassDef, lines: list[str]) -> None:
    lines.append(f"## `class {node.name}`")
    doc = clean_doc(ast.get_docstring(node))
    if doc:
        lines.extend(["", doc])
    lines.append("")
    for item in node.body:
        if isinstance(item, ast.FunctionDef) and (
            not item.name.startswith("_") or item.name == "__init__"
        ):
            lines.append(f"### `{format_signature(item)}`")
            mdoc = clean_doc(ast.get_docstring(item))
            if mdoc:
                lines.extend(["", mdoc])
            lines.append("")


def main() -> None:
    src_tree = ast.parse(SRC.read_text(encoding="utf-8"))
    stub_tree = ast.parse(STUB.read_text(encoding="utf-8")) if STUB.exists() else None

    all_classes: dict[str, ast.ClassDef] = {
        node.name: node for node in src_tree.body if isinstance(node, ast.ClassDef)
    }
    td_names: set[str] = set()
    for node in src_tree.body:
        if isinstance(node, ast.ClassDef) and is_typed_dict_class(node, td_names):
            td_names.add(node.name)

    module_funcs: list[ast.FunctionDef] = []
    regular_classes: list[ast.ClassDef] = []
    typed_dicts: list[tuple[str, str, list[tuple[str, str, bool]]]] = []
    type_aliases: list[tuple[str, str]] = []

    if stub_tree is not None:
        for node in stub_tree.body:
            if isinstance(node, ast.ClassDef) and not node.name.startswith("_"):
                regular_classes.append(node)

    for node in src_tree.body:
        if isinstance(node, ast.FunctionDef) and not node.name.startswith("_"):
            module_funcs.append(node)
        elif isinstance(node, ast.ClassDef) and not node.name.startswith("_"):
            if node.name in td_names:
                typed_dicts.append(
                    (
                        node.name,
                        clean_doc(ast.get_docstring(node)),
                        extract_class_fields(node, all_classes),
                    )
                )
            else:
                regular_classes.append(node)
        elif isinstance(node, ast.Assign):
            ftd = extract_functional_typed_dict(node)
            if ftd is not None and not ftd[0].startswith("_"):
                typed_dicts.append((ftd[0], "", ftd[1]))
            elif (
                len(node.targets) == 1
                and isinstance(node.targets[0], ast.Name)
                and node.targets[0].id == "Step"
            ):
                type_aliases.append((node.targets[0].id, ast.unparse(node.value)))

    lines: list[str] = [
        "<!-- AUTO-GENERATED by scripts/gen-docs.sh — do not edit by hand. -->",
        "",
        "# Python Reference",
        "",
        "> Auto-generated from [`crates/dealbreaker-py/python/dealbreaker/__init__.py`](https://github.com/samdobson/dealbreaker/blob/main/crates/dealbreaker-py/python/dealbreaker/__init__.py) and [`_dealbreaker.pyi`](https://github.com/samdobson/dealbreaker/blob/main/crates/dealbreaker-py/python/dealbreaker/_dealbreaker.pyi).",
        "",
        "```sh",
        "pip install dealbreaker",
        "```",
        "",
    ]

    if module_funcs:
        lines.append("## Module Functions")
        lines.append("")
        for fn in module_funcs:
            lines.append(f"### `{format_signature(fn)}`")
            doc = clean_doc(ast.get_docstring(fn))
            if doc:
                lines.extend(["", doc])
            lines.append("")

    # Put Solver before CancelToken for readability.
    regular_classes.sort(key=lambda c: (0 if c.name == "Solver" else 1, c.name))
    for cls in regular_classes:
        render_class(cls, lines)

    if typed_dicts or type_aliases:
        lines.append("## Data Types")
        lines.append("")
        for alias_name, alias_val in type_aliases:
            lines.append(f"### `{alias_name}`")
            lines.append("")
            lines.append(f"Type alias: `{alias_val}`")
            lines.append("")
        for name, doc, fields in typed_dicts:
            lines.append(f"### `{name}`")
            if doc:
                lines.extend(["", doc])
            lines.extend(format_fields_table(fields))
            lines.append("")

    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text("\n".join(lines).rstrip() + "\n", encoding="utf-8")
    print("[gen-docs] wrote docs/reference/python.md")


if __name__ == "__main__":
    main()
