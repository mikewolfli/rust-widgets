#!/usr/bin/env python3
"""Generate feature-completeness matrix report for src modules.

This script scans Rust sources under `src/` and counts placeholder/fallback/no-op
signals plus explicit unimplemented markers. The output is a markdown report that
can be uploaded as a CI artifact.
"""

from __future__ import annotations

import argparse
import datetime as dt
import pathlib
import re
from dataclasses import dataclass
from typing import Dict, Iterable, List, Mapping, Tuple

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover
    tomllib = None


PATTERNS: Dict[str, re.Pattern[str]] = {
    "placeholder": re.compile(r"\bplaceholder\b|\bstub\b|\btodo\b", re.IGNORECASE),
    "fallback": re.compile(r"\bfallback\b", re.IGNORECASE),
    "no_op": re.compile(r"\bno[- ]?op\b", re.IGNORECASE),
    "unimplemented": re.compile(
        r"todo!\s*\(|unimplemented!\s*\(|not\s+implemented|not\s+yet\s+implemented",
        re.IGNORECASE,
    ),
}


@dataclass(frozen=True)
class FileScan:
    path: pathlib.Path
    module: str
    raw_counts: Dict[str, int]
    effective_counts: Dict[str, int]
    suppressed_counts: Dict[str, int]
    suppression_reasons: Dict[str, str]

    @property
    def raw_total(self) -> int:
        return sum(self.raw_counts.values())

    @property
    def effective_total(self) -> int:
        return sum(self.effective_counts.values())

    @property
    def suppressed_total(self) -> int:
        return sum(self.suppressed_counts.values())


@dataclass(frozen=True)
class AllowlistRule:
    values: Dict[str, int]
    reasons: Dict[str, str]


@dataclass(frozen=True)
class AllowlistConfig:
    files: Dict[str, AllowlistRule]
    modules: Dict[str, AllowlistRule]


def discover_rust_files(src_root: pathlib.Path) -> Iterable[pathlib.Path]:
    return sorted(path for path in src_root.rglob("*.rs") if path.is_file())


def module_name_for(path: pathlib.Path, src_root: pathlib.Path) -> str:
    relative = path.relative_to(src_root)
    if len(relative.parts) == 1:
        stem = relative.stem
        return stem if stem != "mod" else "root"
    return relative.parts[0]


def parse_allowlist_rule(raw: Mapping[str, object]) -> AllowlistRule:
    values: Dict[str, int] = {}
    reasons: Dict[str, str] = {}
    for category in PATTERNS:
        value = raw.get(category)
        if isinstance(value, int):
            values[category] = max(value, 0)
        reason = raw.get(f"{category}_reason")
        if isinstance(reason, str) and reason.strip():
            reasons[category] = reason.strip()
    return AllowlistRule(values=values, reasons=reasons)


def load_allowlist(path: pathlib.Path | None) -> AllowlistConfig:
    if path is None or not path.exists() or tomllib is None:
        return AllowlistConfig(files={}, modules={})

    data = tomllib.loads(path.read_text(encoding="utf-8"))
    files = {
        file_path: parse_allowlist_rule(rule)
        for file_path, rule in data.get("files", {}).items()
        if isinstance(rule, dict)
    }
    modules = {
        module_name: parse_allowlist_rule(rule)
        for module_name, rule in data.get("modules", {}).items()
        if isinstance(rule, dict)
    }
    return AllowlistConfig(files=files, modules=modules)


def scan_file(path: pathlib.Path, src_root: pathlib.Path, allowlist: AllowlistConfig) -> FileScan:
    text = path.read_text(encoding="utf-8")
    raw_counts = {name: len(pattern.findall(text)) for name, pattern in PATTERNS.items()}
    relative = path.relative_to(src_root.parent).as_posix()
    module = module_name_for(path, src_root)
    file_rule = allowlist.files.get(relative)
    module_rule = allowlist.modules.get(module)

    effective_counts: Dict[str, int] = {}
    suppressed_counts: Dict[str, int] = {}
    suppression_reasons: Dict[str, str] = {}

    for category, raw in raw_counts.items():
        file_allow = file_rule.values.get(category, 0) if file_rule else 0
        module_allow = module_rule.values.get(category, 0) if module_rule else 0
        allow = max(file_allow, module_allow)
        effective = max(raw - allow, 0)
        suppressed = raw - effective
        effective_counts[category] = effective
        suppressed_counts[category] = suppressed
        if suppressed > 0:
            reason = None
            if file_rule:
                reason = file_rule.reasons.get(category)
            if not reason and module_rule:
                reason = module_rule.reasons.get(category)
            if reason:
                suppression_reasons[category] = reason

    return FileScan(
        path=path,
        module=module,
        raw_counts=raw_counts,
        effective_counts=effective_counts,
        suppressed_counts=suppressed_counts,
        suppression_reasons=suppression_reasons,
    )


def aggregate_by_module(scans: List[FileScan], field: str) -> Dict[str, Dict[str, int]]:
    aggregated: Dict[str, Dict[str, int]] = {}
    for scan in scans:
        if scan.module not in aggregated:
            aggregated[scan.module] = {name: 0 for name in PATTERNS}
        counts = getattr(scan, field)
        for name, count in counts.items():
            aggregated[scan.module][name] += count
    return aggregated


