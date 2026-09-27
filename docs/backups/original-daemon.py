#!/usr/bin/env python3
"""Map an Elgato Stream Deck Pedal to Omarchy desktop actions."""

import select
import subprocess
import time
from pathlib import Path


VENDOR_ID = "0fd9"
PRODUCT_ID = "0086"


def find_pedal():
    for entry in Path("/sys/class/hidraw").glob("hidraw*"):
        device = entry.resolve()
        for parent in (device, *device.parents):
            vendor = parent / "idVendor"
            product = parent / "idProduct"
            if vendor.exists() and product.exists():
                if vendor.read_text().strip() == VENDOR_ID and product.read_text().strip() == PRODUCT_ID:
                    return Path("/dev") / entry.name
                break
    return None


def run(*command):
    subprocess.run(command, check=False, stdout=subprocess.DEVNULL)


def handle(index):
    names = ("left: previous workspace", "middle: toggle dictation", "right: next workspace")
    print(names[index], flush=True)
    if index == 0:
        run("hyprctl", "eval", 'hl.dispatch(hl.dsp.focus({ workspace = "e-1" }))')
    elif index == 1:
        run("voxtype", "record", "toggle")
    elif index == 2:
        run("hyprctl", "eval", 'hl.dispatch(hl.dsp.focus({ workspace = "e+1" }))')


def listen(path):
    previous = [False, False, False]
    with path.open("rb", buffering=0) as pedal:
        poller = select.poll()
        poller.register(pedal, select.POLLIN | select.POLLERR | select.POLLHUP)
        while True:
            events = poller.poll(5000)
            if not events:
                continue
            if events[0][1] & (select.POLLERR | select.POLLHUP):
                return
            report = pedal.read(8)
            if len(report) < 8:
                return
            current = [bool(value) for value in report[4:7]]
            for index, (was_pressed, is_pressed) in enumerate(zip(previous, current)):
                if is_pressed and not was_pressed:
                    handle(index)
            previous = current


def main():
    while True:
        pedal = find_pedal()
        if pedal:
            try:
                listen(pedal)
            except (OSError, PermissionError):
                pass
        time.sleep(2)


if __name__ == "__main__":
    main()
