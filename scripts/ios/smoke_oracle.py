#!/usr/bin/env python3
"""Assert painted regions and receiving-side state, independently of liveness."""
import argparse
from collections import Counter
import json
import math
from pathlib import Path
import struct
import zlib
import importlib.util

_spec = importlib.util.spec_from_file_location("png_colors", Path(__file__).with_name("screenshot-color.py"))
png = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(png)

# Independent RGB expectations from app.rs source constants, never screenshot samples.
TITLE = "yebisu · GPUI 0.3.12 · P0"
EXPECTED = {
    "initial": "P0-input",
    "returned": "P0-input-returned",
    "rotated": "P0-input-returned-rotated",
    "idle-before": "P0-input-returned-rotated",
    "idle": "P0-input-returned-rotated-idle",
    "image-clear": "P0-input-returned-rotated-idle",
}
COLORS = {"text": [("eceff4", 0.02)], "quad": [("88c0d0", 0.5)],
          "image": [("ebcb8b", 0.25), ("b48ead", 0.25)], "input": [("eceff4", 0.005)]}


class SmokeFailure(ValueError):
    pass


def require(condition, reason):
    if not condition:
        raise SmokeFailure(reason)


def rectangle(frame, width, height):
    x, y, w, h = frame["bounds"]
    vw, vh = frame["viewport"]
    left, top = max(0, int(x * width / vw)), max(0, int(y * height / vh))
    right, bottom = min(width, math.ceil((x+w)*width/vw)), min(height, math.ceil((y+h)*height/vh))
    require(right > left and bottom > top, "invalid painted region geometry")
    return left, top, right, bottom


def frame_at(events, stage, region):
    frames = [e for e in events if e.get("kind") == "paint" and e.get("region") == region and e["time"] <= stage["time"]]
    require(bool(frames), f'{stage["stage"]}: missing {region} paint receipt')
    frame = max(frames, key=lambda e: e["time"])
    require(stage["time"] - frame["time"] < 4, f'{stage["stage"]}: stale {region} paint receipt')
    return frame


def validate(directory, verbose=True):
    directory = Path(directory)
    stages = json.loads((directory / "stages.json").read_text())
    events = [json.loads(s) for s in (directory / "app.jsonl").read_text().splitlines()]
    by_stage = {s["stage"]: s for s in stages}
    require(set(EXPECTED) <= set(by_stage), "missing lifecycle screenshot stage")
    receipts = {}
    for name, expected_input in EXPECTED.items():
        stage = by_stage[name]
        width, height, channels, rows = png.decode((directory / stage["file"]).read_bytes())
        for region, expectations in COLORS.items():
            frame = frame_at(events, stage, region)
            left, top, right, bottom = rectangle(frame, width, height)
            # The OS keyboard occludes lower landscape content. Sample the first
            # two source rows of the 32px checker there; image-clear checks it fully.
            if width > height and region == "image":
                bottom = min(bottom, top + max(1, (bottom-top)//16))
            cropped = [row[left*channels:right*channels] for row in rows[top:bottom]]
            counts = png.color_counts(cropped, channels)
            if width > height and region == "input":
                expectations = []  # Native keyboard covers this field; receipt remains mandatory.
            for color, minimum in expectations:
                fraction = png.fraction_near(counts, bytes.fromhex(color), 4)
                require(fraction >= minimum, f'{name}: {region} region expected #{color} >= {minimum}, observed {fraction:.5f}')
                if verbose:
                    print(f'REGION PASS {name} {region} #{color} fraction={fraction:.5f} pixels={(right-left)*(bottom-top)} rect={[left,top,right,bottom]}')
            require(frame["title"] == TITLE, f'{name}: wrong title receipt')
            require(frame["input"] == expected_input, f'{name}: input result expected {expected_input!r}, received {frame["input"]!r}')
            require(frame["marked"] is None, f'{name}: marked text was not committed')
            if region == "input":
                receipts[name] = frame
        require(any(e.get("kind") == "input" and e.get("input") == expected_input and e["time"] <= stage["time"] for e in events), f'{name}: missing input-handler receipt')
        if verbose:
            print(f'RECEIVE PASS {name} input={expected_input!r} marked=None ticks={receipts[name]["ticks"]}')
    require(receipts["returned"]["ticks"] > receipts["initial"]["ticks"], "counter did not advance across background/return")
    require(by_stage["idle"]["time"] - by_stage["idle-before"]["time"] >= 60, "idle interval was shorter than 60 seconds")
    require(receipts["idle"]["ticks"] > receipts["idle-before"]["ticks"], "counter did not advance across idle")
    if verbose:
        print('SMOKE PASS: regions, input, marked text, background return, rotation and >=60-second idle counter progression')
    return events, stages


def write_png(path, width, height, channels, rows):
    def chunk(name, data):
        return struct.pack('>I', len(data)) + name + data + struct.pack('>I', zlib.crc32(name+data)&0xffffffff)
    raw = b''.join(b'\0'+bytes(row) for row in rows)
    Path(path).write_bytes(png.SIGNATURE + chunk(b'IHDR', struct.pack('>IIBBBBB', width,height,8,6 if channels==4 else 2,0,0,0)) + chunk(b'IDAT',zlib.compress(raw,1)) + chunk(b'IEND',b''))


def negative_fixture(source, destination, mode):
    """Mutate captured evidence, never the app's product path."""
    import shutil
    source, destination = Path(source), Path(destination)
    shutil.copytree(source, destination, ignore=shutil.ignore_patterns('result.xcresult', 'attachments', 'launch', 'project', 'negatives'))
    stages = json.loads((destination / 'stages.json').read_text())
    events = [json.loads(line) for line in (destination / 'app.jsonl').read_text().splitlines()]
    if mode in {'text', 'quad', 'image'}:
        stage = next(s for s in stages if s['stage'] == 'initial')
        path = destination / stage['file']
        width, height, channels, rows = png.decode(path.read_bytes())
        left, top, right, bottom = rectangle(frame_at(events, stage, mode), width, height)
        for row in rows[top:bottom]:
            for x in range(left, right):
                row[x*channels:x*channels+3] = bytes.fromhex('2e3440')
        write_png(path, width, height, channels, rows)
    elif mode == 'input':
        for event in events:
            if 'input' in event:
                event['input'] = ''
    elif mode == 'counter-background':
        for event in events:
            if 'ticks' in event:
                event['ticks'] = 0
    elif mode == 'counter-idle':
        before = next(s for s in stages if s['stage'] == 'idle-before')
        frozen = frame_at(events, before, 'input')['ticks']
        for event in events:
            if 'ticks' in event and event['time'] > before['time']:
                event['ticks'] = frozen
    else:
        raise ValueError('unknown negative mode')
    (destination / 'app.jsonl').write_text(''.join(json.dumps(e)+'\n' for e in events))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=Path)
    arguments = parser.parse_args()
    try:
        validate(arguments.directory)
    except (SmokeFailure, OSError, ValueError) as error:
        print(f'SMOKE FAIL: {error}')
        return 1
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
