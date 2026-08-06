#!/usr/bin/env python3
"""Package an already-built ghost-clap-plugin dynamic library as a CLAP artifact."""
from __future__ import annotations
import argparse, plistlib, shutil, sys
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("library", type=Path)
parser.add_argument("--output", type=Path, default=Path("dist"))
parser.add_argument("--name", default="Ghost Agent Host")
args = parser.parse_args()
args.output.mkdir(parents=True, exist_ok=True)

if sys.platform == "darwin":
    bundle = args.output / f"{args.name}.clap"
    binary_dir = bundle / "Contents" / "MacOS"
    binary_dir.mkdir(parents=True, exist_ok=True)
    shutil.copy2(args.library, binary_dir / args.name)
    info = {
        "CFBundleName": args.name,
        "CFBundleDisplayName": args.name,
        "CFBundleIdentifier": "ai.konko.ghost-agent-host",
        "CFBundleVersion": "0.1.0",
        "CFBundleShortVersionString": "0.1.0",
        "CFBundlePackageType": "BNDL",
        "CFBundleExecutable": args.name,
    }
    with (bundle / "Contents" / "Info.plist").open("wb") as handle:
        plistlib.dump(info, handle)
    print(bundle)
else:
    target = args.output / f"{args.name}.clap"
    shutil.copy2(args.library, target)
    print(target)
