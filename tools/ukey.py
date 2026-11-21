#!/usr/bin/env python3
"""uinput key injector for Clippie verification: real key events, not synthetic text.

Creates a virtual keyboard via /dev/uinput and presses a combo like
`ctrl+alt+v` as real kernel key events (same mechanism as ydotool). Used to
verify the full hotkey chain key → Hyprland bind → clippie-ctl → socket →
daemon, which no in-app test can reach.

Requirement: /dev/uinput must be writable by the seat user (ACL or udev rule);
on a default Arch install it is root-only, so grant access once with
`sudo setfacl -m u:$USER /dev/uinput`.

Usage:
    python3 tools/ukey.py ctrl+alt+v
    python3 tools/ukey.py ctrl+alt+1

Keys are `+`-separated; the last key is pressed and released while earlier
(modifier) keys are held. Known keys: a-z, 0-9, ctrl, alt, shift, super,
return/enter, space, tab, esc, backspace.
"""
import fcntl
import os
import struct
import sys
import time

UI_SET_EVBIT = 0x40045564
UI_SET_KEYBIT = 0x40045565
UI_DEV_SETUP = 0x405C5503
UI_DEV_CREATE = 0x5501
UI_DEV_DESTROY = 0x5502

EV_SYN = 0
EV_KEY = 1

KEYMAP = {
    "a": 30, "b": 48, "c": 46, "d": 32, "e": 18, "f": 33, "g": 34, "h": 35,
    "i": 23, "j": 36, "k": 37, "l": 38, "m": 50, "n": 49, "o": 24, "p": 25,
    "q": 16, "r": 19, "s": 31, "t": 20, "u": 22, "v": 47, "w": 17, "x": 45,
    "y": 21, "z": 44,
    "1": 2, "2": 3, "3": 4, "4": 5, "5": 6, "6": 7, "7": 8, "8": 9, "9": 10, "0": 11,
    "ctrl": 29, "alt": 56, "shift": 42, "super": 125,
    "return": 28, "enter": 28, "space": 57, "tab": 15, "esc": 1, "backspace": 14,
}


def event(fd, etype, code, value):
    os.write(fd, struct.pack("<qqHHi", 0, 0, etype, code, value))


def main():
    if len(sys.argv) != 2:
        sys.exit(f"usage: {sys.argv[0]} ctrl+alt+v")
    keys = [k.strip().lower() for k in sys.argv[1].split("+") if k.strip()]
    try:
        codes = [KEYMAP[k] for k in keys]
    except KeyError as e:
        sys.exit(f"unknown key: {e}")

    fd = os.open("/dev/uinput", os.O_WRONLY)
    try:
        fcntl.ioctl(fd, UI_SET_EVBIT, EV_KEY)
        fcntl.ioctl(fd, UI_SET_EVBIT, EV_SYN)
        for code in codes:
            fcntl.ioctl(fd, UI_SET_KEYBIT, code)

        # struct uinput_setup: input_id(8) + name[80] + ff_effects_max(4) = 92 bytes
        name = b"clippie-ukey".ljust(80, b"\0")
        setup = struct.pack("<HHHH", 3, 1, 1, 1) + name + struct.pack("<I", 0)
        fcntl.ioctl(fd, UI_DEV_SETUP, setup)
        fcntl.ioctl(fd, UI_DEV_CREATE)
        time.sleep(0.2)  # let libinput/Hyprland pick up the new keyboard

        # press modifiers, press+release main key, release modifiers
        for code in codes[:-1]:
            event(fd, EV_KEY, code, 1)
            event(fd, EV_SYN, 0, 0)
            time.sleep(0.03)
        event(fd, EV_KEY, codes[-1], 1)
        event(fd, EV_SYN, 0, 0)
        time.sleep(0.05)
        event(fd, EV_KEY, codes[-1], 0)
        event(fd, EV_SYN, 0, 0)
        time.sleep(0.03)
        for code in reversed(codes[:-1]):
            event(fd, EV_KEY, code, 0)
            event(fd, EV_SYN, 0, 0)
        time.sleep(0.1)

        fcntl.ioctl(fd, UI_DEV_DESTROY)
    finally:
        os.close(fd)
    print(f"sent: {'+'.join(keys)}")


if __name__ == "__main__":
    main()