def render_markdown(
    *,
    scans: List[FileScan],
    src_root: pathlib.Path,
    threshold: int,
    allowlist_path: pathlib.Path | None,
) -> str:
    generated_at = dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    module_raw = aggregate_by_module(scans, "raw_counts")
    module_effective = aggregate_by_module(scans, "effective_counts")
    module_suppressed = aggregate_by_module(scans, "suppressed_counts")
    flagged = [scan for scan in scans if scan.effective_total >= threshold]
    flagged.sort(key=lambda item: (-item.effective_total, str(item.path.relative_to(src_root.parent))))

    lines: List[str] = []
    lines.append("# rust_widgets feature-completeness matrix")
    lines.append("")
    lines.append(f"Generated at: {generated_at}")
    lines.append(f"Scanned root: `{src_root}`")
    lines.append(
        f"Allowlist: `{allowlist_path}`" if allowlist_path else "Allowlist: disabled"
    )
    lines.append("")
    lines.append("## Signal definitions")
    lines.append("- `placeholder`: placeholder/stub/todo textual markers")
    lines.append("- `fallback`: fallback paths")
    lines.append("- `no_op`: no-op markers")
    lines.append("- `unimplemented`: explicit unimplemented/todo! markers")
    lines.append("")

    lines.append("## Module summary (effective)")
    lines.append("| Module | Placeholder | Fallback | No-op | Unimplemented | Effective Total | Raw Total | Suppressed |")
    lines.append("|---|---:|---:|---:|---:|---:|---:|---:|")

    def module_total(entry: Tuple[str, Dict[str, int]]) -> int:
        _, counts = entry
        return sum(counts.values())

    for module, counts in sorted(module_effective.items(), key=lambda entry: (-module_total(entry), entry[0])):
        total = sum(counts.values())
        raw_total = sum(module_raw.get(module, {}).values())
        suppressed_total = sum(module_suppressed.get(module, {}).values())
        lines.append(
            f"| {module} | {counts['placeholder']} | {counts['fallback']} | {counts['no_op']} | {counts['unimplemented']} | {total} | {raw_total} | {suppressed_total} |"
        )

    lines.append("")
    lines.append(f"## File details (effective total >= {threshold})")
    if not flagged:
        lines.append("No files exceed the threshold.")
    else:
        lines.append("| File | Placeholder | Fallback | No-op | Unimplemented | Effective Total | Raw Total | Suppressed |")
        lines.append("|---|---:|---:|---:|---:|---:|---:|---:|")
        for scan in flagged:
            relative = scan.path.relative_to(src_root.parent)
            lines.append(
                f"| {relative} | {scan.effective_counts['placeholder']} | {scan.effective_counts['fallback']} | {scan.effective_counts['no_op']} | {scan.effective_counts['unimplemented']} | {scan.effective_total} | {scan.raw_total} | {scan.suppressed_total} |"
            )

    allowlisted = [scan for scan in scans if scan.suppressed_total > 0]
    allowlisted.sort(key=lambda item: (-item.suppressed_total, str(item.path.relative_to(src_root.parent))))

    lines.append("")
    lines.append("## Allowlist suppressions")
    if not allowlisted:
        lines.append("No allowlist suppressions were applied.")
    else:
        lines.append("| File | Category | Suppressed | Reason |")
        lines.append("|---|---|---:|---|")
        for scan in allowlisted:
            relative = scan.path.relative_to(src_root.parent)
            for category in PATTERNS:
                suppressed = scan.suppressed_counts.get(category, 0)
                if suppressed <= 0:
                    continue
                reason = scan.suppression_reasons.get(category, "(no reason provided)")
                lines.append(f"| {relative} | {category} | {suppressed} | {reason} |")

    lines.append("")
    lines.append("## Notes")
    lines.append("- This is a heuristic text scan intended for trend tracking and audit surfacing.")
    lines.append("- Findings should be reviewed manually before deciding remediation priorities.")
    lines.append("")
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description="Generate feature completeness matrix report")
    parser.add_argument("--src", default="src", help="Source root to scan")
    parser.add_argument(
        "--output",
        default="target/qa/feature_completeness_matrix.md",
        help="Markdown report output path",
    )
    parser.add_argument(
        "--threshold",
        type=int,
        default=1,
        help="Only include files with total >= threshold in the file details section",
    )
    parser.add_argument(
        "--allowlist",
        default="tools/feature_completeness_allowlist.toml",
        help="Optional TOML allowlist for suppressing known false positives",
    )
    args = parser.parse_args()

    src_root = pathlib.Path(args.src).resolve()
    out_path = pathlib.Path(args.output).resolve()
    allowlist_path = pathlib.Path(args.allowlist).resolve() if args.allowlist else None
    allowlist = load_allowlist(allowlist_path)

    scans = [scan_file(path, src_root, allowlist) for path in discover_rust_files(src_root)]
    report = render_markdown(
        scans=scans,
        src_root=src_root,
        threshold=args.threshold,
        allowlist_path=allowlist_path if allowlist_path and allowlist_path.exists() else None,
    )

    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(report + "\n", encoding="utf-8")
    print(f"Feature completeness matrix report written to {out_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
